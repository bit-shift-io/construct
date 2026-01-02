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

pub async fn handle_status(
    state: &Arc<Mutex<BotState>>,
    chat: &impl ChatProvider,
) -> Result<()> {
    let state_guard = state.lock().await;
    let room_state = state_guard.rooms.get(&chat.room_id());
    
    if let Some(s) = room_state {
        let msg = crate::strings::messages::room_status_msg(
            s.current_project_path.as_deref().unwrap_or("None"),
            s.current_working_dir.as_deref().unwrap_or("Default"),
            s.active_model.as_deref().unwrap_or("Default"),
            s.active_agent.as_deref().unwrap_or("Default")
        );
        chat.send_message(&msg).await.map_err(|e| anyhow::anyhow!(e))?;
    } else {
        chat.send_message(crate::strings::messages::NO_ACTIVE_STATE).await.map_err(|e| anyhow::anyhow!(e))?;
    }
    Ok(())
}

pub async fn handle_ask(
    state: &Arc<Mutex<BotState>>,
    tools: SharedToolExecutor, // For context reading
    llm: &Arc<dyn LlmProvider>,
    chat: &impl ChatProvider,
    args: &str,
) -> Result<()> {
    if args.trim().is_empty() {
        chat.send_notification(crate::strings::messages::ASK_USAGE).await.map_err(|e| anyhow::anyhow!(e))?;
        return Ok(());
    }

    // 1. Gather Context
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
