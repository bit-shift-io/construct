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
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FeedMode {
    Active,
    Squashed,
    Final,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FeedEntry {
    timestamp: String,
    action_type: String,
    content: String,
    status: String, 
    output: Option<String>,
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
        let status_icon = match self.status.as_str() {
            "success" => "✅",
            "failed" => "❌",
            _ => "📋",
        };
        format!("{} **[{}]** {}", status_icon, self.timestamp, self.content)
    }
}

#[derive(Debug, Clone)]
pub struct FeedManager {
    entries: Vec<FeedEntry>,
    mode: FeedMode,
    project_path: Option<String>,
    current_task: Option<String>,
    feed_event_id: Option<String>,
    recent_activities: Vec<String>,
    tools: SharedToolExecutor,
}

impl FeedManager {
    pub fn new(project_path: Option<String>, tools: SharedToolExecutor) -> Self {
        Self {
            entries: Vec::new(),
            mode: FeedMode::Active,
            project_path,
            current_task: None,
            feed_event_id: None,
            recent_activities: Vec::new(),
            tools,
        }
    }

    pub fn initialize(&mut self, task: String) {
        self.current_task = Some(task);
        self.entries.clear();
        self.mode = FeedMode::Active;
        self.recent_activities.clear();
        self.feed_event_id = None; // Reset feed ID on new task? Or keep sticky? Usually new task = new feed.
        
        self.add_entry("Task Started".to_string(), self.current_task.clone().unwrap_or_default());
    }

    pub fn add_entry(&mut self, action_type: String, content: String) {
        let activity = format!("• {}", content);
        self.recent_activities.push(activity);
        if self.recent_activities.len() > 15 {
            self.recent_activities.remove(0);
        }

        self.entries.push(FeedEntry::new(action_type, content));
    }

    pub fn update_last_entry(&mut self, output: String, success: bool) {
        if let Some(entry) = self.entries.last_mut() {
            entry.status = if success { "success" } else { "failed" }.to_string();
            entry.output = Some(output);
        }
    }

    pub fn squash(&mut self) {
        self.mode = FeedMode::Squashed;
    }

    pub fn finalize(&mut self) {
        self.mode = FeedMode::Final;
    }

    fn get_feed_content(&self) -> String {
        match self.mode {
            FeedMode::Active => self.format_active(),
            FeedMode::Squashed => self.format_squashed(),
            FeedMode::Final => self.format_final(),
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
            let start = self.entries.len().saturating_sub(5);
            for entry in &self.entries[start..] {
                content.push_str(&format!("{}\n\n", entry.format_active()));
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
            if entry.status != "running" {
                content.push_str(&format!("{}\n", entry.format_squashed()));
            }
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
            if entry.status == "success" {
                content.push_str(&format!("• {}\n", entry.content));
            }
        }
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
        content.push_str(&format!("\n**Completed**: {}", timestamp));
        content
    }

    pub async fn process_action(&mut self, action: &AgentAction) {
        match action {
            AgentAction::ShellCommand(cmd) => {
                self.add_entry("COMMAND".to_string(), cmd.clone());
            }
            AgentAction::Done => {
                self.squash();
                self.add_entry("STATUS".to_string(), "Task Complete".to_string());
            }
        }
    }

    /// Save feed to project directory as feed.md using ToolExecutor
    pub async fn save_to_disk(&self) {
        if let Some(project_path) = &self.project_path {
            let feed_path = Path::new(project_path).join("feed.md");
            // Use tools to write
            let client = self.tools.lock().await;
            // TODO: convert path to string properly
            let path_str = feed_path.to_string_lossy().to_string();
            let content = self.get_feed_content();
            
            if let Err(e) = client.write_file(&path_str, &content).await {
                // We fallback to tracing in console if fails
                tracing::error!("Failed to write feed.md via tools: {}", e);
            }
        }
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
                // Can't determine latest, safe backend default: edit if we have ID? 
                // Or send new to be safe?
                // Let's assume edit is safe if we have an ID.
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
                     // If edit fails (e.g. message deleted), maybe clear ID?
                     // self.feed_event_id = None; 
                }
            }
        }
        
        Ok(())
    }
}
