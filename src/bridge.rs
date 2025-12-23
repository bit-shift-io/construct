use crate::commands;
use crate::config::AppConfig;
use crate::state::BotState;
use matrix_sdk::{room::Room, ruma::events::room::message::SyncRoomMessageEvent};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::Mutex;

/// Manages the connections and interactions between Matrix rooms and agents.
/// Supports multiple chat rooms and multiple agents.
pub struct BridgeManager {
    config: AppConfig,
    state: Arc<Mutex<BotState>>,
    start_time: SystemTime,
}

impl BridgeManager {
    /// Creates a new BridgeManager instance.
    pub fn new(config: AppConfig, state: Arc<Mutex<BotState>>) -> Self {
        Self {
            config,
            state,
            start_time: SystemTime::now(),
        }
    }

    /// Handles an incoming Matrix message event.
    /// Routes the message to the appropriate handler in the commands module.
    pub async fn handle_message(&self, event: SyncRoomMessageEvent, room: Room) {
        // Ensure we only process messages from rooms we have joined
        if room.state() != matrix_sdk::RoomState::Joined {
            return;
        }
        let SyncRoomMessageEvent::Original(event) = event else {
            return;
        };

        // Ignore messages from self
        if event.sender == room.own_user_id() {
            return;
        }

        // Ignore messages sent before the bot started
        let ts = SystemTime::UNIX_EPOCH
            + std::time::Duration::from_millis(event.origin_server_ts.0.into());
        if ts < self.start_time {
            return;
        }

        let msg_body = event.content.body();

        // Handle admin shell commands
        if msg_body.starts_with(',') {
            crate::admin::handle_command(&self.config, self.state.clone(), &room, event.sender.as_str(), msg_body[1..].trim()).await;
            return;
        }

        // Handle active wizard session
        {
            let wizard_active = {
                let mut bot_state = self.state.lock().await;
                let room_state = bot_state.get_room_state(room.room_id().as_str());
                room_state.wizard.active
            };

            if wizard_active {
                crate::wizard::handle_input(&self.config, self.state.clone(), &room, msg_body).await;
                return;
            }
        }

        // Support only . as command prefix for other commands
        if !msg_body.starts_with('.') {
            return;
        }

        let mut parts = msg_body[1..].splitn(2, ' ');
        let trigger = parts.next().unwrap_or("");
        let argument = parts.next().unwrap_or("").trim();

        // Check permissions for help context
        // Check permissions for help context
        let sender_lower = event.sender.as_str().to_lowercase();
        let is_admin = self.config.system.admin.iter().any(|u| u.to_lowercase() == sender_lower);

        match trigger {
            "help" => commands::handle_help(&self.config, self.state.clone(), &room, is_admin).await,
            "project" | "workdir" => {
                commands::handle_project(&self.config, self.state.clone(), argument, &room).await
            }
            "set" => commands::handle_set(&self.config, self.state.clone(), argument, &room).await,
            "list" => commands::handle_list(&self.config, &room).await,
            "agents" => commands::handle_agents(&self.config, self.state.clone(), &room).await,
            "model" => commands::handle_model(self.state.clone(), argument, &room).await,
            "read" => commands::handle_read(self.state.clone(), argument, &room).await,
            "new" => commands::handle_new(&self.config, self.state.clone(), argument, &room).await,
            "task" => {
                commands::handle_task(&self.config, self.state.clone(), argument, &room).await
            }
            "modify" => {
                commands::handle_modify(&self.config, self.state.clone(), argument, &room).await
            }
            "approve" => commands::handle_approve(&self.config, self.state.clone(), &room).await,
            "continue" => commands::handle_continue(&self.config, self.state.clone(), &room).await,
            "start" => commands::handle_start(&self.config, self.state.clone(), &room).await,
            "stop" => commands::handle_stop(self.state.clone(), &room).await,
            "ask" => commands::handle_ask(&self.config, self.state.clone(), argument, &room).await,
            "reject" => commands::handle_reject(self.state.clone(), &room).await,
            "changes" => commands::handle_changes(self.state.clone(), &room).await,
            "commit" => commands::handle_commit(self.state.clone(), argument, &room).await,
            "discard" => commands::handle_discard(self.state.clone(), &room).await,
            "build" => commands::handle_build(&self.config, self.state.clone(), &room).await,
            "deploy" => commands::handle_deploy(&self.config, self.state.clone(), &room).await,
            "status" => commands::handle_status(self.state.clone(), &room).await,
            _ => {
                // Ignore unknown dot commands
            }
        }
    }
}
