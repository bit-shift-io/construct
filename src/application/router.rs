//! # Command Router
//!
//! Routes incoming messages to the appropriate command handler (in `interface/commands`).
//! It parses the command string (e.g., `.task`) and dispatches it with the necessary context.

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::application::engine::ExecutionEngine;
use crate::application::feed::FeedManager;
use crate::application::project::ProjectManager;
use crate::domain::config::AppConfig;
use crate::domain::traits::{ChatProvider, LlmProvider};
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
    where
        C: ChatProvider + Clone + Send + Sync + 'static,
    {
        // Cancel Auto-Continue Timer on ANY user message (unless it's the bot itself, handled by caller check usually)
        if sender != chat.room_id() { // Simple check, though main.rs already checks sender != own_user_id
             let mut guard = self.state.lock().await;
             if let Some(room) = guard.rooms.get_mut(&chat.room_id()) {
                 if room.task_completion_time.is_some() {
                     room.task_completion_time = None;
                     // access feed via room
                     if let Some(feed_mutex) = &room.feed_manager {
                         let mut feed = feed_mutex.lock().await;
                         feed.auto_start_timestamp = None;
                         // Force update to remove countdown? Or wait for next sticky?
                         // Ideally we force update if it was currently displaying the countdown.
                         // But router doesn't easily async update feed here without holding locks too long.
                         // We'll let the interaction trigger the next update naturally or rely on the fact the user is typing.
                     }
                 }
             }
        }

        let msg = message.trim();

        // Debug Log (Moved to top)
        let (cmd_preview, args_preview) = if let Some(idx) = msg.find(' ') {
            (&msg[..idx], &msg[idx + 1..])
        } else {
            (msg, "")
        };
        tracing::info!(
            "Router dispatching cmd='{}' args='{}' sender='{}'",
            cmd_preview,
            args_preview,
            sender
        );

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
                msg,
            )
            .await?;

            if let commands::wizard::WizardAction::TransitionToTask {
                prompt,
                display_prompt,
                workdir,
                create_new_folder,
            } = result
            {
                // Transition to Task!
                // Reuse feed from RoomState
                let feed = {
                    let guard = self.state.lock().await;
                    let room = guard.rooms.get(&chat.room_id());
                    let stored_id = room.and_then(|r| r.feed_event_id.clone());
                    room.and_then(|r| r.feed_manager.clone())
                        .unwrap_or_else(|| {
                            Arc::new(Mutex::new(FeedManager::new(
                                Some(workdir.clone()),
                                self.config.system.projects_dir.clone(),
                                self.tools.clone(),
                                stored_id,
                            )))
                        })
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

                commands::task::handle_task(
                    &self.config,
                    &self.state,
                    &engine,
                    chat,
                    &prompt,
                    display_prompt.as_deref(),
                    Some(workdir),
                    create_new_folder,
                )
                .await?;
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
                    cmd_str,
                )
                .await?;
                return Ok(());
            }
        }

        let (cmd, args) = (cmd_preview, args_preview);

        match cmd {
            ".1" | ".2" | ".3" | ".4" => {
                // Interactive Menu Handlers
                // .1 -> Architecture
                // .2 -> Roadmap
                // .3 -> Plan (Active Task or Root)
                // .4 -> Tasks (Active Task)
                
                let workdir = {
                    let guard = self.state.lock().await;
                    let room = guard.rooms.get(&chat.room_id());
                    room.and_then(|r| r.current_working_dir.clone())
                };

                if let Some(wd) = workdir {
                    let active_task = {
                        let guard = self.state.lock().await;
                        let room = guard.rooms.get(&chat.room_id());
                        room.and_then(|r| r.active_task.clone())
                    };

                    let file_to_read = match cmd {
                        ".1" => crate::domain::paths::architecture_path(&wd),
                        ".2" => crate::domain::paths::roadmap_path(&wd),
                        ".3" => {
                            if let Some(task_rel) = &active_task {
                                format!("{}/{}/plan.md", wd, task_rel)
                            } else {
                                format!("{}/plan.md", wd)
                            }
                        }
                        ".4" => {
                            if let Some(task_rel) = &active_task {
                                format!("{}/{}/tasks.md", wd, task_rel)
                            } else {
                                format!("{}/tasks.md", wd)
                            }
                        }
                        _ => unreachable!(),
                    };

                     let client = self.tools.lock().await;
                     match client.read_file(&file_to_read).await {
                         Ok(content) => {
                             let _ = chat.send_message(&content).await;
                         }
                         Err(_) => {
                             let _ = chat.send_notification(&format!("⚠️ File not found: `{}`", file_to_read)).await;
                         }
                     }
                } else {
                    let _ = chat.send_notification("⚠️ No active project found.").await;
                }
                return Ok(());
            }
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
                    // Check for conversation.md (Active Conversation)
                    let active_task = {
                        let guard = self.state.lock().await;
                        guard
                            .rooms
                            .get(&chat.room_id())
                            .and_then(|r| r.active_task.clone())
                    };

                    let conversation_path = if let Some(task_rel) = &active_task {
                        std::path::Path::new(&wd)
                            .join(task_rel)
                            .join("conversation.md")
                    } else {
                        std::path::Path::new(&wd).join("conversation.md")
                    };

                    // Helper validation
                    let conversation_active = {
                        let t = self.tools.lock().await;
                        // Check fast usage? Or just try read?
                        // We can just rely on the file existing.
                        // But we can't easily check existence.
                        // We'll trust checking if we can "read metadata" or similar?
                        // Using read_file might be heavy if big? typically small.
                        match t
                            .read_file(&conversation_path.to_string_lossy().to_string())
                            .await
                        {
                            Ok(_) => true,
                            Err(_) => false,
                        }
                    };

                    if conversation_active {
                        // Continue Conversation
                        commands::misc::handle_ask(
                            &self.config,
                            &self.state,
                            self.tools.clone(),
                            &self.llm,
                            chat,
                            "yes",
                        )
                        .await?;
                        return Ok(());
                    }

                    // Check if plan.md exists?
                    let plan_path = if let Some(task_rel) = &active_task {
                        std::path::Path::new(&wd).join(task_rel).join("plan.md")
                    } else {
                        std::path::Path::new(&wd).join("plan.md")
                    };

                    let plan_exists = {
                        let t = self.tools.lock().await;
                        match t.read_file(&plan_path.to_string_lossy().to_string()).await {
                            Ok(_) => true,
                            Err(_) => false,
                        }
                    };

                    if plan_exists {
                        // Re-initialize Engine
                        let feed = {
                            let guard = self.state.lock().await;
                            let room = guard.rooms.get(&chat.room_id());
                            let stored_id = room.and_then(|r| r.feed_event_id.clone());
                            room.and_then(|r| r.feed_manager.clone())
                                .unwrap_or_else(|| {
                                    Arc::new(Mutex::new(FeedManager::new(
                                        Some(wd.clone()),
                                        self.config.system.projects_dir.clone(),
                                        self.tools.clone(),
                                        stored_id,
                                    )))
                                })
                        };

                        let engine = ExecutionEngine::new(
                            self.config.clone(),
                            self.llm.clone(),
                            self.tools.clone(),
                            feed,
                            self.state.clone(),
                        );

                        // Use handle_start for Execution Phase
                        commands::start::handle_start(&self.config, &self.state, &engine, chat, Some(wd.clone()))
                            .await?;
                    } else {
                        let _ = chat
                            .send_message("No active conversation or plan found to continue.")
                            .await;
                    }
                } else {
                    let _ = chat
                        .send_message("You are not in a project directory.")
                        .await;
                }
            }
            ".new" => {
                commands::new::handle_new(
                    &self.config,
                    &self.project_manager,
                    &self.state,
                    self.tools.clone(),
                    chat,
                    args,
                )
                .await?;
            }
            ".task" => {
                if args.trim().is_empty() {
                    commands::task::start_task_wizard(
                        &self.config,
                        &self.state,
                        self.tools.clone(),
                        chat,
                    )
                    .await?;
                } else {
                    // Initialize Engine

                    // Determine working directory AND existing feed AND stored_id
                    let (workdir, _existing_feed, _stored_id) = {
                        let guard = self.state.lock().await;
                        let room_state = guard.rooms.get(&chat.room_id());
                        let path = room_state
                            .and_then(|r| r.current_working_dir.clone())
                            .or_else(|| room_state.and_then(|r| r.current_project_path.clone()))
                            .or_else(|| self.config.system.projects_dir.clone())
                            .or_else(|| Some(".".to_string()))
                            .unwrap_or_else(|| ".".to_string());
                        let feed = room_state.and_then(|r| r.feed_manager.clone());
                        let fid = room_state.and_then(|r| r.feed_event_id.clone());
                        (path, feed, fid)
                    };

                    // Create NEW Feed for this task (User Request #3)
                    // We knowingly ignore any existing feed in the room to create a fresh start for the milestone.
                    let feed = Arc::new(Mutex::new(FeedManager::new(
                        Some(workdir.clone()),
                        self.config.system.projects_dir.clone(),
                        self.tools.clone(),
                        None, // Force new feed event ID
                    )));

                    // Store feed in RoomState
                    {
                        let mut guard = self.state.lock().await;
                        let room = guard.get_room_state(&chat.room_id());
                        room.feed_manager = Some(feed.clone());
                        // feed_event_id will be updated when feed.initialize() is called (if implemented to do so)
                        // Actually FeedManager::initialize sets the ID if it sends a message?
                        // We need to make sure the new ID is saved to room state eventually.
                        // But Engine updates feed, so it should be fine.
                        room.feed_event_id = None; // Reset so next update captures new ID
                    }

                    let engine = ExecutionEngine::new(
                        self.config.clone(),
                        self.llm.clone(),
                        self.tools.clone(),
                        feed,
                        self.state.clone(),
                    );

                    commands::task::handle_task(
                        &self.config,
                        &self.state,
                        &engine,
                        chat,
                        args,
                        None,
                        Some(workdir),
                        true,
                    )
                    .await?;
                }
            }
            ".run" | ".exec" => {
                commands::admin::handle_admin(
                    &self.config,
                    &self.state,
                    self.tools.clone(),
                    chat,
                    sender,
                    args,
                )
                .await?;
            }
            ".help" => {
                commands::help::handle_help(chat).await?;
            }
            ".agent" => {
                commands::agent::handle_agent(&self.config, &self.state, chat, args).await?;
            }
            ".project" => {
                commands::project::handle_project(
                    &self.config,
                    &self.project_manager,
                    &self.state,
                    chat,
                    args,
                )
                .await?;
            }
            ".list" => {
                commands::project::handle_list(&self.config, &self.project_manager, chat).await?;
            }
            ".status" => {
                commands::misc::handle_status(&self.config, &self.state, chat).await?;
            }
            ".ask" => {
                commands::misc::handle_ask(
                    &self.config,
                    &self.state,
                    self.tools.clone(),
                    &self.llm,
                    chat,
                    args,
                )
                .await?;
            }
            ".read" => {
                commands::misc::handle_read(&self.state, self.tools.clone(), chat, args).await?;
            }
            ".start" => {
                // Resolve ExecutionEngine dependencies
                // We need to resolve workdir similar to .ok/.task logic
                let workdir = {
                    let guard = self.state.lock().await;
                    let room = guard.rooms.get(&chat.room_id());
                    room.and_then(|r| r.current_working_dir.clone())
                };

                let feed = {
                    let guard = self.state.lock().await;
                    let room = guard.rooms.get(&chat.room_id());
                    let stored_id = room.and_then(|r| r.feed_event_id.clone());

                    room.and_then(|r| r.feed_manager.clone())
                        .unwrap_or_else(|| {
                            Arc::new(Mutex::new(FeedManager::new(
                                workdir.clone(),
                                self.config.system.projects_dir.clone(),
                                self.tools.clone(),
                                stored_id,
                            )))
                        })
                };

                let engine = ExecutionEngine::new(
                    self.config.clone(),
                    self.llm.clone(),
                    self.tools.clone(),
                    feed,
                    self.state.clone(),
                );

                // Use handle_start for Execution Phase
                commands::start::handle_start(&self.config, &self.state, &engine, chat, workdir).await?;
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
                        let _ = chat
                            .send_message("🛑 **Task Stopped Instantly (Aborted)**")
                            .await;
                    } else {
                        let _ = chat
                            .send_message("🛑 Stop requested (Flag set, no active handle).")
                            .await;
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
                let _ = chat
                    .send_message(crate::strings::messages::UNKNOWN_COMMAND)
                    .await;
            }
        }

        Ok(())
    }
}
