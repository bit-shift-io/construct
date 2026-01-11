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


use crate::domain::config::AppConfig;

pub async fn handle_start<C>(
    config: &AppConfig,
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

        // Notification removed as per user request
        // let _ = chat
        //     .send_notification("🚀 **Starting Execution Phase**")
        //     .await;

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
        // Smart Start Logic: Look for next milestone
        if let Some(wd) = workdir {
             let roadmap_path = std::path::Path::new(&wd).join("specs/roadmap.md");
             if roadmap_path.exists() {
                 let content = std::fs::read_to_string(&roadmap_path).unwrap_or_default();
                 let mut next_task = None;
                 
                 for line in content.lines() {
                     let trimmed = line.trim();
                     if trimmed.starts_with("- [ ]") {
                         // Found unchecked task
                         // Extract text
                         next_task = Some(trimmed[5..].trim().to_string());
                         break;
                     }
                 }

                 if let Some(task_desc) = next_task {
                     // Notification removed as per user request e.g. "🚀 **Found Next Milestone**: ..."
                     
                     // Trigger new task
                     crate::interface::commands::task::handle_task(
                         config,
                         state,
                         engine,
                         chat,
                         &task_desc,
                         None,
                         Some(wd),
                         true // Create new folder
                     ).await?;
                 } else {
                     let _ = chat.send_notification("ℹ️ No pending milestones found in roadmap.md. Use `.task` to create a custom task.").await;
                 }
             } else {
                 let _ = chat.send_notification("ℹ️ No roadmap.md found. Use `.task` to create a custom task.").await;
             }
        } else {
             let _ = chat.send_notification("⚠️ You are not in a valid project directory.").await;
        }
    }

    Ok(())
}
