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
        
        // Access Feed
        // (Assuming signature update comes next, I'll write logic assuming `room_state.feed_manager` exists or I can create it)
        // Actually, if I can't create it here (no tools), router should ensure it exists or pass tools.
        // Let's assume Router initializes FeedManager when starting Wizard (via .new).
        
        // Helper to update feed (ignoring missing feed for now, but should handle)
        let mut feed_updates: Vec<(String, String, bool)> = Vec::new(); // (Action, Content, IsSquash/Success)

        // Handle Cancel
        if input == ".cancel" {
            room_state.wizard.active = false;
            room_state.wizard.step = None;
            room_state.wizard.data.clear();
            
             if let Some(feed) = &room_state.feed_manager {
                let mut f = feed.lock().await;
                f.add_entry("Status".to_string(), "❌ Wizard Cancelled".to_string());
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
                    
                    // Feed updates
                    feed_updates.push(("Project Name".to_string(), input.to_string(), true));
                    let _next_msg = crate::strings::wizard::format_wizard_step(&WizardStep::Description, &room_state.wizard.mode, "", &room_state.wizard.data);
                    // Strip the "### 📝 Project Description..." header for feed?
                    // format_wizard_step returns full markdown.
                    // For feed active entry, we just want the question/instructions.
                    feed_updates.push(("Step 2".to_string(), "Describe your project. Type `.ok` to finish.".to_string(), false));
                }
            }
            Some(WizardStep::TaskDescription) => {
                 // Check for completion/multi-line
                 if input == ".ok" {
                     let description = room_state.wizard.buffer.clone();
                     room_state.wizard.active = false;
                     room_state.wizard.step = None;
                     
                     feed_updates.push(("Task".to_string(), "Task Description Captured".to_string(), true));
                     feed_updates.push(("Status".to_string(), "Generating Plan & Tasks...".to_string(), false));

                     // Trigger Task!
                     // Use current working directory
                     let workdir = room_state.current_working_dir.clone().unwrap_or_else(|| ".".to_string());
                     
                     // We need a prompt for "Plan & execute this task"
                     // We can just pass the description as the argument to the agent, similar to .task <args>
                     // The agent's system prompt handles "Generate plan.md / tasks.md".
                     // So prompt = description.
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
                     room_state.wizard.data.insert("description".to_string(), description);
                     
                     let name = room_state.wizard.data.get("name").unwrap_or(&"unnamed".to_string()).clone();
                     create_params = Some(name);
                     
                     room_state.wizard.active = false; // Wizard Logic Done
                     room_state.wizard.step = None;
                     
                     feed_updates.push(("Description".to_string(), "Project Description Captured".to_string(), true));
                     feed_updates.push(("Status".to_string(), "Creating Project Structure...".to_string(), false));
                 } else {
                     if !room_state.wizard.buffer.is_empty() {
                         room_state.wizard.buffer.push('\n');
                     }
                     room_state.wizard.buffer.push_str(input);
                     
                     // Live update active entry
                     // We can't easily do "live update" without modifying the previous active entry.
                     // FeedManager needs 'update_last_entry'
                     // For now, let's just append to buffer and NOT spam chat updates for every line unless 'sticky'.
                     // Ideally we update the feed message to show accumulated buffer.
                     // feed.update_last_entry("Current Description:\n" + buffer)?
                 }
            }
             _ => {
                 room_state.wizard.active = false;
            }
        }
        
        // Apply Feed Updates
        if let Some(feed) = &room_state.feed_manager {
             let mut f = feed.lock().await;
             for (action, content, is_success) in feed_updates {
                 if is_success {
                     f.add_entry(action, content);
                     f.update_last_entry("".to_string(), true); // Mark success
                 } else {
                     f.add_entry(action, content); // Default running
                 }
             }
             // Explicit update
             f.update_feed(chat).await?;
        }

    } // guard drops

    // Creation Logic (Outside main lock)
    if let Some(name) = create_params {
        let parent_dir = config.system.projects_dir.clone().unwrap_or(".".to_string());
        
        // Ensure feed shows creation
        if let Some(feed) = {
             let guard = state.lock().await;
             let room = guard.rooms.get(&chat.room_id());
             room.and_then(|r| r.feed_manager.clone())
        } {
             let mut f = feed.lock().await;
             f.add_entry("System".to_string(), format!("Creating directory: {}/{}", parent_dir, name));
             f.update_feed(chat).await?;
        }

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
                         f.update_last_entry("Done".to_string(), true);
                         f.add_entry("Success".to_string(), format!("Project created at {}", path));
                         
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
