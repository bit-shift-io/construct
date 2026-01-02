use crate::domain::config::AppConfig;
use crate::domain::traits::ChatProvider;
use crate::infrastructure::tools::executor::SharedToolExecutor;
use crate::application::state::BotState;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::path::Path;

pub async fn handle_admin(
    config: &AppConfig,
    state: &Arc<Mutex<BotState>>,
    tools: SharedToolExecutor,
    chat: &impl ChatProvider,
    sender: &str,
    command: &str,
) -> Result<()> {
    // Check Permissions
    let is_admin = config.system.admin.iter().any(|a| a.to_lowercase() == sender.to_lowercase());
    if !is_admin {
        let _ = chat.send_notification(crate::strings::messages::AUTH_DENIED).await;
        return Ok(());
    }
    
    // **Navigation Logic (cd)**
    // We handle `cd` here because it changes *Application State*, not just running a command.
    if command.trim().starts_with("cd ") {
        let target = command.trim()[3..].trim();
        let mut state_guard = state.lock().await;
        let room_id = chat.room_id();
        let room_state = state_guard.get_room_state(&room_id);
        
        let current_cwd = room_state.current_working_dir.clone().unwrap_or_else(|| ".".to_string());
        
        // Resolve path via ToolExecutor validation logic? 
        // We can use the executor to cannonicalize.
        let tool_executor = tools.lock().await;
        let resolved_path = Path::new(&current_cwd).join(target);
        
        match tool_executor.validate_path(&resolved_path) {
            Ok(safe_path) => {
                let path_str = safe_path.to_string_lossy().to_string();
                room_state.current_working_dir = Some(path_str.clone());
                state_guard.save();
                let _ = chat.send_message(&crate::strings::messages::directory_changed_msg(&path_str)).await;
            }
            Err(e) => {
                 let _ = chat.send_notification(&crate::strings::messages::invalid_directory(&e.to_string())).await;
            }
        }
        return Ok(());
    }

    // Determine CWD
    let workdir = {
        let guard = state.lock().await;
        let room_state = guard.rooms.get(&chat.room_id());
        room_state.and_then(|r| r.current_working_dir.clone())
            .or_else(|| config.system.projects_dir.clone())
            .unwrap_or_else(|| ".".to_string())
    };

    // Execute Shell Command via ToolExecutor
    let client = tools.lock().await;
    match client.execute_command(command, Path::new(&workdir)).await {
        Ok(output) => {
            let _ = chat.send_message(&crate::strings::messages::command_output_format(&workdir, command, &output)).await;
        }
        Err(e) => {
            let _ = chat.send_notification(&crate::strings::messages::command_failed(&e.to_string())).await;
        }
    }
    
    Ok(())
}
