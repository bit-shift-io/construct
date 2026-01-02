//! # New Project Command
//!
//! Handles the `.new` command.
//! Scaffolds a new project directory (roadmap.md, tasks.md) via MCP.

use crate::application::state::{BotState, WizardStep, WizardMode};
use crate::domain::config::AppConfig;
use crate::domain::traits::ChatProvider;
use crate::application::project::ProjectManager;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn handle_new(
    config: &AppConfig,
    project_manager: &ProjectManager,
    state: &Arc<Mutex<BotState>>,
    chat: &impl ChatProvider,
    args: &str,
) -> Result<()> {
    // Parse args: .new <project_name> <requirements...>
    let parts: Vec<&str> = args.splitn(2, ' ').collect();
    
    // If no args (or empty string), Start Wizard
    if args.trim().is_empty() {
        {
            let mut guard = state.lock().await;
            let room_state = guard.get_room_state(&chat.room_id());
            room_state.wizard.active = true;
            room_state.wizard.mode = WizardMode::Project;
            room_state.wizard.step = Some(WizardStep::ProjectName);
            room_state.wizard.data.clear();
        }
        
        let msg = crate::strings::wizard::format_wizard_step(&WizardStep::ProjectName, &WizardMode::Project, "", &std::collections::HashMap::new());
        let _ = chat.send_message(&msg).await;
        return Ok(());
    }
    
    let name = parts[0];
    let _requirements = if parts.len() > 1 { parts[1] } else { "" };
    
    // Create Project
    let parent_dir = config.system.projects_dir.clone().unwrap_or(".".to_string());
    match project_manager.create_project(name, &parent_dir).await {
        Ok(path) => {
            let _ = chat.send_notification(&crate::strings::messages::project_created_notification(name, &path)).await;
            
            // Switch context
            let mut guard = state.lock().await;
            {
                let room_state = guard.get_room_state(&chat.room_id());
                room_state.current_working_dir = Some(path.clone());
                room_state.current_project_path = Some(path.clone());
            } // Borrow ends
            guard.save();
            
            // Optional: Confirm switch? The notification above is minimal.
            // But we already said "created at path". The user expectation is cd.
            // Maybe add a small note?
            // chat.send_notification("Switched to project directory.").await; 
            // Stick to the minimal notification for now, or append to it?
            // The notification string is: "Project 'name' created at `path`."
            // Implicitly that's where we are now.
        }
        Err(e) => {
            let _ = chat.send_notification(&crate::strings::messages::project_creation_failed(&e.to_string())).await;
        }
    }
    
    Ok(())
}
