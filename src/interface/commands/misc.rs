//! # Miscellaneous Commands
//!
//! Handles `.status`, `.ask`, `.read`, etc.
//! Provides utility functions for state inspection and ad-hoc queries.

use crate::domain::traits::{ChatProvider, LlmProvider};
use crate::application::state::BotState;
use crate::infrastructure::tools::executor::SharedToolExecutor;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::domain::config::AppConfig;
pub async fn handle_status(
    config: &AppConfig,
    state: &Arc<Mutex<BotState>>,
    chat: &impl ChatProvider,
) -> Result<()> {
    let mut guard = state.lock().await;
    let room_state = guard.get_room_state(&chat.room_id());
    
    let project = crate::application::utils::sanitize_path(
        room_state.current_project_path.as_deref().unwrap_or("None"),
        config.system.projects_dir.as_deref()
    );
    let cwd = crate::application::utils::sanitize_path(
        room_state.current_working_dir.as_deref().unwrap_or("Default"),
        config.system.projects_dir.as_deref()
    );
    
    let msg = crate::strings::messages::room_status_msg(
        &project,
        &cwd,
        room_state.active_model.as_deref().unwrap_or("Default"),
        room_state.active_agent.as_deref().unwrap_or("Default")
    );
    
    // Save state if it was created
    guard.save();
    
    chat.send_message(&msg).await.map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

pub async fn handle_ask<C>(
    config: &AppConfig,
    state: &Arc<Mutex<BotState>>,
    tools: SharedToolExecutor, // For context reading
    llm: &Arc<dyn LlmProvider>,
    chat: &C,
    args: &str,
) -> Result<()>
where C: ChatProvider + Clone + Send + Sync + 'static
{
    if args.trim().is_empty() {
        chat.send_notification(crate::strings::messages::ASK_USAGE).await.map_err(|e| anyhow::anyhow!(e))?;
        return Ok(());
    }

    // 1. Resolve Workdir and Feed
    let (workdir, active_task, feed_manager) = {
        let guard = state.lock().await;
        if let Some(r) = guard.rooms.get(&chat.room_id()) {
            (
                r.current_working_dir.clone(),
                r.active_task.clone(),
                r.feed_manager.clone()
            )
        } else {
            (None, None, None)
        }
    };

    let feed_id = {
         let guard = state.lock().await;
         guard.rooms.get(&chat.room_id()).and_then(|r| r.feed_event_id.clone())
    };
    
    let feed = feed_manager.unwrap_or_else(|| Arc::new(Mutex::new(crate::application::feed::FeedManager::new(
        workdir.clone(), 
        config.system.projects_dir.clone(), 
        tools.clone(),
        feed_id
    ))));

    let engine = crate::application::engine::ExecutionEngine::new(
        config.clone(),
        llm.clone(),
        tools.clone(),
        feed.clone(),
        state.clone()
    );

    // 2. Resolve Active Agent
    let agent_name = {
        let guard = state.lock().await;
        guard.rooms.get(&chat.room_id())
            .and_then(|r| r.active_agent.clone())
            .unwrap_or_else(|| "default".to_string())
    };
    
    // 3. Resolve History Path
    // If active task: {wd}/{task}/conversation.md
    // Else: {wd}/conversation.md
    let history_path = if let Some(wd) = &workdir {
         if let Some(task_rel) = &active_task {
             std::path::Path::new(wd).join(task_rel).join("conversation.md")
         } else {
             std::path::Path::new(wd).join("conversation.md")
         }
    } else {
         std::path::PathBuf::from("conversation.md")
    };
    let history_path_str = history_path.to_string_lossy().to_string();

    // 4. Read History
    let mut history_content = String::new();
    {
         let client = tools.lock().await;
         // Try reading
         if let Ok(content) = client.read_file(&history_path_str).await {
              history_content = content;
         }
    }

    // 5. Run Task (Conversational Mode Default, but allow Switching)
    let _task_prompt = args.to_string();
    
    // 5. Run Task Loop
    let mut current_prompt = args.to_string();
    let _task_prompt = args.to_string(); // Keep original if needed or just use args
    let mut current_history = history_content.clone();
    
    // Reset Phase to Conversational initially
    {
        let mut guard = state.lock().await;
        // Only reset if we are NOT already in a task? 
        // User expects .ask to start fresh conversation or continue?
        // Usually .ask implies conversational entry.
        let room = guard.get_room_state(&chat.room_id());
        room.task_phase = crate::application::state::TaskPhase::Conversational;
    }

    loop {
        // Run engine
        // We pass None for override_phase so it uses the RoomState phase (which might have just changed)
        // We pass current_history to seed context.
        let result = engine.run_task(chat, &current_prompt, None, &agent_name, workdir.clone(), None, Some(current_history.clone())).await?;

        // 6. Update History with Turn
        if let Some(response) = &result {
             let new_entry = format!("\n**User**: {}\n\n**Agent**: {}\n", current_prompt, response);
             current_history.push_str(&new_entry);
             
             // Write back
             let client = tools.lock().await;
             let _ = client.write_file(&history_path_str, &current_history).await;
        }

        // Check if we should continue (Mode Switched?)
        let current_phase = {
             let mut guard = state.lock().await;
             guard.get_room_state(&chat.room_id()).task_phase.clone()
        };
        
        tracing::info!("DEBUG: .ask loop check. Result is None? {}. Current Phase: {:?}", result.is_none(), current_phase);

        if result.is_none() {
            // Engine returned None. 
            // Could be Stop Requested OR SwitchMode.
            // If Stop Requested, room state stop flag is already cleared.
            // If SwitchMode, phase is different.
            
            // How do we distinguish? 
            // If SwitchMode happened, engine.rs sets phase.
            // If we are in Planning/Execution, we should probably auto-continue with "Perform Next Step".
            
            if current_phase != crate::application::state::TaskPhase::Conversational {
                 // Auto-continue!
                 // What is the prompt? 
                 // If we just switched, the prompt should probably be "Continue" or empty?
                 // Engine will generate prompt based on phase. 
                 // We can pass empty task prompt?
                 
                 // Check if we switched to a non-Conversational phase
                 if matches!(current_phase, crate::application::state::TaskPhase::Conversational) {
                     // No mode switch happened, so we are purely done with this turn.
                     break;
                 }
                 
                 // SAFETY: If we switched to EXECUTION, we should STOP and ask for confirmation.
                 if matches!(current_phase, crate::application::state::TaskPhase::Execution) {
                     let _ = chat.send_message("⚠️ **Plan Approved (Auto-Stop)**: The agent is ready to execute. Please type `.start` to begin implementation.").await;
                     break;
                 }

                 // Continue loop for Planning/Verification
                 current_prompt = "Proceed with next step.".to_string();
                 tracing::info!("Auto-continuing .ask loop in {:?} phase", current_phase);
                 continue;
            } else {
                 tracing::info!("DEBUG: Result None but phase is Conversational. Breaking loop.");
            }
        } else {
             tracing::info!("DEBUG: Result is Some (Response received). Breaking loop.");
        }
        
        // If we got a response, or if we are in Conversational mode and done, break.
        // Usually Conversational returns Some(response).
        
        break;
    }

    Ok(())
}

pub async fn handle_read(
    state: &Arc<Mutex<BotState>>, // Re-add state param
    tools: SharedToolExecutor,
    chat: &impl ChatProvider,
    args: &str,
) -> Result<()> {
    let path = args.trim();
    if path.is_empty() {
        chat.send_notification(crate::strings::messages::READ_USAGE).await.map_err(|e| anyhow::anyhow!(e))?;
        return Ok(());
    }

    let cwd = {
         let guard = state.lock().await;
         let room_state = guard.rooms.get(&chat.room_id());
         room_state.and_then(|r| r.current_working_dir.clone())
    };

    let resolved_path = if let Some(wd) = cwd {
        if std::path::Path::new(path).is_absolute() {
            path.to_string()
        } else {
             format!("{}/{}", wd, path)
        }
    } else {
        path.to_string()
    };

    let client = tools.lock().await;
    match client.read_file(&resolved_path).await {
        Ok(content) => {
            chat.send_message(&crate::strings::messages::file_read_success(&resolved_path, &content)).await.map_err(|e| anyhow::anyhow!(e))?;
        }
        Err(e) => {
            chat.send_notification(&crate::strings::messages::file_read_failed(&e.to_string())).await.map_err(|e| anyhow::anyhow!(e))?;
        }
    }
    Ok(())
}

/// Attempts to handle a pending approval request.
/// Returns true if an approval was pending and handled, false otherwise.
pub async fn try_handle_approval(
    state: &Arc<Mutex<BotState>>,
    _chat: &impl ChatProvider, // chat not strictly needed if we just trigger channel, seeing as engine handles notification
    approved: bool,
) -> Result<bool> {
    let mut guard = state.lock().await;
    // We use a dummy key if chat isn't passed, but we need room_id. 
    // Wait, signature needs chat to get room_id.
    // (chat param above is correct)
    
    // Actually, passing chat is fine.
    // Let's fix the guard access.
    // We need room_id.
    // The simplified signature above took `_chat` which is generic.
    // We can't access `chat.room_id()` if `_chat` is effectively ignored or typed generic without bounds in this snippet?
    // Ah, impl ChatProvider is fine.
    
    // Re-do body:
    let room_id = _chat.room_id();
    let room = guard.get_room_state(&room_id);
    
    if let Some(wrapper) = &room.pending_approval_tx {
         let mut tx_guard = wrapper.lock().await;
         if let Some(tx) = tx_guard.take() {
             let _ = tx.send(approved);
             // Optional: Clean up room.pending_approval_tx = None? 
             // Ideally yes, but we are holding state lock?
             // Yes, `guard` matches `room`.
             // We can set room.pending_approval_tx = None;
             return Ok(true); 
         }
    }
    // Clean up if empty?
    if room.pending_approval_tx.is_some() {
        room.pending_approval_tx = None;
    }
    Ok(false)
}
