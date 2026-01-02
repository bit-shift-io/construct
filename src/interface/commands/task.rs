//! # Task Command
//!
//! Handles the `.task` command.
//! Initializes the `ExecutionEngine` to autonomously perform a complex multi-step task based on user input.

use crate::application::engine::ExecutionEngine;
use crate::domain::traits::ChatProvider;
use anyhow::Result;

pub async fn handle_task(
    engine: &ExecutionEngine,
    chat: &impl ChatProvider,
    task: &str,
    workdir: Option<String>,
) -> Result<()>
{
    // Validate if inside a project?
    if workdir.is_none() {
        let _ = chat.send_notification(crate::strings::messages::NOT_IN_PROJECT).await;
        return Ok(());
    }

    // Run task using Engine
    // Default agent "default" for now
    match engine.run_task(chat, task, "default", workdir).await {
        Ok(_) => {
            let _ = chat.send_notification(crate::strings::messages::TASK_COMPLETE).await;
        }
        Err(e) => {
            let _ = chat.send_notification(&crate::strings::messages::task_failed(&e.to_string())).await;
        }
    }
    
    Ok(())
}
