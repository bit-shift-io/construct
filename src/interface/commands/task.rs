//! # Task Command
//!
//! Handles the `.task` command.
//! Initializes the `ExecutionEngine` to autonomously perform a complex multi-step task based on user input.

use crate::application::engine::ExecutionEngine;
use crate::application::feed::{FeedManager, FeedMode};
use crate::application::state::{BotState, WizardMode, WizardStep};
use crate::domain::traits::ChatProvider;
use crate::infrastructure::tools::executor::SharedToolExecutor;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::domain::config::AppConfig;

pub async fn start_task_wizard(
    config: &AppConfig,
    state: &Arc<Mutex<BotState>>,
    tools: SharedToolExecutor,
    chat: &impl ChatProvider,
) -> Result<()> {
    // Initialize Feed
    // We need to resolve workdir for the feed
    let workdir = {
        let mut guard = state.lock().await;
        let room_state = guard.get_room_state(&chat.room_id());
        room_state
            .current_working_dir
            .clone()
            .unwrap_or_else(|| ".".to_string())
    };

    let feed_id = {
        let mut guard = state.lock().await;
        let room_state = guard.get_room_state(&chat.room_id());
        room_state.feed_event_id.clone()
    };

    let feed = Arc::new(Mutex::new(FeedManager::new(
        Some(workdir),
        config.system.projects_dir.clone(),
        tools.clone(),
        feed_id,
    )));
    
    {
        let mut f = feed.lock().await;
        f.set_title("🧙 New Task Wizard".to_string());
    }

    {
        let mut guard = state.lock().await;
        let room_state = guard.get_room_state(&chat.room_id());
        room_state.wizard.active = true;
        room_state.wizard.mode = WizardMode::Task;
        room_state.wizard.step = Some(WizardStep::TaskDescription);
        room_state.wizard.data.clear();
        room_state.wizard.buffer.clear();
        room_state.active_agent = Some("default".to_string()); // Default agent

        // Set Feed
        room_state.feed_manager = Some(feed.clone());
    }

    // Initial Wizard Entry
    {
        let mut f = feed.lock().await;
        f.mode = FeedMode::Wizard;
        f.add_prompt("Please describe the task you want to perform.\nType `.ok` when finished (multi-line supported).".to_string());
        f.update_feed(chat).await?;
    }

    Ok(())
}

pub async fn handle_task<C>(
    config: &AppConfig,
    state: &Arc<Mutex<BotState>>,
    engine: &ExecutionEngine,
    chat: &C,
    task: &str,
    display_task: Option<&str>,
    workdir: Option<String>,
    create_new_folder: bool,
) -> Result<()>
where
    C: ChatProvider + Clone + Send + Sync + 'static,
{
    // Validate if inside a project?
    if workdir.is_none() {
        let _ = chat
            .send_notification(crate::strings::messages::NOT_IN_PROJECT)
            .await;
        return Ok(());
    }

    // Resolve Active Agent
    let agent_name = {
        let mut guard = state.lock().await;
        let room = guard.get_room_state(&chat.room_id());

        // IMPORTANT: Clear any pending stop request from previous sessions
        room.stop_requested = false;

        // Ensure active agent is set
        if room.active_agent.is_none() {
            // Fallback to first available agent if "default" is not found
            let default_agent = if config.agents.contains_key("default") {
                "default".to_string()
            } else {
                config.agents.keys().next().cloned().unwrap_or_else(|| "default".to_string())
            };
            room.active_agent = Some(default_agent);
        }

        // Reset Phase to Planning, UNLESS we are in NewProject phase (from wizard)
        if room.task_phase != crate::application::state::TaskPhase::NewProject {
            room.task_phase = crate::application::state::TaskPhase::Planning;
        }

        if create_new_folder {
            // Create Task Subfolder Logic
            // Find next ID
            let wd = workdir.as_ref().unwrap();
            let tasks_dir = std::path::Path::new(wd).join("tasks");
            // Ensure tasks dir exists (in case it wasn't made yet)
            let _ = std::fs::create_dir_all(&tasks_dir);

            let mut max_id = 0;
            if let Ok(entries) = std::fs::read_dir(&tasks_dir) {
                for entry in entries.flatten() {
                    if let Some(file_name) = entry.file_name().to_str() {
                        if let Some(id_str) = file_name.split('-').next() {
                            if let Ok(id) = id_str.parse::<u32>() {
                                if id > max_id {
                                    max_id = id;
                                }
                            }
                        }
                    }
                }
            }
            let next_id = max_id + 1;
            // Sanitize Task Description for Folder Name
            // Use only alphanumeric and dashes, max 30 chars
            let safe_desc: String = task
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == ' ')
                .map(|c| {
                    if c == ' ' {
                        '-'
                    } else {
                        c.to_ascii_lowercase()
                    }
                })
                .collect();
            let safe_desc = if safe_desc.len() > 30 {
                &safe_desc[..30]
            } else {
                &safe_desc
            };

            let task_folder_name = format!("{:03}-{}", next_id, safe_desc.trim_matches('-'));
            let task_path = tasks_dir.join(&task_folder_name);

            let _ = std::fs::create_dir_all(&task_path);

            // Write Templates
            let req_content =
                crate::strings::templates::REQUEST_TEMPLATE.replace("{{OBJECTIVE}}", task);
            let _ = std::fs::write(task_path.join("request.md"), req_content);
            
            // Initialize Plan
            let plan_content = crate::strings::templates::PLAN_TEMPLATE;
            let _ = std::fs::write(task_path.join("plan.md"), plan_content);

            // Initialize Walkthrough
            let walkthrough_content = crate::strings::templates::WALKTHROUGH_TEMPLATE;
            let _ = std::fs::write(task_path.join("walkthrough.md"), walkthrough_content);

            // Initialize Tasks (Empty stub)
            let tasks_content = "# Active Task List\n\n- [ ] Initial Task Setup\n";
            let _ = std::fs::write(task_path.join("tasks.md"), tasks_content);

            // Set Active Task Folder
            let rel_task_path = format!("tasks/{}", task_folder_name);
            room.active_task = Some(rel_task_path);
        }

        room.active_agent.clone().unwrap()
    };

    // Run task using Engine
    let engine_clone = engine.clone();
    let chat_clone = chat.clone();
    let task_owned = task.to_string();
    let display_task_owned = display_task.map(|s| s.to_string());
    let workdir_owned = workdir.clone();
    let agent_name_owned = agent_name.clone();

    let handle = tokio::spawn(async move {
        match engine_clone
            .run_task(
                &chat_clone,
                &task_owned,
                display_task_owned.as_deref(),
                &agent_name_owned,
                workdir_owned,
                None,
                None,
            )
            .await
        {
            Ok(_) => {
                // We assume success if Ok
                // do NOT send TASK_COMPLETE here.
                // engine.rs handles "Plan Generated" notification for Planning phase.
                // For Execution phase, we use start.rs.
            }
            Err(e) => {
                let _ = chat_clone
                    .send_notification(&crate::strings::messages::task_failed(&e.to_string()))
                    .await;
            }
        }
    });

    // Store Handle in RoomState
    {
        let mut guard = state.lock().await;
        let room = guard.get_room_state(&chat.room_id());
        room.task_handle = Some(Arc::new(Mutex::new(Some(handle))));
    }

    Ok(())
}
