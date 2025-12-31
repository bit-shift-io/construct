use crate::core::state::project::ProjectStateManager;
use crate::core::utils::AgentAction;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use tracing;

/// Represents the three stages of feed evolution
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FeedMode {
    /// Active: Verbose, real-time updates during execution
    Active,
    /// Squashed: Concise one-liners per completed task
    Squashed,
    /// Final: Clean bullet list of all completed work
    Final,
}

/// A single entry in the feed
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FeedEntry {
    timestamp: String,
    action_type: String,
    content: String,
    status: String,         // "running", "success", "failed"
    output: Option<String>, // Truncated output (only in Active mode)
}

impl FeedEntry {
    fn new(action_type: String, content: String) -> Self {
        Self {
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            action_type,
            content,
            status: "running".to_string(),
            output: None,
        }
    }

    /// Format as active (verbose) entry
    fn format_active(&self) -> String {
        let status_icon = match self.status.as_str() {
            "running" => "⏳",
            "success" => "✅",
            "failed" => "❌",
            _ => "📋",
        };

        let mut result = format!(
            "{} **[{}]** {}",
            status_icon, self.timestamp, self.action_type
        );

        if !self.content.is_empty() {
            result.push_str(&format!(": `{}`", self.content));
        }

        if let Some(output) = &self.output {
            if !output.is_empty() {
                // Truncate output to 300 chars
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

    /// Format as squashed (concise) entry
    fn format_squashed(&self) -> String {
        let status_icon = match self.status.as_str() {
            "success" => "✅",
            "failed" => "❌",
            _ => "📋",
        };

        format!("{} **[{}]** {}", status_icon, self.timestamp, self.content)
    }
}

/// Manages the three-stage feed system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedManager {
    entries: Vec<FeedEntry>,
    mode: FeedMode,
    project_path: Option<String>,
    current_task: Option<String>,
    feed_event_id: Option<String>,
    recent_activities: Vec<String>, // Keep last 15 for active mode
    project_state: Option<ProjectStateManager>, // For logging to state.md
}

impl FeedManager {
    /// Create a new FeedManager
    pub fn new(project_path: Option<String>) -> Self {
        let project_state = project_path.clone().map(ProjectStateManager::new);

        Self {
            entries: Vec::new(),
            mode: FeedMode::Active,
            project_path,
            current_task: None,
            feed_event_id: None,
            recent_activities: Vec::new(),
            project_state,
        }
    }

    /// Initialize the feed for a new task
    pub fn initialize(&mut self, task: String) {
        self.current_task = Some(task);
        self.entries.clear();
        self.mode = FeedMode::Active;
        self.recent_activities.clear();

        // Add initial entry
        self.add_entry(
            "Task Started".to_string(),
            self.current_task.clone().unwrap_or_default(),
        );
    }

    /// Add a new entry to the feed
    pub fn add_entry(&mut self, action_type: String, content: String) {
        // Update recent activities for active mode first (before content is moved)
        let activity = format!("• {}", content);
        self.recent_activities.push(activity);

        let entry = FeedEntry::new(action_type, content);
        self.entries.push(entry);

        // Keep only last 15 activities
        if self.recent_activities.len() > 15 {
            self.recent_activities.remove(0);
        }
    }

    /// Update the most recent entry with output
    pub fn update_last_entry(&mut self, output: String, success: bool) {
        if let Some(entry) = self.entries.last_mut() {
            entry.status = if success { "success" } else { "failed" }.to_string();
            entry.output = Some(output);
        }
    }

    /// Mark the feed as paused (waiting for user input)
    pub fn pause(&mut self) {
        self.add_entry(
            "WAITING".to_string(),
            "Waiting for user input...".to_string(),
        );
    }

    /// Transition to squashed mode (when a task completes)
    pub fn squash(&mut self) {
        self.mode = FeedMode::Squashed;
    }

    /// Transition to final mode (when all tasks complete)
    pub fn finalize(&mut self) {
        self.mode = FeedMode::Final;
    }

    /// Get the current feed content as markdown
    pub fn get_feed_content(&self) -> String {
        match self.mode {
            FeedMode::Active => self.format_active(),
            FeedMode::Squashed => self.format_squashed(),
            FeedMode::Final => self.format_final(),
        }
    }

    /// Get a clone of the project state manager
    pub fn get_project_state_manager(&self) -> Option<ProjectStateManager> {
        self.project_state
            .as_ref()
            .map(|m| ProjectStateManager::new(m.project_path.clone()))
    }

    /// Format feed in active mode (verbose, real-time)
    fn format_active(&self) -> String {
        let mut content = String::from("**🔄 Active Task**\n\n");

        if let Some(task) = &self.current_task {
            content.push_str(&format!("**Task**: {}\n\n", task));
        }

        content.push_str("**Recent Activity** (last 15):\n");
        for activity in &self.recent_activities {
            content.push_str(&format!("{}\n", activity));
        }

        // Show last few entries in detail
        if self.entries.len() > 0 {
            content.push_str("\n**Latest Details**:\n");
            let start = if self.entries.len() > 5 {
                self.entries.len() - 5
            } else {
                0
            };
            for entry in &self.entries[start..] {
                content.push_str(&format!("{}\n", entry.format_active()));
                content.push_str("\n");
            }
        }

        content
    }

    /// Format feed in squashed mode (concise, one task = one line)
    fn format_squashed(&self) -> String {
        let mut content = String::from("**📋 Task Progress**\n\n");

        if let Some(task) = &self.current_task {
            content.push_str(&format!("**Task**: {}\n\n", task));
        }

        content.push_str("**Completed Steps**:\n");
        for entry in &self.entries {
            if entry.status != "running" {
                content.push_str(&format!("{}\n", entry.format_squashed()));
            }
        }

        content
    }

    /// Format feed in final mode (clean summary)
    fn format_final(&self) -> String {
        let mut content = String::from("**✅ Execution Complete**\n\n");

        if let Some(task) = &self.current_task {
            content.push_str(&format!("**Task**: {}\n\n", task));
        }

        content.push_str("**Summary**:\n");
        for entry in &self.entries {
            if entry.status == "success" {
                content.push_str(&format!("• {}\n", entry.content));
            }
        }

        // Add completion timestamp
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
        content.push_str(&format!("\n**Completed**: {}", timestamp));

        content
    }

    /// Save feed to project directory as feed.md
    pub fn save_to_disk(&self) {
        if let Some(project_path) = &self.project_path {
            let feed_path = Path::new(project_path).join("feed.md");
            let content = self.get_feed_content();

            if let Err(e) = fs::write(&feed_path, content) {
                tracing::error!("Failed to write feed.md: {}", e);
            }
        }
    }

    /// Get the Matrix event ID of the feed message (for editing)
    pub fn get_event_id(&self) -> Option<&String> {
        self.feed_event_id.as_ref()
    }

    /// Set the Matrix event ID after sending the initial feed message
    pub fn set_event_id(&mut self, event_id: String) {
        self.feed_event_id = Some(event_id);
    }

    /// Update feed based on agent action
    pub fn process_action(&mut self, action: &AgentAction) {
        match action {
            AgentAction::ShellCommand(cmd) => {
                self.add_entry("COMMAND".to_string(), cmd.clone());
            }
            AgentAction::Done => {
                // Task completed, transition to squashed mode
                self.squash();
                self.add_entry("STATUS".to_string(), "Task Complete".to_string());
            }
        }
    }

    /// Update feed with command output
    pub fn update_with_output(&mut self, output: &str, success: bool) {
        self.update_last_entry(output.to_string(), success);
        self.save_to_disk();

        // Also log to project state.md
        if let Some(ref state_manager) = self.project_state {
            if let Some(last_entry) = self.entries.last() {
                let log_content = format!("{}: {}", last_entry.action_type, last_entry.content);
                let _ = state_manager.log_command(&log_content, output, success);
            }
        }
    }

    /// Mark task as complete and transition to final mode
    pub fn complete_task(&mut self) {
        self.finalize();
        self.save_to_disk();

        // Log task completion to project state
        if let Some(ref state_manager) = self.project_state {
            let summary = format!(
                "Task completed: {}",
                self.current_task.as_ref().unwrap_or(&"Unknown".to_string())
            );
            let _ = state_manager.log_note(&summary);
        }
    }
}
