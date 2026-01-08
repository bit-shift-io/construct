//! # Start Command
//!
//! Handles the `.start` command.
//! This command transitions the agent from Planning phase to Execution phase
//! and resumes the execution loop.

use crate::application::engine::ExecutionEngine;
use crate::application::state::{BotState, TaskPhase};
use crate::domain::traits::ChatProvider;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn handle_start<C>(
    state: &Arc<Mutex<BotState>>,
    engine: &ExecutionEngine,
    chat: &C,
    workdir: Option<String>,
) -> Result<()>
where
    C: ChatProvider + Clone + Send + Sync + 'static,
{
    // Check if we are in Planning phase
    let (active_task, agent_name, phase) = {
        let mut guard = state.lock().await;
        let room = guard.get_room_state(&chat.room_id());

        // If task is already completed or not active?
        // We assume valid context if we are "paused".
        // But what if there is no task?
        if room.task_phase != TaskPhase::Planning {
            // If already in Execution, tell them it's running? Or do nothing?
            // If in Execution and loop is dead (e.g. crashed?), we might want to restart?
            // For now, assume it's only valid if in Planning.
            // UNLESS the user stopped it manually and wants to resume?
            // My plan said: "If task_phase is Planning, switch to Execution."
        }

        (
            room.active_task.clone(),
            room.active_agent.clone(),
            room.task_phase.clone(),
        )
    };

    if phase == TaskPhase::Planning || phase == TaskPhase::NewProject {
        // Transition to Execution
        {
            let mut guard = state.lock().await;
            let room = guard.get_room_state(&chat.room_id());
            room.task_phase = TaskPhase::Execution;
            room.stop_requested = false; // Ensure cleared
        }

        let _ = chat
            .send_notification("🚀 **Starting Execution Phase**")
            .await;

        // Resume Engine
        // Convert options to strings safely
        let task_str = active_task.unwrap_or_else(|| "Resume Task".to_string());
        let agent_str = agent_name.unwrap_or_else(|| "default".to_string());

        let engine_clone = engine.clone();
        let chat_clone = chat.clone();
        let workdir_owned = workdir.clone();

        // Spawn new task loop
        let handle = tokio::spawn(async move {
            match engine_clone
                .run_task(
                    &chat_clone,
                    &task_str,
                    None,
                    &agent_str,
                    workdir_owned,
                    None,
                    None,
                )
                .await
            {
                Ok(_) => {
                    // Task loop finished. 
                    // Feed handles "Task Complete" display.
                }
                Err(e) => {
                    let _ = chat_clone
                        .send_notification(&crate::strings::messages::task_failed(&e.to_string()))
                        .await;
                }
            }
        });

        // Update handle in state
        {
            let mut guard = state.lock().await;
            let room = guard.get_room_state(&chat.room_id());
            room.task_handle = Some(Arc::new(Mutex::new(Some(handle))));
        }
    } else {
        let _ = chat
            .send_notification("ℹ️ Task is already valid or not in Planning/NewProject phase.")
            .await;
    }

    Ok(())
}
