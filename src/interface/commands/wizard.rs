//! # Wizard Command Handler
//!
//! Handles interactive wizard steps for creating projects or tasks.
//! Managed by `RoomState`'s `WizardState`.

use crate::application::state::{BotState, WizardStep};
use crate::domain::config::AppConfig;
use crate::domain::traits::ChatProvider;
use crate::application::project::ProjectManager;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn handle_step(
    config: &AppConfig,
    state: &Arc<Mutex<BotState>>,
    project_manager: &ProjectManager,
    chat: &impl ChatProvider,
    message: &str,
) -> Result<()> {
    let mut create_params: Option<String> = None;

    {
        let mut guard = state.lock().await;
        let room_state = guard.get_room_state(&chat.room_id());
        
        // Check if wizard is active (should be if we routed here)
        if !room_state.wizard.active {
            return Ok(());
        }

        let input = message.trim();
        
        // Handle specific commands inside wizard
        if input == ".cancel" {
            room_state.wizard.active = false;
            room_state.wizard.step = None;
            room_state.wizard.data.clear();
            let _ = chat.send_message(crate::strings::messages::WIZARD_CANCELLED).await;
            return Ok(());
        }

        // Process current step
        match room_state.wizard.step.clone() {
            Some(WizardStep::ProjectName) => {
                if input.is_empty() {
                    // Re-prompt
                    let msg = crate::strings::wizard::format_wizard_step(&WizardStep::ProjectName, &room_state.wizard.mode, "", &room_state.wizard.data);
                    let _ = chat.send_message(&msg).await;
                } else {
                    room_state.wizard.data.insert("name".to_string(), input.to_string());
                    room_state.wizard.step = Some(WizardStep::Description);
                    room_state.wizard.buffer.clear(); // Clear buffer for description
                    let msg = crate::strings::wizard::format_wizard_step(&WizardStep::Description, &room_state.wizard.mode, "", &room_state.wizard.data);
                    let _ = chat.send_message(&msg).await;
                }
            }
            Some(WizardStep::Description) => {
                 // Check for completion command
                 if input == ".ok" {
                     // Use buffer as description
                     let description = room_state.wizard.buffer.clone();
                     room_state.wizard.data.insert("description".to_string(), description);
                     
                     // Prepare for creation
                     let name = room_state.wizard.data.get("name").unwrap_or(&"unnamed".to_string()).clone();
                     create_params = Some(name);
                     
                     // Clear active state now
                     room_state.wizard.active = false;
                     room_state.wizard.step = None;
                 } else {
                     // Accumulate
                     if !room_state.wizard.buffer.is_empty() {
                         room_state.wizard.buffer.push('\n');
                     }
                     room_state.wizard.buffer.push_str(input);
                 }
            }
            Some(WizardStep::Confirmation) => {
                // Legacy / Not reachable if we skip from Description
                if input == ".ok" {
                     let name = room_state.wizard.data.get("name").unwrap_or(&"unnamed".to_string()).clone();
                     create_params = Some(name);
                     room_state.wizard.active = false;
                     room_state.wizard.step = None;
                } else {
                    let _ = chat.send_message(crate::strings::messages::PLEASE_CONFIRM_OR_CANCEL).await;
                }
            }
            _ => {
                // Unknown or null state
                 room_state.wizard.active = false;
            }
        }
    } // guard drops here

    // Perform Creation if needed
    if let Some(name) = create_params {
        let parent_dir = config.system.projects_dir.clone().unwrap_or(".".to_string());
        let creation_result = project_manager.create_project(&name, &parent_dir).await;

        // Re-acquire lock to update room state (CWD)
        // Re-acquire lock to update room state (CWD)
        let mut guard = state.lock().await;
        

        {
            let room_state = guard.get_room_state(&chat.room_id());

            match &creation_result {
                 Ok(path) => {
                    let _ = chat.send_message(&crate::strings::messages::wizard_project_created(path)).await;
                    // Set Context
                    room_state.current_working_dir = Some(path.clone());
                    room_state.current_project_path = Some(path.clone());
                }
                Err(e) => {
                     let _ = chat.send_message(&crate::strings::messages::project_creation_failed(&e.to_string())).await;
                }
            }
            
            // Ensure wizard data is fully cleared
            room_state.wizard.data.clear();
            room_state.wizard.buffer.clear();
        } // room_state borrow ends

        // Save state if needed (always save on wizard exit to clear flags)
        guard.save();

    }

    Ok(())
}
