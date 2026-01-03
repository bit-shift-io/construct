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

    pub async fn route<C>(&self, chat: &C, message: &str, sender: &str) -> Result<()> 
    where C: ChatProvider + Clone + Send + Sync + 'static
    {
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
            let result = commands::wizard::handle_step(
                &self.config,
                &self.state,
                &self.project_manager,
                chat,
                msg
            ).await?;

            if let commands::wizard::WizardAction::TransitionToTask { prompt, workdir } = result {
                 // Transition to Task!
                 // Reuse feed from RoomState
                 let feed = {
                     let guard = self.state.lock().await;
                     let room = guard.rooms.get(&chat.room_id());
                     room.and_then(|r| r.feed_manager.clone())
                         .unwrap_or_else(|| Arc::new(Mutex::new(FeedManager::new(Some(workdir.clone()), self.config.system.projects_dir.clone(), self.tools.clone()))))
                 };
                 
                 // Update Feed Mode to Active (from Wizard)
                 {
                     let mut f = feed.lock().await;
                     f.initialize("Initializing Project Content...".to_string());
                 }

                 let engine = ExecutionEngine::new(
                     self.config.clone(),
                     self.llm.clone(),
                     self.tools.clone(),
                     feed, 
                     self.state.clone(),
                 );
                 
                 commands::task::handle_task(&self.state, &engine, chat, &prompt, Some(workdir)).await?;
            }
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
            ".ok" | ".continue" | ".approve" | ".yes" => {
                 // 1. Try handling pending approval
                 if commands::misc::try_handle_approval(&self.state, chat, true).await? {
                     return Ok(());
                 }

                 // Check if we are in a project and have a plan?
                 // Or just assume the user wants to continue the last intention?
                 // For now, let's treat it as "Execute the plan" if we are in a project.
                 
                 let workdir = {
                     let guard = self.state.lock().await;
                     let room = guard.rooms.get(&chat.room_id());
                     room.and_then(|r| r.current_working_dir.clone())
                 };
                 
                 if let Some(wd) = workdir {
                     // Check if plan.md exists?
                     // Verify via tools
                     let plan_exists = {
                         let _t = self.tools.lock().await;
                         let _path = std::path::Path::new(&wd).join("plan.md");
                         // We can't easily check existence with current ToolExecutor without read/list.
                         // Let's just assume and try to run.
                         // Or use "ls"?
                         true 
                     };

                     if plan_exists {
                          // Re-initialize Engine
                         let feed = {
                             let guard = self.state.lock().await;
                             let room = guard.rooms.get(&chat.room_id());
                             room.and_then(|r| r.feed_manager.clone())
                                 .unwrap_or_else(|| Arc::new(Mutex::new(FeedManager::new(Some(wd.clone()), self.config.system.projects_dir.clone(), self.tools.clone()))))
                         };
                         
                         let engine = ExecutionEngine::new(
                             self.config.clone(),
                             self.llm.clone(),
                             self.tools.clone(),
                             feed, 
                             self.state.clone(),
                         );
                         
                         let _ = chat.send_message("🚀 **Executing Plan**...").await;
                         // Execute Plan
                         commands::task::handle_task(&self.state, &engine, chat, "Execute the implementation details described in `plan.md`. Implement the code.", Some(wd)).await?;
                     } else {
                         let _ = chat.send_message("No active project or plan found to continue.").await;
                     }
                 } else {
                      let _ = chat.send_message("You are not in a project directory.").await;
                 }
            }
            ".new" => {
                commands::new::handle_new(&self.config, &self.project_manager, &self.state, self.tools.clone(), chat, args).await?;
            }
            ".task" => {
                if args.trim().is_empty() {
                    commands::task::start_task_wizard(&self.config, &self.state, self.tools.clone(), chat).await?;
                } else {
                    // Initialize Engine
                    
                    // Determine working directory from RoomState or Default
                    let (workdir, existing_feed) = {
                        let guard = self.state.lock().await;
                        let room_state = guard.rooms.get(&chat.room_id());
                        let path = room_state.and_then(|r| r.current_working_dir.clone())
                            .or_else(|| room_state.and_then(|r| r.current_project_path.clone()))
                            .or_else(|| self.config.system.projects_dir.clone())
                            .or_else(|| Some(".".to_string())).unwrap_or_else(|| ".".to_string());
                        let feed = room_state.and_then(|r| r.feed_manager.clone());
                        (path, feed)
                    };

                    // Create Feed or Reuse
                    let feed = existing_feed.unwrap_or_else(|| Arc::new(Mutex::new(FeedManager::new(Some(workdir.clone()), self.config.system.projects_dir.clone(), self.tools.clone()))));
                    
                    // Store feed in RoomState if not present?
                    // Ideally yes, so it persists for wizard/other flows
                    {
                        let mut guard = self.state.lock().await;
                        let room = guard.get_room_state(&chat.room_id());
                        if room.feed_manager.is_none() {
                            room.feed_manager = Some(feed.clone());
                        }
                    }

                    let engine = ExecutionEngine::new(
                        self.config.clone(),
                        self.llm.clone(),
                        self.tools.clone(),
                        feed,
                        self.state.clone(), 
                    );
                    
                    commands::task::handle_task(&self.state, &engine, chat, args, Some(workdir)).await?;
                }
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
                commands::misc::handle_status(&self.config, &self.state, chat).await?;
            }
            ".ask" => {
                commands::misc::handle_ask(&self.state, self.tools.clone(), &self.llm, chat, args).await?;
            }
            ".read" => {
                commands::misc::handle_read(self.tools.clone(), chat, args).await?;
            }
            ".stop" => {
                 let mut guard = self.state.lock().await;
                 let room = guard.get_room_state(&chat.room_id());
                 room.stop_requested = true;
                 
                 // Instant Abort
                 if let Some(handle_lock) = &room.task_handle {
                     let mut handle = handle_lock.lock().await;
                     if let Some(h) = handle.take() {
                         h.abort();
                         let _ = chat.send_message("🛑 **Task Stopped Instantly (Aborted)**").await;
                     } else {
                         let _ = chat.send_message("🛑 Stop requested (Flag set, no active handle).").await;
                     }
                 } else {
                     let _ = chat.send_message("🛑 Stop requested (Flag set).").await;
                 }
            }
            ".deny" | ".no" | ".cancel" => {
                 if commands::misc::try_handle_approval(&self.state, chat, false).await? {
                     return Ok(());
                 }
                 if cmd != ".cancel" {
                    let _ = chat.send_message("No pending approval to deny.").await;
                 }
            }
             _ => {
                 let _ = chat.send_message(crate::strings::messages::UNKNOWN_COMMAND).await;
             }
        }
        
        Ok(())
    }
}
