//! # Feed Manager
//!
//! Manages the real-time "Feed" UI message in the chat.
//! It handles updates, sticky logic (re-sending the feed if buried), and rendering the current state.

use crate::infrastructure::tools::executor::SharedToolExecutor;
use crate::domain::traits::ChatProvider;
use crate::domain::types::AgentAction;
use anyhow::Result;
use chrono::Local;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FeedMode {
    Active,
    Squashed,
    Final,
    Wizard,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FeedEntryKind {
    Checkpoint, // Permanent history (e.g. Completed Step)
    Prompt,     // Transient user request
    Activity,   // Transient system process
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FeedEntry {
    timestamp: String,
    kind: FeedEntryKind,
    label: String,   // Was action_type
    content: String,
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

    fn format_active(&self) -> String {
        let (icon, bold) = match self.kind {
            FeedEntryKind::Checkpoint => ("✅", true),
            FeedEntryKind::Prompt => ("📝", false),
            FeedEntryKind::Activity => ("🔄", true),
        };

        let mut result = String::new();
        
        // Checkpoints usually look like "✅ Label: Content"
        // Prompts "📝 Content"
        // Activities "🔄 Content" (Label usually hidden or same as content?)

        match self.kind {
            FeedEntryKind::Checkpoint => {
                if self.content.contains('\n') {
                    result.push_str(&format!("**{} {}**:\n{}\n", icon, self.label, self.content));
                } else {
                     result.push_str(&format!("**{} {}**: {}\n", icon, self.label, self.content));
                }
            }
            FeedEntryKind::Prompt => {
                 result.push_str(&format!("{} {}\n", icon, self.content));
            }
            FeedEntryKind::Activity => {
                if bold {
                    result.push_str(&format!("{} **{}**\n", icon, self.content));
                } else {
                    result.push_str(&format!("{} {}\n", icon, self.content)); 
                }
            }
        }

        if let Some(output) = &self.output {
             if !output.is_empty() {
                let truncated = if output.len() > 300 {
                    format!("{}...", &output[..300])
                } else {
                    output.clone()
                };
                result.push_str(&format!("\n```\n{}\n```", truncated));
            }
        }
        
        result
    }

    fn format_squashed(&self) -> String {
        // Only show checkpoints in squashed view
        match self.kind {
            FeedEntryKind::Checkpoint => self.format_active(), // Reuse active format for checkpoints
            _ => String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FeedManager {
    entries: Vec<FeedEntry>,
    pub mode: FeedMode,
    _project_path: Option<String>,
    current_task: Option<String>,
    feed_event_id: Option<String>,
    recent_activities: Vec<String>,
    projects_root: Option<String>,
    _tools: SharedToolExecutor,
}

impl FeedManager {
    pub fn new(project_path: Option<String>, projects_root: Option<String>, tools: SharedToolExecutor) -> Self {
        Self {
            entries: Vec::new(),
            mode: FeedMode::Active,
            _project_path: project_path,
            current_task: None,
            feed_event_id: None,
            recent_activities: Vec::new(),
            projects_root,
            _tools: tools,
        }
    }

    pub fn initialize(&mut self, task: String) {
        self.current_task = Some(task);
        self.entries.clear();
        self.mode = FeedMode::Active;
        self.recent_activities.clear();
        self.feed_event_id = None; 
        
        self.add_activity("Task Started".to_string());
    }

    // --- Type-Specific Add Methods ---

    pub fn add_checkpoint(&mut self, label: String, content: String) {
        self.entries.push(FeedEntry::new(FeedEntryKind::Checkpoint, label, content));
    }

    pub fn add_prompt(&mut self, content: String) {
        self.entries.push(FeedEntry::new(FeedEntryKind::Prompt, "Prompt".to_string(), content));
    }

    pub fn add_activity(&mut self, content: String) {
        let activity_log = format!("• {}", content);
        self.recent_activities.push(activity_log);
         if self.recent_activities.len() > 15 {
            self.recent_activities.remove(0);
        }
        self.entries.push(FeedEntry::new(FeedEntryKind::Activity, "System".to_string(), content));
    }
    
    // Legacy support or generic add?
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


    fn get_feed_content(&self) -> String {
        match self.mode {
            FeedMode::Active => self.format_active(),
            FeedMode::Squashed => self.format_squashed(),
            FeedMode::Final => self.format_final(),
            FeedMode::Wizard => self.format_wizard(),
        }
    }

    fn format_active(&self) -> String {
        let mut content = String::from("**🔄 Active Task**\n\n");
        if let Some(task) = &self.current_task {
            content.push_str(&format!("**Task**: {}\n\n", task));
        }
        content.push_str("**Recent Activity** (last 15):\n");
        for activity in &self.recent_activities {
            content.push_str(&format!("{}\n", activity));
        }
        if !self.entries.is_empty() {
            content.push_str("\n**Latest Details**:\n");
            // Show all active stack items? Or just last 5?
            // With Clean Stack, entries should be relevant.
            let start = self.entries.len().saturating_sub(5);
            for entry in &self.entries[start..] {
                 content.push_str(&entry.format_active());
            }
        }
        content
    }

    fn format_squashed(&self) -> String {
        let mut content = String::from("**📋 Task Progress**\n\n");
        if let Some(task) = &self.current_task {
            content.push_str(&format!("**Task**: {}\n\n", task));
        }
        content.push_str("**Completed Steps**:\n");
        for entry in &self.entries {
            content.push_str(&entry.format_squashed());
        }
        content
    }

    fn format_final(&self) -> String {
        let mut content = String::from("**✅ Execution Complete**\n\n");
        if let Some(task) = &self.current_task {
            content.push_str(&format!("**Task**: {}\n\n", task));
        }
        content.push_str("**Summary**:\n");
        for entry in &self.entries {
            if entry.kind == FeedEntryKind::Checkpoint {
                content.push_str(&format!("• {}: {}\n", entry.label, entry.content));
            }
        }
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
        content.push_str(&format!("\n**Completed**: {}", timestamp));
        content
    }

    pub async fn process_action(&mut self, action: &AgentAction) {
        match action {
            AgentAction::ShellCommand(cmd) => {
                let sanitized = crate::application::utils::sanitize_path(cmd, self.projects_root.as_deref());
                self.add_activity(format!("Running: `{}`", sanitized));
            }
            AgentAction::Done => {
                self.squash();
                self.add_activity("Task Complete".to_string());
            }
        }
    }

    pub fn format_wizard(&self) -> String {
        let mut content = String::from("**🧙 New Project Wizard**\n\n");
        
        for entry in &self.entries {
            content.push_str(&entry.format_active());
        }
        content
    }

    /// Primary Method: Updates the feed message in the chat
    /// Implements Sticky Logic
    pub async fn update_feed(&mut self, chat: &impl ChatProvider) -> Result<()> {
        let content = self.get_feed_content();
        let latest_event_id = chat.get_latest_event_id().await.map_err(|e| anyhow::anyhow!(e))?;

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
            if let Some(feed_id) = &self.feed_event_id {
                if let Err(e) = chat.edit_message(feed_id, &content).await {
                     tracing::error!("Failed to edit feed message: {}", e);
                }
            }
        }
        
        Ok(())
    }
}

