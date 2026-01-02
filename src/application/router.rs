//! # Command Router
//!
//! Routes incoming messages to the appropriate command handler (in `interface/commands`).
//! It parses the command string (e.g., `.task`) and dispatches it with the necessary context.

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::domain::config::AppConfig;
use crate::domain::traits::{ChatProvider, LlmProvider};
use crate::application::project::ProjectManager;
use crate::application::engine::ExecutionEngine;
use crate::application::feed::FeedManager;
use crate::infrastructure::tools::executor::SharedToolExecutor;
use crate::interface::commands;

use crate::application::state::BotState;

pub struct CommandRouter {
    config: AppConfig,
    tools: SharedToolExecutor,
    llm: Arc<dyn LlmProvider>,
    project_manager: Arc<ProjectManager>,
    state: Arc<Mutex<BotState>>,
}
impl CommandRouter {
    pub fn new(
        config: AppConfig,
        tools: SharedToolExecutor,
        llm: Arc<dyn LlmProvider>,
        project_manager: Arc<ProjectManager>,
        state: Arc<Mutex<BotState>>,
    ) -> Self {
        Self {
            config,
            tools,
            llm,
            project_manager,
            state,
        }
    }

    pub async fn route(&self, chat: &impl ChatProvider, message: &str, sender: &str) -> Result<()> {
        let msg = message.trim();
        
        // Debug Log (Moved to top)
        let (cmd_preview, args_preview) = if let Some(idx) = msg.find(' ') {
            (&msg[..idx], &msg[idx+1..])
        } else {
            (msg, "")
        };
        tracing::info!("Router dispatching cmd='{}' args='{}' sender='{}'", cmd_preview, args_preview, sender);

        // 1. Check for Active Wizard (High Priority Interception)
        // BUT allow .new and .cancel (and maybe .help?) to bypass interception
        let bypass_commands = [".new", ".cancel", ".help"];
        let is_bypass = bypass_commands.contains(&cmd_preview);

        let is_wizard_active = if !is_bypass {
            let guard = self.state.lock().await;
            if let Some(room) = guard.rooms.get(&chat.room_id()) {
                room.wizard.active
            } else {
                false
            }
        } else {
            false
        };

        if is_wizard_active {
            commands::wizard::handle_step(
                &self.config,
                &self.state,
                &self.project_manager,
                chat,
                msg
            ).await?;
            return Ok(());
        }

        if !msg.starts_with('.') && !msg.starts_with(',') {
            return Ok(());
        }

        // Handle Comma Shortcut for Admin Command
        if let Some(cmd_str) = msg.strip_prefix(',') {
            let cmd_str = cmd_str.trim();
            if !cmd_str.is_empty() {
                commands::admin::handle_admin(
                     &self.config, 
                     &self.state,
                     self.tools.clone(), 
                     chat, 
                     sender,
                     cmd_str
                 ).await?;
                 return Ok(());
            }
        }

        let (cmd, args) = (cmd_preview, args_preview);

        match cmd {
            ".new" => {
                commands::new::handle_new(&self.config, &self.project_manager, &self.state, chat, args).await?;
            }
            ".task" => {
                // Initialize Engine
                
                // Determine working directory from RoomState or Default
                let workdir = {
                    let guard = self.state.lock().await;
                    let room_state = guard.rooms.get(&chat.room_id());
                    room_state.and_then(|r| r.current_working_dir.clone())
                        .or_else(|| room_state.and_then(|r| r.current_project_path.clone()))
                        .or_else(|| self.config.system.projects_dir.clone())
                        .or_else(|| Some(".".to_string()))
                };

                // Create Feed
                let feed = Arc::new(Mutex::new(FeedManager::new(workdir.clone(), self.tools.clone())));
                let engine = ExecutionEngine::new(
                    self.config.clone(),
                    self.llm.clone(),
                    self.tools.clone(),
                    feed, 
                );
                
                commands::task::handle_task(&engine, chat, args, workdir).await?;
            }
            ".run" | ".exec" => {
                 commands::admin::handle_admin(
                     &self.config, 
                     &self.state,
                     self.tools.clone(), 
                     chat, 
                     sender, 
                     args
                 ).await?;
            }
            ".help" => {
                commands::help::handle_help(chat).await?;
            }
            ".project" => {
                commands::project::handle_project(&self.config, &self.project_manager, &self.state, chat, args).await?;
            }
            ".list" => {
                commands::project::handle_list(&self.config, &self.project_manager, chat).await?;
            }
            ".status" => {
                commands::misc::handle_status(&self.state, chat).await?;
            }
            ".ask" => {
                commands::misc::handle_ask(&self.state, self.tools.clone(), &self.llm, chat, args).await?;
            }
            ".read" => {
                commands::misc::handle_read(self.tools.clone(), chat, args).await?;
            }
             _ => {
                 let _ = chat.send_message(crate::strings::messages::UNKNOWN_COMMAND).await;
             }
        }

        Ok(())
    }
}
