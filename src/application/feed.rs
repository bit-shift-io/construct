//! # Feed Manager
//!
//! Manages the real-time "Feed" UI message in the chat.
//! It handles updates, sticky logic (re-sending the feed if buried), and rendering the current state.

use crate::domain::traits::ChatProvider;
use crate::domain::types::AgentAction;
use crate::infrastructure::tools::executor::SharedToolExecutor;
use anyhow::Result;
use chrono::Local;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FeedMode {
    Active,
    Squashed,
    PlanReview,
    Final,
    Wizard,
    Conversational,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FeedEntryKind {
    Checkpoint, // Permanent history (e.g. Completed Step)
    Prompt,     // Transient user request
    Activity,   // Transient system process
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FeedEntry {
    timestamp: String,
    pub(crate) kind: FeedEntryKind,
    pub(crate) label: String, // Was action_type
    pub(crate) content: String,
    output: Option<String>,
}

impl FeedEntry {
    fn new(kind: FeedEntryKind, label: String, content: String) -> Self {
        Self {
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            kind,
            label,
            content,
            output: None,
        }
    }

    pub(crate) fn format_active(&self, is_last: bool) -> String {
        let (icon, bold) = match self.kind {
            FeedEntryKind::Checkpoint => ("✅", true),
            FeedEntryKind::Prompt => ("📝", false),
            // Dynamic checkmark logic:
            // If it's Activity (•) AND NOT the last one, it becomes ✅.
            // If it IS the last one, it stays 🔄 (or • as passed in label).
            FeedEntryKind::Activity => ("🔄", true), 
        };

        let mut result = String::new();

        if self.kind == FeedEntryKind::Activity {
            // Special handling for Activity bullets
            // If content starts with "• ", we might replace it.
            if self.content.trim_start().starts_with("• ") {
                if !is_last {
                     // Replace "• " with "✅ "
                     result.push_str(&self.content.replacen("• ", "✅ ", 1));
                } else {
                    // Active item: Replace "• " with "🔄 "
                    result.push_str(&self.content.replacen("• ", "🔄 ", 1));
                }
            } else {
                 result.push_str(&self.content);
            }
        } else if self.kind == FeedEntryKind::Checkpoint {
             let icon = if is_last { "🔄" } else { "✅" };
             if self.label == "Completed" {
                  result.push_str(&format!("{} {}", icon, self.content));
             } else {
                  result.push_str(&format!("{} {}: {}", icon, self.label, self.content));
             }
        } else {
             if bold {
                result.push_str(&format!("**{} {}**", icon, self.label));
            } else {
                result.push_str(&format!("{} {}", icon, self.label));
            }
             if !self.content.is_empty() {
                result.push_str(&format!(": {}", self.content));
            }
        }
        
        result.push('\n');
        
        if let Some(output) = &self.output {
            if !output.is_empty() {
                 let truncated = if output.len() > 300 {
                    format!("{}...", &output[..300])
                } else {
                    output.clone()
                };
                result.push_str(&format!("> {}\n", truncated.replace('\n', "\n> ")));
            }
        }

        result
    }

    pub(crate) fn format_squashed(&self) -> String {
        let mut result = String::new();
        match self.kind {
            FeedEntryKind::Activity => {
                // Force checkmark if it was a bullet
                 if self.content.trim_start().starts_with("• ") {
                     result.push_str(&self.content.replacen("• ", "✅ ", 1));
                } else {
                     result.push_str(&self.content);
                }
            },
            FeedEntryKind::Checkpoint => {
                 if self.label == "Completed" {
                      result.push_str(&format!("✅ {}", self.content));
                 } else if self.label == "Failed" {
                      result.push_str(&format!("❌ {}", self.content));
                 } else {
                      result.push_str(&format!("✅ {}: {}", self.label, self.content));
                 }
            },
            FeedEntryKind::Prompt => {
                 result.push_str(&format!("📝 {}", self.content));
            }
        }
        result.push('\n');
        result
    }
}

#[derive(Debug, Clone)]
pub struct FeedManager {
    pub(crate) entries: Vec<FeedEntry>,
    pub mode: FeedMode,
    _project_path: Option<String>,
    pub(crate) current_task: Option<String>,
    feed_event_id: Option<String>,
    pub(crate) recent_activities: Vec<String>,
    projects_root: Option<String>,
    _tools: SharedToolExecutor,

    pub plan_content: Option<String>,
    pub roadmap_content: Option<String>,
    pub completion_message: Option<String>,
    pub last_agent_thought: Option<String>,
    pub title: String,
}

impl FeedManager {
    pub fn new(
        project_path: Option<String>,
        projects_root: Option<String>,
        tools: SharedToolExecutor,
        feed_event_id: Option<String>,
    ) -> Self {
        Self {
            entries: Vec::new(),
            mode: FeedMode::Active,
            _project_path: project_path,
            current_task: None,
            feed_event_id,
            recent_activities: Vec::new(),
            projects_root,
            _tools: tools,
            plan_content: None,
            roadmap_content: None,
            completion_message: None,
            last_agent_thought: None,
            title: "Construct".to_string(),
        }
    }

    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }

    pub fn initialize(&mut self, task: String) {
        self.current_task = Some(task);
        self.entries.clear();
        self.mode = FeedMode::Active;
        self.recent_activities.clear();
        self.feed_event_id = None;
        self.completion_message = None;
        self.last_agent_thought = None;

        self.add_activity("Task Started".to_string());
    }

    /// Starts a fresh block for a new Phase (e.g. Planning, Execution).
    /// This prevents "wall of text" by forcing a new message and clearing previous activities.
    #[allow(dead_code)]
    pub fn start_new_block(&mut self, label: String) {
        // Archive current state?
        // For now, just reset for the new view.
        self.recent_activities.clear();
        self.feed_event_id = None; // Force new message
        self.completion_message = None;
        self.add_activity(label);
    }

    pub fn set_completion(&mut self, message: String) {
        self.completion_message = Some(message);
    }

    // --- Type-Specific Add Methods ---

    pub fn add_checkpoint(&mut self, label: String, content: String) {
        self.entries
            .push(FeedEntry::new(FeedEntryKind::Checkpoint, label, content));
    }

    pub fn add_prompt(&mut self, content: String) {
        self.entries.push(FeedEntry::new(
            FeedEntryKind::Prompt,
            "Prompt".to_string(),
            content,
        ));
    }

    pub fn add_activity(&mut self, content: String) {
        // Smart Formatting:
        // If content already has a list marker or checkmark, don't add default bullet.
        let activity_log = if content.starts_with("✅") || content.starts_with("•") || content.starts_with("❌") {
            content.clone()
        } else {
            format!("• {}", content)
        };

        // Deduplicate: If the last activity is identical (ignoring status icon), do nothing.
        if let Some(last) = self.recent_activities.last() {
            // Strip known prefixes
            let clean_last = last
                .trim_start_matches("✅ ")
                .trim_start_matches("❌ ")
                .trim_start_matches("• ")
                .trim_start_matches("🔄 ");
            let clean_new = content
                .trim_start_matches("✅ ")
                .trim_start_matches("❌ ")
                .trim_start_matches("• ")
                .trim_start_matches("🔄 ");

            if clean_last == clean_new {
                return;
            }
        }

        self.recent_activities.push(activity_log.clone());
        if self.recent_activities.len() > 15 {
            self.recent_activities.remove(0);
        }
        self.entries.push(FeedEntry::new(
            FeedEntryKind::Activity,
            "System".to_string(),
            activity_log,
        ));
    }

    pub fn replace_last_activity(&mut self, new_content: String, success: bool) {
        // Update recent log
        if let Some(last) = self.recent_activities.last_mut() {
            let icon = if success { "✅" } else { "❌" };
            *last = format!("{} {}", icon, new_content);
        }
        // Update entry
        if let Some(entry) = self.entries.last_mut()
            && entry.kind == FeedEntryKind::Activity
        {
            entry.content = new_content; // Update text
            // We rely on format_active to add icon?
            // format_active uses 🔄 for Activity.
            // We might need to change Kind to Checkpoint for "Done" items?
            // Checkpoint uses ✅.
            if success {
                entry.kind = FeedEntryKind::Checkpoint;
                entry.label = "Completed".to_string(); // or System?
            } else {
                // Keep Activity but maybe mark failed?
                entry.label = "Failed".to_string();
            }
            entry.output = None; // Clear any output text
        }
    }
    // Prefer strict methods now.

    pub fn update_last_entry(&mut self, output: String, _success: bool) {
        if let Some(entry) = self.entries.last_mut() {
            // entry.status update removed.
            // Only update output.
            entry.output = Some(output);
        }
    }

    pub fn clean_stack(&mut self) {
        // Pop entries from the end until we hit a Checkpoint or empty
        while let Some(last) = self.entries.last() {
            if last.kind != FeedEntryKind::Checkpoint {
                self.entries.pop();
            } else {
                break;
            }
        }
    }

    pub fn projects_root(&self) -> Option<String> {
        self.projects_root.clone()
    }

    pub fn squash(&mut self) {
        self.mode = FeedMode::Squashed;
    }

    pub fn get_feed_content(&self) -> String {
        use crate::application::feed_formatter::FeedFormatter;
        match self.mode {
            FeedMode::Active => FeedFormatter::format_active(self),
            FeedMode::Squashed => FeedFormatter::format_squashed(self),
            FeedMode::PlanReview => FeedFormatter::format_plan_review(self),
            FeedMode::Final => FeedFormatter::format_final(self),
            FeedMode::Wizard => FeedFormatter::format_wizard(self),
            FeedMode::Conversational => FeedFormatter::format_conversational(self),
        }
    }

    pub fn add_completion_message(&mut self, msg: String) {
        self.completion_message = Some(msg);
    }

    pub fn set_agent_thought(&mut self, thought: String) {
        self.last_agent_thought = Some(thought);
    }

    pub async fn process_action(&mut self, action: &AgentAction) {
        match action {
            AgentAction::ShellCommand(cmd) => {
                let sanitized =
                    crate::application::utils::sanitize_path(cmd, self.projects_root.as_deref());
                self.add_activity(format!("Running: `{}`", sanitized));
            }
            AgentAction::Done => {
                self.squash();
                self.add_activity("Task Complete".to_string());
            }
            AgentAction::WriteFile(path, _) => {
                self.add_activity(format!("Writing: `{}`", path));
            }
            AgentAction::Find(path, pattern) => {
                let sanitized =
                    crate::application::utils::sanitize_path(path, self.projects_root.as_deref());
                self.add_activity(format!("Finding: `{} {}`", sanitized, pattern));
            }
            AgentAction::ReadFile(path) => {
                self.add_activity(format!("Reading: `{}`", path));
            }
            AgentAction::ListDir(path) => {
                self.add_activity(format!("Listing: `{}`", path));
            }
            AgentAction::SwitchMode(phase) => {
                self.add_activity(format!("Switching to mode: {}", phase));
            }
        }
    }

    /// Primary Method: Updates the feed message in the chat
    /// Implements Sticky Logic
    pub async fn update_feed(&mut self, chat: &impl ChatProvider) -> Result<()> {
        let content = self.get_feed_content();
        if content.is_empty() {
            return Ok(());
        }
        let latest_event_id = chat
            .get_latest_event_id()
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        // Determine if we should edit or send new
        let should_send_new = if let Some(feed_id) = &self.feed_event_id {
            if let Some(latest) = latest_event_id {
                // If latest event != feed_event_id, someone interrupted
                latest != *feed_id
            } else {
                false
            }
        } else {
            // No feed ID, must send new
            true
        };

        if should_send_new {
            // Send new message
            match chat.send_message(&content).await {
                Ok(new_id) => {
                    self.feed_event_id = Some(new_id);
                }
                Err(e) => {
                    tracing::error!("Failed to send feed message: {}", e);
                }
            }
        } else {
            // Edit existing
            if let Some(feed_id) = &self.feed_event_id
                && let Err(e) = chat.edit_message(feed_id, &content).await
            {
                tracing::error!("Failed to edit feed message: {}", e);
            }
        }

        Ok(())
    }
}
