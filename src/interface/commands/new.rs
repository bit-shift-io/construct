//! # New Project Command
//!
//! Handles the `.new` command.
//! Scaffolds a new project directory (roadmap.md, tasks.md) via MCP.

use crate::domain::config::AppConfig;
use crate::domain::traits::ChatProvider;
use crate::application::project::ProjectManager;
use anyhow::Result;

pub async fn handle_new(
    config: &AppConfig,
    project_manager: &ProjectManager,
    chat: &impl ChatProvider,
    args: &str,
) -> Result<()> {
    // Parse args: .new <project_name> <requirements...>
    let parts: Vec<&str> = args.splitn(2, ' ').collect();
    if parts.len() < 1 {
        let _ = chat.send_notification("Usage: .new <project_name> [requirements]").await;
        return Ok(());
    }
    
    let name = parts[0];
    let _requirements = if parts.len() > 1 { parts[1] } else { "" };
    
    // Create Project
    let parent_dir = config.system.projects_dir.clone().unwrap_or(".".to_string());
    match project_manager.create_project(name, &parent_dir).await {
        Ok(path) => {
            let _ = chat.send_notification(&format!("Project '{}' created at `{}`.", name, path)).await;
            // TODO: Switch context to this project automatically?
        }
        Err(e) => {
            let _ = chat.send_notification(&format!("Failed to create project: {}", e)).await;
        }
    }
    
    Ok(())
}
