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
use crate::infrastructure::mcp::client::SharedMcpClient;
use crate::interface::commands;

use crate::application::state::BotState;

pub struct CommandRouter {
    config: AppConfig,
    mcp: SharedMcpClient,
    llm: Arc<dyn LlmProvider>,
    project_manager: Arc<ProjectManager>,
    state: Arc<Mutex<BotState>>,
}
impl CommandRouter {
    pub fn new(
        config: AppConfig,
        mcp: SharedMcpClient,
        llm: Arc<dyn LlmProvider>,
        project_manager: Arc<ProjectManager>,
        state: Arc<Mutex<BotState>>,
    ) -> Self {
        Self {
            config,
            mcp,
            llm,
            project_manager,
            state,
        }
    }

    pub async fn route(&self, chat: &impl ChatProvider, message: &str) -> Result<()> {
        let msg = message.trim();
        if !msg.starts_with('.') {
            return Ok(());
        }

        let (cmd, args) = if let Some(idx) = msg.find(' ') {
            (&msg[..idx], &msg[idx+1..])
        } else {
            (msg, "")
        };

        match cmd {
            ".new" => {
                commands::new::handle_new(&self.config, &self.project_manager, chat, args).await?;
            }
            ".task" => {
                // Initialize Engine
                // Assume CWD is root of projects_dir or explicitly passed arg?
                // For MVP, if args start with /, treat as path?
                // Or just use default project dir if set?
                // We'll use config.system.projects_dir as base, but we need specific project context.
                // admin.rs handles global CWD? NO, admin.rs commands are ephemeral/stateless currently in V2 mostly.
                // We need `ApplicationState` to track active project per room?
                // For now, let's assume we REQUIRE a project path or use a hardcoded dev path.
                // Better: Parse args. If first arg is path, use it. Else use default.
                
                // Determine working directory
                let workdir = {
                    let guard = self.state.lock().await;
                    let room_state = guard.rooms.get(&chat.room_id());
                    room_state.and_then(|r| r.current_project_path.clone())
                        .or_else(|| self.config.system.projects_dir.clone())
                        .or_else(|| Some(".".to_string()))
                };

                // Create Feed
                let feed = Arc::new(Mutex::new(FeedManager::new(workdir.clone(), self.mcp.clone())));
                let engine = ExecutionEngine::new(
                    self.config.clone(),
                    self.llm.clone(),
                    self.mcp.clone(),
                    feed, 
                );
                
                commands::task::handle_task(&engine, chat, args, workdir).await?;
            }
            ".run" | ".exec" => {
                 commands::admin::handle_admin(
                     &self.config, 
                     self.mcp.clone(), 
                     chat, 
                     "user", // TODO: Extract sender from chat?
                     args, 
                     None
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
                commands::misc::handle_ask(&self.state, self.mcp.clone(), &self.llm, chat, args).await?;
            }
            ".read" => {
                commands::misc::handle_read(self.mcp.clone(), chat, args).await?;
            }
             _ => {
                 let _ = chat.send_message("Unknown command.").await;
             }
        }

        Ok(())
    }
}
