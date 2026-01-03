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
    let state_guard = state.lock().await;
    let room_state = state_guard.rooms.get(&chat.room_id());
    
    if let Some(s) = room_state {
        let project = crate::application::utils::sanitize_path(
            s.current_project_path.as_deref().unwrap_or("None"),
            config.system.projects_dir.as_deref()
        );
        let cwd = crate::application::utils::sanitize_path(
            s.current_working_dir.as_deref().unwrap_or("Default"),
            config.system.projects_dir.as_deref()
        );
        
        let msg = crate::strings::messages::room_status_msg(
            &project,
            &cwd,
            s.active_model.as_deref().unwrap_or("Default"),
            s.active_agent.as_deref().unwrap_or("Default")
        );
        chat.send_message(&msg).await.map_err(|e| anyhow::anyhow!(e))?;
    } else {
        chat.send_message(crate::strings::messages::NO_ACTIVE_STATE).await.map_err(|e| anyhow::anyhow!(e))?;
    }
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

    // 0. Check for Planning Phase (Refinement Mode)
    // We only divert to "Refine Plan" if we are consistently in Planning Mode AND have a project.
    let (is_planning, workdir, feed_manager) = {
        let guard = state.lock().await;
        if let Some(r) = guard.rooms.get(&chat.room_id()) {
            (
                r.task_phase == crate::application::state::TaskPhase::Planning || r.task_phase == crate::application::state::TaskPhase::NewProject,
                r.current_working_dir.clone(),
                r.feed_manager.clone()
            )
        } else {
            (false, None, None)
        }
    };

    if is_planning && workdir.is_some() {
        let wd = workdir.unwrap();
        // Check if we assume valid project context (e.g. implementation_plan.md exists?)
        // The user prompted: "Review implementation_plan.md. Type .start to proceed or .ask to refine."
        // So we strictly follow that.
        
        let feed_id = {
             let guard = state.lock().await;
             guard.rooms.get(&chat.room_id()).and_then(|r| r.feed_event_id.clone())
        };
        let feed = feed_manager.unwrap_or_else(|| Arc::new(Mutex::new(crate::application::feed::FeedManager::new(
            Some(wd.clone()), 
            config.system.projects_dir.clone(), 
            tools.clone(),
            feed_id
        ))));
        
        // ensure feed in state? (Is usually already there if we got it from state)
        // If it wasn't there, we made new one.
        
        let engine = crate::application::engine::ExecutionEngine::new(
            config.clone(),
            llm.clone(), // We need a clone of Arc<dyn LlmProvider>
            tools.clone(),
            feed.clone(),
            state.clone()
        );

        // We use handle_task to ensure proper spawning and active_agent handling
        // Pass "Refine the plan based on: <args>" as the task? 
        // Or just the args?
        // If we pass just args, the prompt "Analyze the request... Create or update..." should handle it.
        // It's safer to prepend context maybe? 
        // "Update the plan: <args>"
        let task_prompt = format!("Refine the plan: {}", args);
        
        return crate::interface::commands::task::handle_task(state, &engine, chat, &task_prompt, None, Some(wd)).await;
    }

    // 1. Gather Context (Standard Q&A)
    let mut context = String::new();
    let (model, project_path) = {
        let guard = state.lock().await;
        let rs = guard.rooms.get(&chat.room_id());
        (
            rs.and_then(|r| r.active_model.clone()),
            rs.and_then(|r| r.current_project_path.clone())
        )
    };

    if let Some(path) = project_path {
        // Try reading tasks.md or roadmap.md
        // We use tools via lock
        let client = tools.lock().await;
        if let Ok(content) = client.read_file(&format!("{}/tasks.md", path)).await {
            context.push_str("\n\nCurrent Tasks Context:\n");
            context.push_str(&content);
        } else if let Ok(content) = client.read_file(&format!("{}/roadmap.md", path)).await {
            context.push_str("\n\nRoadmap Context:\n");
            context.push_str(&content);
        }
    }

    // 2. Construct System Prompt
    let system_prompt = format!(
        "You are a helpful coding assistant.\n{}{}", 
        if context.is_empty() { "" } else { "Use the following context to answer:\n" },
        context
    );

    // 2. Construct Prompt (Simple text for now, since LlmProvider is text-completion-based in traits)
    let prompt = format!(
        "{}\n\nUser: {}", 
        system_prompt,
        args
    );

    // 3. Send to LLM
    chat.typing(true).await.map_err(|e| anyhow::anyhow!(e))?;
    
    // Use default model if none selected
    let model_id = model.as_deref().unwrap_or("gemini-1.5-pro-latest"); // Default fallback
    
    // cast Arc<dyn LlmProvider> ???
    // The `llm` param is `&Arc<dyn LlmProvider>`.
    // completion takes `&self`.
    
    let response = llm.completion(&prompt, model_id).await;

    chat.typing(false).await.map_err(|e| anyhow::anyhow!(e))?;

    match response {
        Ok(ans) => {
            chat.send_message(&ans).await.map_err(|e| anyhow::anyhow!(e))?;
        }
        Err(e) => {
             chat.send_notification(&crate::strings::messages::llm_error(&e.to_string())).await.map_err(|e| anyhow::anyhow!(e))?;
        }
    }

    Ok(())
}

pub async fn handle_read(
    tools: SharedToolExecutor,
    chat: &impl ChatProvider,
    args: &str,
) -> Result<()> {
    let path = args.trim();
    if path.is_empty() {
        chat.send_notification(crate::strings::messages::READ_USAGE).await.map_err(|e| anyhow::anyhow!(e))?;
        return Ok(());
    }

    let client = tools.lock().await;
    match client.read_file(path).await {
        Ok(content) => {
            chat.send_message(&crate::strings::messages::file_read_success(path, &content)).await.map_err(|e| anyhow::anyhow!(e))?;
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
