//! # Project Command
//!
//! Handles `.project` and `.list`.
//! Manages the association between a chat room and a project path on the filesystem.

use crate::domain::config::AppConfig;
use crate::domain::traits::ChatProvider;
use crate::application::project::ProjectManager;
use crate::application::state::BotState;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::path::Path;

pub async fn handle_project(
    config: &AppConfig,
    project_manager: &ProjectManager,
    state: &Arc<Mutex<BotState>>,
    chat: &impl ChatProvider,
    args: &str,
) -> Result<()> {
    // 1. Validate Args
    let path_str = args.trim();
    if path_str.is_empty() {
        chat.send_notification("Usage: .project <path>").await.map_err(|e| anyhow::anyhow!(e))?;
        return Ok(());
    }

    // 2. Resolve Path (handle . as current if valid, or relative to strict root?)
    // Constraints: user might give partial path.
    // For now, let's treat it as relative to config.system.projects_dir if not absolute.
    let base_dir = config.system.projects_dir.clone().unwrap_or(".".to_string());
    
    // Simple resolution logic
    let full_path = if path_str.starts_with("/") {
        path_str.to_string()
    } else {
         Path::new(&base_dir).join(path_str).to_string_lossy().to_string()
    };

    // 3. Verify Validity via ProjectManager
    if project_manager.is_valid_project(&full_path).await {
        // 4. Update State
        let mut state_guard = state.lock().await;
        // Ensure room state exists
        let room_state = state_guard.get_room_state(&chat.room_id());
        room_state.current_project_path = Some(full_path.clone());
        state_guard.save();
        
        chat.send_notification(&format!("Active project set to: `{}`", full_path)).await.map_err(|e| anyhow::anyhow!(e))?;
    } else {
        chat.send_notification(&format!("Path `{}` does not appear to be a valid project (missing roadmap.md).", full_path)).await.map_err(|e| anyhow::anyhow!(e))?;
    }

    Ok(())
}

pub async fn handle_list(
    config: &AppConfig,
    _project_manager: &ProjectManager,
    chat: &impl ChatProvider,
) -> Result<()> {
    // List projects in projects_dir
    let base_dir = config.system.projects_dir.clone().unwrap_or(".".to_string());
    // Use MCP via project_manager to list?
    // ProjectManager needs a list method.
    // Creating "list_projects" helper in ProjectManager is cleaner.
    // For now, let's assume direct usage.
    
    // We can't access MCP client directly here efficiently without exposing it from PM.
    // Let's defer to PM.
    
    // Fallback message for now since PM implementation update is separate step.
    chat.send_notification(&format!("Project listing for `{}` not yet implemented in PM.", base_dir)).await.map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}
