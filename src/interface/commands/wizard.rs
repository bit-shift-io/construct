//! # Wizard Command Handler
//!
//! Handles interactive wizard steps for creating projects or tasks.
//! Managed by `RoomState`'s `WizardState`.

use crate::application::state::{BotState, WizardStep, WizardMode};

use crate::domain::config::AppConfig;
use crate::domain::traits::ChatProvider;
use crate::application::project::ProjectManager;
use crate::infrastructure::llm::Client;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use serde_json;
pub enum WizardAction {
    Continue,
    TransitionToTask {
        prompt: String,
        display_prompt: Option<String>,
        workdir: String,
        create_new_folder: bool,
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

    let mut handover_context: Option<(String, Option<String>, String, bool)> = None; // (prompt, display_prompt, workdir, create_new_folder)
    let mut run_status_update = false;

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
                     
                     handover_context = Some((description, None, workdir, true));

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
            Some(WizardStep::AgentSelection) => {
                let available_str = room_state.wizard.data.get("available_agents").cloned().unwrap_or_default();
                let agents: Vec<&str> = if available_str.is_empty() {
                     // Fallback (shouldn't happen if initialized correctly)
                     vec!["zai", "gemini", "groq", "anthropic", "openai"]
                } else {
                     available_str.split(',').collect()
                };

                if let Ok(idx) = input.parse::<usize>() {
                    if idx > 0 && idx <= agents.len() {
                        let selected_agent = agents[idx - 1];
                        room_state.wizard.data.insert("agent".to_string(), selected_agent.to_string());
                        room_state.wizard.step = Some(WizardStep::ModelSelection);
                        
                        // List Models (Dynamic)
                        let client = Client::new(config.clone());
                        let models = match client.list_models(selected_agent).await {
                            Ok(mut list) => {
                                // Sort by display name (second element)
                                list.sort_by(|a, b| a.1.cmp(&b.1));
                                list
                            }
                            Err(e) => {
                                tracing::error!("Failed to fetch models for {}: {}. Client fallback failed.", selected_agent, e);
                                vec![]
                            }
                        };
                         
                        // Cache/Store active list in room state so we can validate selection next
                         if let Ok(json_str) = serde_json::to_string(&models) {
                             room_state.wizard.data.insert("model_list_cache".to_string(), json_str);
                         }

                        let mut msg = format!("Select a Model for **{}**:\n", selected_agent);
                        for (i, (_id, name)) in models.iter().enumerate() {
                            msg.push_str(&format!("{}. {}\n", i + 1, name));
                        }
                        
                         if let Some(feed) = &feed_manager {
                            let mut f = feed.lock().await;
                            f.clean_stack();
                            f.add_checkpoint("Agent".to_string(), selected_agent.to_string());
                            f.add_prompt(msg);
                            f.update_feed(chat).await?;
                        } else {
                            let _ = chat.send_message(&msg).await;
                        }
                    } else {
                        let _ = chat.send_message("Invalid selection. Please enter a number.").await;
                    }
                } else {
                    let _ = chat.send_message("Please enter a number.").await;
                }
            }
            Some(WizardStep::ModelSelection) => {
                let agent = room_state.wizard.data.get("agent").cloned().unwrap_or_default();
                
                // Use cached list from previous step
                let models: Vec<(String, String)> = if let Some(cache) = room_state.wizard.data.get("model_list_cache") {
                     serde_json::from_str(cache).unwrap_or_default()
                } else {
                    Vec::new()
                };

                 if let Ok(idx) = input.parse::<usize>() {
                    if idx > 0 && idx <= models.len() {
                        let selected_model = models[idx - 1].0.clone(); // Use ID
                        
                        room_state.active_agent = Some(agent.clone());
                        room_state.active_model = Some(selected_model.clone());
                        
                        room_state.wizard.active = false;
                        room_state.wizard.step = None;
                        room_state.wizard.data.clear();
                        
                         if let Some(feed) = &feed_manager {
                            let mut f = feed.lock().await;
                            f.clean_stack();
                            // Just update status (Defer to avoid deadlock)
                            run_status_update = true;
                            f.update_feed(chat).await?;
                        } else {
                             run_status_update = true;
                        }
                    } else {
                         let _ = chat.send_message("Invalid selection. Please enter a number.").await;
                    }
                 } else {
                     let _ = chat.send_message("Please enter a number.").await;
                 }
            }
             _ => {
                 room_state.wizard.active = false;
            }
        }
        
        
    } // guard drops

    // Independent Status Update (Avods Deadlock)
    if run_status_update {
        let _ = crate::interface::commands::misc::handle_status(config, state, chat).await;
    }

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
                             let sanitized_path = crate::application::utils::sanitize_path(&path, config.system.projects_dir.as_deref());
                             f.clean_stack();
                             f.add_checkpoint("Project created".to_string(), sanitized_path.clone());
                             
                             // Set Context
                         room_state.current_working_dir = Some(path.clone());
                         room_state.current_project_path = Some(path.clone());
                         room_state.task_phase = crate::application::state::TaskPhase::NewProject;
                         let description_str = room_state.wizard.data.get("description").cloned().unwrap_or_default();
                         
                         // Create task subfolder: tasks/001-init
                         let task_dir = std::path::Path::new(&path).join("tasks").join("001-init");
                         let _ = std::fs::create_dir_all(&task_dir);
                         
                         // Write request.md
                         if !description_str.is_empty() {
                              let req_content = crate::strings::templates::REQUEST_TEMPLATE.replace("{{OBJECTIVE}}", &description_str);
                              let _ = std::fs::write(task_dir.join("request.md"), req_content);
                         } else {
                              let req_content = crate::strings::templates::REQUEST_TEMPLATE.replace("{{OBJECTIVE}}", "(No description provided)");
                              let _ = std::fs::write(task_dir.join("request.md"), req_content);
                         }
                         
                         // Note: plan.md creation is delegated to Agent via new_project_prompt.
                         
                         // Set active task
                         room_state.active_task = Some("tasks/001-init".to_string());
                         
                         _success_path = Some(path.clone());
                         
                         // Prepare Handover Prompt
                         let description = room_state.wizard.data.get("description").cloned().unwrap_or_default();
                         // We still pass FULL path to prompt context mechanics?
                         // Prompt instructions say: "You are ALREADY inside it ({workdir})"
                         // User wants: "You are ALREADY inside it (/a3)"?
                         // Prompts usually benefit from being clear about real paths if agent needs to know them,
                         // BUT strict sandbox relies on relative paths anyway.
                         // Displaying simplified path in prompt text is fine as long as CWD is set correctly in backend.
                          let current_date = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
                          let prompt = crate::strings::prompts::new_project_prompt(&name, &description, &sanitized_path, &current_date);
                          // "Generating documentation for project 'a4'."
                          let display_prompt = format!("Generating documentation for project '{}'.", name);

                          handover_context = Some((prompt, Some(display_prompt), path.clone(), false));

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

     if let Some((prompt, display_prompt, workdir, create_new_folder)) = handover_context {
         return Ok(WizardAction::TransitionToTask{ prompt, display_prompt, workdir, create_new_folder });
     }

    Ok(WizardAction::Continue)
}

pub async fn start_agent_wizard(
    config: &AppConfig,
    state: &Arc<Mutex<BotState>>,
    chat: &impl ChatProvider,
) -> Result<()> {
    // 1. Set State
    let feed_manager = {
        let mut guard = state.lock().await;
        let room = guard.get_room_state(&chat.room_id());
        room.wizard.active = true;
        room.wizard.mode = WizardMode::AgentConfig;
        room.wizard.step = Some(WizardStep::AgentSelection);
        room.wizard.data.clear();
        room.feed_manager.clone()
    };

    // 2. Initial Prompt
    // Find valid agents for this room
    let room_id = chat.room_id();
    let mut allowed_agents: Vec<String> = Vec::new();

    // Check bridges
    for (_bridge_name, entries) in &config.bridges {
         let mut is_room_bridge = false;
         for entry in entries {
             if let Some(chan) = &entry.channel {
                 if chan == &room_id {
                     is_room_bridge = true;
                 }
             }
         }
         
         if is_room_bridge {
             // Collect agents from this bridge
             for entry in entries {
                 if let Some(agents_list) = &entry.agents {
                     for a in agents_list {
                         allowed_agents.push(a.clone());
                     }
                 }
             }
         }
    }

    // Default if no specific config
    if allowed_agents.is_empty() {
         // Use all available keys
         for key in config.agents.keys() {
             allowed_agents.push(key.clone());
         }
         // Sort for stability
         allowed_agents.sort();
    }
    
    // De-duplicate just in case
    allowed_agents.sort();
    allowed_agents.dedup();
    
    // Store in data for validation in next step
    let allowed_str = allowed_agents.join(",");
    {
         let mut guard = state.lock().await;
         let room = guard.get_room_state(&chat.room_id());
         room.wizard.data.insert("available_agents".to_string(), allowed_str);
    }


    let mut msg = String::from("Select an AI Provider:\n");
    for (i, agent) in allowed_agents.iter().enumerate() {
        msg.push_str(&format!("{}. {}\n", i + 1, agent));
    }


    if let Some(feed) = feed_manager {
        let mut f = feed.lock().await;
        f.clean_stack();
        f.add_prompt(msg);
        f.update_feed(chat).await?;
    } else {
         let _ = chat.send_message(&msg).await;
    }

    Ok(())
}


