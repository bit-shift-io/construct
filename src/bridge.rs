use crate::commands;
use crate::config::AppConfig;
use crate::services::ChatService;
use crate::state::BotState;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Manages the connections and interactions between rooms and agents.
/// Supports multiple chat platforms via the generic ChatService trait.
pub struct BridgeManager {
    config: AppConfig,
    state: Arc<Mutex<BotState>>,
    mcp_manager: Option<Arc<crate::mcp::McpManager>>,
}

impl BridgeManager {
    /// Creates a new BridgeManager instance.
    pub fn new(
        config: AppConfig,
        state: Arc<Mutex<BotState>>,
        mcp_manager: Option<Arc<crate::mcp::McpManager>>,
    ) -> Self {
        Self {
            config,
            state,
            mcp_manager,
        }
    }

    /// Dispatches a message to the appropriate handler.
    pub async fn dispatch<S: ChatService + Clone + Send + 'static>(
        &self,
        room: &S,
        sender: &str,
        msg_body: &str,
    ) {
        // Handle admin shell commands
        if msg_body.starts_with(',') {
            crate::commands::admin::handle_command(
                &self.config,
                self.state.clone(),
                room,
                sender,
                msg_body[1..].trim(),
            )
            .await;
            return;
        }

        // Handle active wizard session
        {
            let wizard_active = {
                let mut bot_state = self.state.lock().await;
                let room_state = bot_state.get_room_state(&room.room_id());
                room_state.wizard.active
            };

            if wizard_active {
                crate::commands::wizard::handle_input(
                    &self.config,
                    self.state.clone(),
                    self.mcp_manager.clone(),
                    room,
                    msg_body,
                )
                .await;
                return;
            }
        }

        // Support only . as command prefix for other commands
        if !msg_body.starts_with('.') {
            // User sent a non-command message - reset tracking
            // so the next bot response will be a new message instead of editing
            {
                let mut bot_state = self.state.lock().await;
                let room_state = bot_state.get_room_state(&room.room_id());
                room_state.last_message_event_id = None;
                room_state.feed_event_id = None; // Also reset feed event ID
                bot_state.save();
            }
            return;
        }

        let mut parts = msg_body[1..].splitn(2, ' ');
        let trigger = parts.next().unwrap_or("");
        let argument = parts.next().unwrap_or("").trim();

        // Check permissions for help context
        let sender_lower = sender.to_lowercase();
        let is_admin = self
            .config
            .system
            .admin
            .iter()
            .any(|u| u.to_lowercase() == sender_lower);

        match trigger {
            "help" => commands::handle_help(&self.config, self.state.clone(), room, is_admin).await,
            "project" | "workdir" => {
                commands::handle_project(
                    &self.config,
                    self.state.clone(),
                    self.mcp_manager.clone(),
                    argument,
                    room,
                )
                .await
            }
            "set" => {
                commands::handle_set(
                    &self.config,
                    self.state.clone(),
                    self.mcp_manager.clone(),
                    argument,
                    room,
                )
                .await
            }
            "list" => commands::handle_list(&self.config, self.mcp_manager.clone(), room).await,
            "agents" => commands::handle_agents(&self.config, self.state.clone(), room).await,
            "agent" => {
                commands::handle_agent(&self.config, self.state.clone(), argument, room).await
            }
            "models" => commands::handle_models(&self.config, self.state.clone(), room).await,
            "model" => commands::handle_model(self.state.clone(), argument, room).await,
            "read" => {
                commands::handle_read(self.state.clone(), self.mcp_manager.clone(), argument, room)
                    .await
            }
            "new" => {
                commands::handle_new(
                    &self.config,
                    self.state.clone(),
                    self.mcp_manager.clone(),
                    argument,
                    room,
                )
                .await
            }
            "task" => {
                commands::handle_task(
                    &self.config,
                    self.state.clone(),
                    self.mcp_manager.clone(),
                    argument,
                    room,
                )
                .await
            }
            "modify" => {
                commands::handle_modify(
                    &self.config,
                    self.state.clone(),
                    self.mcp_manager.clone(),
                    argument,
                    room,
                )
                .await
            }
            "approve" => {
                commands::handle_approve(
                    &self.config,
                    self.state.clone(),
                    self.mcp_manager.clone(),
                    room,
                )
                .await
            }
            "continue" => {
                commands::handle_continue(
                    &self.config,
                    self.state.clone(),
                    self.mcp_manager.clone(),
                    room,
                )
                .await
            }
            "start" => {
                commands::handle_start(
                    &self.config,
                    self.state.clone(),
                    self.mcp_manager.clone(),
                    room,
                )
                .await
            }
            "ok" => {
                commands::handle_ok(
                    &self.config,
                    self.state.clone(),
                    self.mcp_manager.clone(),
                    room,
                )
                .await
            }
            "no" => commands::handle_no(&self.config, self.state.clone(), room).await,
            "stop" => commands::handle_stop(self.state.clone(), room).await,
            "ask" => {
                commands::handle_ask(
                    &self.config,
                    self.state.clone(),
                    self.mcp_manager.clone(),
                    argument,
                    room,
                )
                .await
            }
            "reject" => commands::handle_no(&self.config, self.state.clone(), room).await,
            "changes" => {
                commands::handle_changes(self.state.clone(), self.mcp_manager.clone(), room).await
            }
            "commit" => {
                commands::handle_commit(
                    self.state.clone(),
                    self.mcp_manager.clone(),
                    argument,
                    room,
                )
                .await
            }
            "discard" => {
                commands::handle_discard(self.state.clone(), self.mcp_manager.clone(), room).await
            }
            "cleanup" => {
                commands::handle_cleanup(&self.config, self.state.clone(), room, sender).await
            }
            "build" => {
                commands::handle_build(
                    &self.config,
                    self.state.clone(),
                    self.mcp_manager.clone(),
                    room,
                )
                .await
            }
            "deploy" => {
                commands::handle_deploy(
                    &self.config,
                    self.state.clone(),
                    self.mcp_manager.clone(),
                    room,
                )
                .await
            }
            "check" => {
                commands::handle_check(
                    &self.config,
                    self.state.clone(),
                    self.mcp_manager.clone(),
                    room,
                )
                .await
            }
            "status" => {
                commands::handle_status(
                    &self.config,
                    self.state.clone(),
                    self.mcp_manager.clone(),
                    room,
                )
                .await
            }
            _ => {
                // Ignore unknown dot commands
            }
        }
    }
}
