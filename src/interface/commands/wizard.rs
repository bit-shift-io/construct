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
pub enum WizardAction {
    Continue,
    TransitionToTask {
        prompt: String,
        workdir: String,
    },
}

pub async fn handle_step(
    config: &AppConfig,
    state: &Arc<Mutex<BotState>>,
    project_manager: &ProjectManager,
    chat: &impl ChatProvider,
    message: &str,
) -> Result<WizardAction> {
    let mut create_params: Option<String> = None;
    let mut handover_context: Option<(String, String)> = None; // (prompt, workdir)

    {
        let mut guard = state.lock().await;
        let room_state = guard.get_room_state(&chat.room_id());
        
        // Check if wizard is active
        if !room_state.wizard.active {
            return Ok(WizardAction::Continue);
        }

        // Initialize Feed if missing (First Step)
        if room_state.feed_manager.is_none() {
            let _working_dir = room_state.current_working_dir.clone().unwrap_or_else(|| ".".to_string());
            // We need tools here? Wizard doesn't use tools during questions, but Engine does.
            // Router has tools. Can't easy create FeedManager here without tools. 
            // Wait, FeedManager needs SharedToolExecutor. handle_step doesn't have tools passed in.
            // I need to update handle_step signature to take tools.
            // Temporarily, let's assume router provides the feed or we update signature.
            // For now, I'll update signature in next step.
        }

        let input = message.trim();
        
        // Helper to update feed (ignoring missing feed for now, but should handle)
        let feed_manager = room_state.feed_manager.clone();

        // Handle Cancel
        if input == ".cancel" {
            room_state.wizard.active = false;
            room_state.wizard.step = None;
            room_state.wizard.data.clear();
            
             if let Some(feed) = &room_state.feed_manager {
                let mut f = feed.lock().await;
                f.clean_stack();
                f.add_activity("❌ Wizard Cancelled".to_string());
                f.update_feed(chat).await?;
            } else {
                 let _ = chat.send_message(crate::strings::messages::WIZARD_CANCELLED).await;
            }
            return Ok(WizardAction::Continue);
        }

         // Process current step
        match room_state.wizard.step.clone() {
            Some(WizardStep::ProjectName) => {
                if !input.is_empty() {
                    room_state.wizard.data.insert("name".to_string(), input.to_string());
                    room_state.wizard.step = Some(WizardStep::Description);
                    room_state.wizard.buffer.clear();
                    
                    if let Some(feed) = &feed_manager {
                        let mut f = feed.lock().await;
                        f.clean_stack();
                        f.add_checkpoint("Name".to_string(), input.to_string());
                        f.add_prompt("Describe your project. (`.ok` to finish).".to_string());
                        f.update_feed(chat).await?;
                    }
                }
            }
            Some(WizardStep::TaskDescription) => {
                 // Check for completion/multi-line
                 if input == ".ok" {
                     let description = room_state.wizard.buffer.clone();
                     room_state.wizard.active = false;
                     room_state.wizard.step = None;
                     
                     if let Some(feed) = &feed_manager {
                        let mut f = feed.lock().await;
                        f.clean_stack();
                        f.add_checkpoint("Description".to_string(), description.clone());
                        f.add_activity("Generating Plan & Tasks...".to_string());
                        f.update_feed(chat).await?;
                     }

                     // Trigger Task!
                     // Use current working directory
                     let workdir = room_state.current_working_dir.clone().unwrap_or_else(|| ".".to_string());
                     
                     handover_context = Some((description, workdir));

                 } else {
                     if !room_state.wizard.buffer.is_empty() {
                         room_state.wizard.buffer.push('\n');
                     }
                     room_state.wizard.buffer.push_str(input);
                 }
            }
            Some(WizardStep::Description) => {
                 if input == ".ok" {
                     let description = room_state.wizard.buffer.clone();
                     room_state.wizard.data.insert("description".to_string(), description.clone());
                     
                     let name = room_state.wizard.data.get("name").unwrap_or(&"unnamed".to_string()).clone();
                     create_params = Some(name);
                     
                     room_state.wizard.active = false; // Wizard Logic Done
                     room_state.wizard.step = None;
                     
                     if let Some(feed) = &feed_manager {
                        let mut f = feed.lock().await;
                        f.clean_stack();
                        f.add_checkpoint("Description".to_string(), description);
                        f.add_activity("Creating Project Structure..".to_string());
                        f.update_feed(chat).await?;
                     }
                 } else {
                     if !room_state.wizard.buffer.is_empty() {
                         room_state.wizard.buffer.push('\n');
                     }
                     room_state.wizard.buffer.push_str(input);
                 }
            }
             _ => {
                 room_state.wizard.active = false;
            }
        }
        


    } // guard drops

    // Creation Logic (Outside main lock)
    if let Some(name) = create_params {
        let parent_dir = config.system.projects_dir.clone().unwrap_or(".".to_string());
        


        let creation_result = project_manager.create_project(&name, &parent_dir).await;

        let mut guard = state.lock().await;
        // Re-get room state
        // (Scope block to allow releasing guard)
        
        let mut _success_path = None;

        {
            let room_state = guard.get_room_state(&chat.room_id());
             if let Some(feed) = &room_state.feed_manager {
                 let mut f = feed.lock().await;
                     match &creation_result {
                         Ok(path) => {
                             f.clean_stack();
                             f.add_checkpoint("Project created".to_string(), path.clone());
                             
                             // Set Context
                         room_state.current_working_dir = Some(path.clone());
                         room_state.current_project_path = Some(path.clone());
                         _success_path = Some(path.clone());
                         
                         // Prepare Handover Prompt
                         let description = room_state.wizard.data.get("description").cloned().unwrap_or_default();
                         let prompt = crate::strings::prompts::new_project_prompt(&name, &description, path);
                         
                         handover_context = Some((prompt, path.clone()));

                     }
                     Err(e) => {
                         f.update_last_entry(format!("Error: {}", e), false);
                     }
                 }
                 f.update_feed(chat).await?;
             }
             
            room_state.wizard.data.clear();
            room_state.wizard.buffer.clear();
        }
        
        guard.save();
    }

    if let Some((prompt, workdir)) = handover_context {
        return Ok(WizardAction::TransitionToTask { prompt, workdir });
    }

    Ok(WizardAction::Continue)
}
