//! # Bot State
//!
//! Defines the persistent state of the application (`BotState`) and per-room state (`RoomState`).
//! This includes active tasks, agent configurations, wizard status, and conversation history context.
//! It handles serialization and deserialization to/from JSON.

use crate::application::feed::FeedManager;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use tokio::sync::watch;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WizardStep {
    ProjectName,
    ProjectType, // App, Lib, etc.
    Stack,       // Language/Framework
    Description, // Accumulate (Project)
    Confirmation,
    TaskDescription, // Accumulate (Task)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum WizardMode {
    #[default]
    Project,
    Task,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct WizardState {
    pub active: bool,
    pub mode: WizardMode,
    pub step: Option<WizardStep>,
    pub data: HashMap<String, String>,
    pub buffer: String, // For multi-message input
}

/// State for a single chat room.
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct RoomState {
    pub current_project_path: Option<String>,
    #[serde(default)]
    pub current_working_dir: Option<String>,
    pub active_task: Option<String>,
    pub active_agent: Option<String>,
    pub active_model: Option<String>,
    #[serde(default)]
    pub stop_requested: bool,
    #[serde(default)]
    pub last_model_list: Vec<String>,
    #[serde(default)]
    pub last_agent_list: Vec<String>,
    #[serde(default)]
    pub is_task_completed: bool,
    #[serde(skip)]
    pub _abort_handle: Option<Arc<watch::Sender<bool>>>,
    #[serde(default)]
    pub wizard: WizardState,
    #[serde(default)]
    pub model_cooldowns: HashMap<String, i64>, // "agent:model" -> timestamp
    #[serde(default)]
    pub last_request_times: HashMap<String, i64>, // "agent" -> timestamp of last request start
    pub pending_command: Option<String>,
    #[serde(default)]
    pub pending_agent_response: Option<String>,
    #[serde(default)]
    pub last_command: Option<String>,
    #[serde(default)]
    pub command_retry_count: u32,
    #[serde(default)]
    pub last_message_event_id: Option<String>,
    #[serde(default)]
    pub feed_event_id: Option<String>,
    #[serde(skip)]
    pub _feed_manager: Option<FeedManager>,
    #[serde(skip)]
    pub _input_tx: Option<tokio::sync::mpsc::Sender<String>>,
}

impl RoomState {


}

/// Persistent state of the bot, mapping Room IDs to their respective room states.
/// Saved to `data/state.json`.
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct BotState {
    #[serde(default)]
    pub rooms: HashMap<String, RoomState>,
}

impl BotState {
    /// Gets or creates the state for a specific room.
    pub fn get_room_state(&mut self, room_id: &str) -> &mut RoomState {
        self.rooms
            .entry(room_id.to_string())
            .or_insert_with(RoomState::default)
    }
    /// Loads the state from `data/state.json` or returns default.
    pub fn load() -> Self {
        if let Ok(content) = fs::read_to_string("data/state.json") {
            if let Ok(mut state) = serde_json::from_str::<Self>(&content) {
                // Sanitize: Reset wizards on load
                for room in state.rooms.values_mut() {
                    room.wizard.active = false;
                    room.wizard.step = None;
                    room.wizard.data.clear();
                    room.wizard.buffer.clear();
                }
                return state;
            }
        }
        Self::default()
    }

    /// Persists the current state to `data/state.json`.
    pub fn save(&self) {
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = fs::write("data/state.json", content);
        }
    }
}
