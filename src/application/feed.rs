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
                // User requested less backticks. Just show as quote or plain? 
                // Using indented block or quote is cleaner than code block for logs sometimes.
                // But code block preserves formatting.
                // User said "Excessive backticks". Maybe standard message has too many?
                // Or "Latest Details" has too many.
                // "Latest Details" calls format_active().
                // format_active wraps output in ```.
                // Let's change to blockquote `> ` or just text if it's short.
                // Or just remove the surrounding ```.
                result.push_str(&format!("\n> {}\n", truncated.replace('\n', "\n> ")));
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
    
    // New fields for Plan Review
    pub plan_content: Option<String>,
    pub roadmap_content: Option<String>,
    pub completion_message: Option<String>,
}

impl FeedManager {
    pub fn new(
        project_path: Option<String>, 
        projects_root: Option<String>, 
        tools: SharedToolExecutor, 
        feed_event_id: Option<String>
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
        }
    }

    pub fn initialize(&mut self, task: String) {
        self.current_task = Some(task);
        self.entries.clear();
        self.mode = FeedMode::Active;
        self.recent_activities.clear();
        self.feed_event_id = None; 
        self.completion_message = None;
        
        self.add_activity("Task Started".to_string());
    }

    /// Starts a fresh block for a new Phase (e.g. Planning, Execution).
    /// This prevents "wall of text" by forcing a new message and clearing previous activities.
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
        self.entries.push(FeedEntry::new(FeedEntryKind::Checkpoint, label, content));
    }

    pub fn add_prompt(&mut self, content: String) {
        self.entries.push(FeedEntry::new(FeedEntryKind::Prompt, "Prompt".to_string(), content));
    }

    pub fn add_activity(&mut self, content: String) {
        let activity_log = format!("• {}", content);
        
        // Deduplicate: If the last activity is identical (ignoring status icon), do nothing.
        if let Some(last) = self.recent_activities.last() {
             // Strip known prefixes
             let clean_last = last.trim_start_matches("✅ ").trim_start_matches("❌ ").trim_start_matches("• ").trim_start_matches("🔄 ");
             let clean_new = content.trim_start_matches("✅ ").trim_start_matches("❌ ").trim_start_matches("• ").trim_start_matches("🔄 ");
             
             // Also handle "Writing file X" vs "Written file X" etc?
             // Maybe too complex. Let's just catch exact matches of content first.
             // If input is "Writing file X", and last is "✅ Written file X" -> diff strings.
             // So this only stops "Writing file X" if last was "Writing file X".
             
             if clean_last == clean_new {
                  return;
             }
             
             // Heuristic: If last was "Written `path`" and new is "Writing `path`", skip?
             // last: "Written `specs/roadmap.md`"
             // new: "Writing file `specs/roadmap.md`" -> diff.
        }

        self.recent_activities.push(activity_log);
         if self.recent_activities.len() > 15 {
            self.recent_activities.remove(0);
        }
        self.entries.push(FeedEntry::new(FeedEntryKind::Activity, "System".to_string(), content));
    }

    pub fn replace_last_activity(&mut self, new_content: String, success: bool) {
        // Update recent log
        if let Some(last) = self.recent_activities.last_mut() {
            let icon = if success { "✅" } else { "❌" };
            *last = format!("{} {}", icon, new_content);
        }
        // Update entry
        if let Some(entry) = self.entries.last_mut() {
            if entry.kind == FeedEntryKind::Activity {
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


    fn get_feed_content(&self) -> String {
        match self.mode {
            FeedMode::Active => self.format_active(),
            FeedMode::Squashed => self.format_squashed(),
            FeedMode::PlanReview => self.format_plan_review(),
            FeedMode::Final => self.format_final(),
            FeedMode::Wizard => self.format_wizard(),
            FeedMode::Conversational => self.format_conversational(),
        }
    }
    

    
    fn format_plan_review(&self) -> String {
        // "REPLACE IS DESIRED"
        // "Last line should say..."
        let mut content = String::new();
        // Maybe header?
        content.push_str("**📋 Plan Generated**\n\n");
        
        if let Some(r) = &self.roadmap_content {
            content.push_str("### Roadmap\n");
            // Truncate if massive? Or show full?
            // User implies full display "show roadmap.md and plan.md"
            content.push_str(r);
            content.push_str("\n\n");
        }
        
        if let Some(p) = &self.plan_content {
            if self.roadmap_content.is_some() {
                content.push_str("---\n\n");
            }
            content.push_str("### Implementation Plan\n");
            content.push_str(p);
            content.push_str("\n\n");
        }
        
        content.push_str("✅ **Plan Generated**: Type `.start` to proceed or `.ask` to refine.");
        content
    }

    fn format_active(&self) -> String {
        let mut content = String::from("**🔄 Thinking & Planning...**\n\n");
        if let Some(task) = &self.current_task {
            // Only show first line
            let summary = task.lines().next().unwrap_or(task);
            let truncated = if summary.len() > 100 {
                format!("{}...", &summary[..100])
            } else {
                summary.to_string()
            };
            content.push_str(&format!("**Task**: {}\n\n", truncated));
        }
        content.push_str("**Progress**:\n");
        for activity in &self.recent_activities {
            content.push_str(&format!("{}\n", activity));
        }
        
        // Only show detailed output if there's an error (or strictly requested)
        // We assume 'output' is populated mostly on errors or meaningful results now.
        if let Some(last) = self.entries.last() {
            if let Some(out) = &last.output {
                if !out.is_empty() {
                    content.push_str(&format!("\n> {}\n", out.replace('\n', "\n> ")));
                }
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
            AgentAction::WriteFile(path, _) => {
                self.add_activity(format!("Writing: `{}`", path));
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

    pub fn format_wizard(&self) -> String {
        let mut content = String::from("**🧙 New Project Wizard**\n\n");
        
        for entry in &self.entries {
            content.push_str(&entry.format_active());
        }
        content
    }

    fn format_conversational(&self) -> String {
        let mut content = String::new();
        
        // Check if we have meaningful activity (ignore just "Task Started")
        let has_real_activity = self.recent_activities.iter().any(|a| !a.contains("Task Started"));

        if has_real_activity {
             content.push_str("**Activity**:\n");
             for activity in &self.recent_activities {
                 // Skip Task Started in display if we want to be cleaner? 
                 // Or keep it. Let's keep it if there are other things.
                 content.push_str(&format!("{}\n", activity));
             }
             content.push_str("\n---\n");
        }
        
        if let Some(msg) = &self.completion_message {
            content.push_str(msg);
        } else {
             content.push_str("Thinking...");
        }
        
        content
    }

    /// Primary Method: Updates the feed message in the chat
    /// Implements Sticky Logic
    pub async fn update_feed(&mut self, chat: &impl ChatProvider) -> Result<()> {
        let content = self.get_feed_content();
        if content.is_empty() {
            return Ok(());
        }
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

