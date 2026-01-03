//! # Task Command
//!
//! Handles the `.task` command.
//! Initializes the `ExecutionEngine` to autonomously perform a complex multi-step task based on user input.

use crate::application::engine::ExecutionEngine;
use crate::domain::traits::ChatProvider;
use crate::application::state::{BotState, WizardStep, WizardMode};
use crate::infrastructure::tools::executor::SharedToolExecutor;
use crate::application::feed::{FeedManager, FeedMode};
use std::sync::Arc;
use tokio::sync::Mutex;
use anyhow::Result;

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
        room_state.current_working_dir.clone().unwrap_or_else(|| ".".to_string())
    };

    let feed = Arc::new(Mutex::new(FeedManager::new(Some(workdir), config.system.projects_dir.clone(), tools.clone())));
    
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
    state: &Arc<Mutex<BotState>>,
    engine: &ExecutionEngine,
    chat: &C,
    task: &str,
    workdir: Option<String>,
) -> Result<()>
where C: ChatProvider + Clone + Send + Sync + 'static
{
    // Validate if inside a project?
    if workdir.is_none() {
        let _ = chat.send_notification(crate::strings::messages::NOT_IN_PROJECT).await;
        return Ok(());
    }

    // Resolve Active Agent
    let agent_name = {
        let mut guard = state.lock().await;
        let room = guard.get_room_state(&chat.room_id());
        
        // IMPORTANT: Clear any pending stop request from previous sessions
        room.stop_requested = false;

        // Ensure active agent is set (e.g. from wizard or sticky)
        if room.active_agent.is_none() {
             // Fallback to "default" or first available? 
             // Ideally we should have a config default. "default" is the safe hardcoded bet if we assume one exists.
             room.active_agent = Some("default".to_string());
        }
        room.active_agent.clone().unwrap()
    };

    // Run task using Engine
    let engine_clone = engine.clone();
    let chat_clone = chat.clone();
    let task_owned = task.to_string();
    let workdir_owned = workdir.clone();
    let agent_name_owned = agent_name.clone();

    let handle = tokio::spawn(async move {
        match engine_clone.run_task(&chat_clone, &task_owned, &agent_name_owned, workdir_owned).await {
            Ok(completed) => {
                if completed {
                    let _ = chat_clone.send_notification(crate::strings::messages::TASK_COMPLETE).await;
                }
            }
            Err(e) => {
                let _ = chat_clone.send_notification(&crate::strings::messages::task_failed(&e.to_string())).await;
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
