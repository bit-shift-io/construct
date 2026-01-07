//! # Execution Engine
//!
//! The core loop that drives the agent's autonomous behavior.
//! It manages the cycle of thinking, acting, and observing, interfacing with the LLM and MCP.

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::path::Path;

use crate::domain::config::AppConfig;
use crate::domain::traits::LlmProvider;
use crate::application::feed::FeedManager;
use crate::infrastructure::tools::executor::SharedToolExecutor;
use crate::domain::traits::ChatProvider; // Keep ChatProvider for run_task method

use crate::application::state::BotState;

#[derive(Clone)]
pub struct ExecutionEngine {
    _config: AppConfig,
    llm: Arc<dyn LlmProvider>,
    tools: SharedToolExecutor,
    feed: Arc<Mutex<FeedManager>>,
    state: Arc<Mutex<BotState>>,
}

impl ExecutionEngine {
    pub fn new(
        config: AppConfig,
        llm: Arc<dyn LlmProvider>,
        tools: SharedToolExecutor,
        feed: Arc<Mutex<FeedManager>>,
        state: Arc<Mutex<BotState>>,
    ) -> Self {
        Self {
            _config: config,
            llm,
            tools,
            feed,
            state,
        }
    }

    /// Primary execution loop
    pub async fn run_task(&self, chat: &impl ChatProvider, task: &str, display_task: Option<&str>, agent_name: &str, working_dir: Option<String>, override_phase: Option<crate::application::state::TaskPhase>, conversation_history: Option<String>) -> Result<Option<String>> {
        // Initialize Feed
        {
            let mut feed = self.feed.lock().await;
            // Use display_task if provided, otherwise task
            let feed_task = display_task.unwrap_or(task).to_string();
            feed.initialize(feed_task);
            
            if matches!(override_phase, Some(crate::application::state::TaskPhase::Conversational)) {
                feed.mode = crate::application::feed::FeedMode::Conversational;
            }

            let _ = feed.update_feed(chat).await;
        }

        let max_steps = 20;
        let mut steps = 0;
        let mut history = String::new();
        // Pre-seed local history with conversation context if provided
        if let Some(ctx) = conversation_history {
            history.push_str(&ctx);
        }

        loop {
            if steps >= max_steps {
                let _ = chat.send_notification("⚠️ Max steps reached.").await;
                break;
            }
            steps += 1;

            // Check for Stop Request & Get Phase
            let task_phase = if let Some(p) = &override_phase {
                p.clone()
            } else {
                let mut guard = self.state.lock().await;
                let room = guard.get_room_state(&chat.room_id());
                if room.stop_requested {
                    room.stop_requested = false; // Reset flag
                    let _ = chat.send_notification("🛑 **Task Stopped by User**").await;
                     // Update Feed to Failed/Stopped
                    {
                        let mut feed = self.feed.lock().await;
                        feed.update_last_entry("Task Stopped".to_string(), false);
                         let _ = feed.update_feed(chat).await;
                    }
                    return Ok(None); // Stopped
                }
                room.task_phase.clone()
            };

            // 1. Build Context
            // Resolve Active Task directory relative to CWD
            let active_task_rel_path = {
                 let mut guard = self.state.lock().await;
                 let room = guard.get_room_state(&chat.room_id());
                 room.active_task.clone()
            };
            
            let (tasks_content, tasks_checklist_content, roadmap_content, architecture_content, progress_content, plan_content) = if let Some(wd) = &working_dir {
                let client = self.tools.lock().await;
                // Specs
                let roadmap = client.read_file(&format!("{}/specs/roadmap.md", wd)).await.unwrap_or_else(|_| "(No roadmap.md)".into());
                let architecture = client.read_file(&format!("{}/specs/architecture.md", wd)).await.unwrap_or_else(|_| "(No architecture.md)".into());
                let progress = client.read_file(&format!("{}/specs/progress.md", wd)).await.unwrap_or_else(|_| "(No progress history yet)".into());
                
                // Active Task Context
                let (request, tasks_checklist, plan) = if let Some(task_rel) = &active_task_rel_path {
                     let request_path = format!("{}/{}/request.md", wd, task_rel);
                     let tasks_path = format!("{}/{}/tasks.md", wd, task_rel);
                     let plan_path = format!("{}/{}/plan.md", wd, task_rel);
                     let r = client.read_file(&request_path).await.unwrap_or_else(|_| "(No request.md)".into());
                     let t = client.read_file(&tasks_path).await.unwrap_or_else(|_| "(No tasks.md)".into());
                     let p = client.read_file(&plan_path).await.unwrap_or_else(|_| "(No plan.md)".into());
                     (r, t, p)
                } else {
                     ("(No active task context)".into(), "(No active task checklist)".into(), "(No active task plan)".into())
                };

                (request, tasks_checklist, roadmap, architecture, progress, plan)
            } else {
                ("(No context)".into(), "(No context)".into(), "(No context)".into(), "(No context)".into(), "(No context)".into(), "(No context)".into())
            };

            let projects_root = {
                let f = self.feed.lock().await;
                f.projects_root()
            };
            
            let cwd_msg = if let Some(wd) = working_dir.as_deref() {
                crate::application::utils::sanitize_path(wd, projects_root.as_deref())
            } else {
                ".".to_string()
            };
            
            // NOTE: 'tasks_content' variable now holds the REQUEST content.
            // 'tasks_checklist_content' holds the TASKS content.
            // 'plan_content' holds the PLAN content.
            // 'roadmap_content' holds the SPEC/ROADMAP content.
            
            // Current Date for contextual awareness in logs
            let current_date = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();

            let prompt = match task_phase {
                crate::application::state::TaskPhase::Planning => {
                    // planning_mode_turn(cwd, roadmap, request, tasks_checklist, plan, architecture, active_task, history)
                    let task_path = active_task_rel_path.as_deref().unwrap_or("tasks/CURRENT");
                    crate::strings::prompts::planning_mode_turn(&cwd_msg, &roadmap_content, &tasks_content, &tasks_checklist_content, &plan_content, &architecture_content, &progress_content, task_path, &history, &current_date)
                },
                crate::application::state::TaskPhase::Execution => {
                    let task_path = active_task_rel_path.as_deref().unwrap_or("tasks/CURRENT");
                    crate::strings::prompts::execution_mode_turn(&cwd_msg, &roadmap_content, &tasks_content, &tasks_checklist_content, &plan_content, &architecture_content, &progress_content, task_path, &history, &current_date)
                },
                crate::application::state::TaskPhase::NewProject => {
                    crate::strings::prompts::new_project_prompt("Project", &tasks_content, &cwd_msg, &current_date)
                },
                crate::application::state::TaskPhase::Conversational => {
                    crate::strings::prompts::conversational_mode_turn(&cwd_msg, &roadmap_content, &tasks_content, &plan_content, &history)
                }
            };
            
            // Allow history to grow?
            let full_prompt = format!("History:\n{}\n\nUser Question/Task: {}\n\n{}", 
                history, task, prompt);
            
            // DEBUG: Log the full prompt to verify formatting
            tracing::info!("DEBUG COMPOSITE PROMPT:\n{}", full_prompt);

            // 2. LLM Completion
            let _ = chat.typing(true).await;
            
            // Pass agent_name directly to LlmProvider (which routes via Client)
            let start = std::time::Instant::now();
            let response = match self.llm.completion(&full_prompt, agent_name).await {
                Ok(r) => {
                    let duration = start.elapsed();
                    tracing::info!("[PERF] LLM Request took {}ms for prompt length {}", duration.as_millis(), full_prompt.len());
                    tracing::info!("DEBUG RAW LLM RESPONSE:\n{}", r);
                    r
                },
                Err(e) => {
                    let _ = chat.send_notification(&format!("LLM Error: {}", e)).await;
                    break;
                }
            };
            let _ = chat.typing(false).await;

            // 3. Parse Actions
            history.push_str(&format!("\n\nAgent: {}\n", response));
            let actions = crate::application::parsing::parse_actions(&response);

            if actions.is_empty() {
                // Conversational response
                match task_phase {
                    crate::application::state::TaskPhase::Conversational => {
                        {
                            let mut feed = self.feed.lock().await;
                            feed.set_completion(response.clone());
                            let _ = feed.update_feed(chat).await;
                        }
                        return Ok(Some(response));
                    }
                    _ => {
                         let _ = chat.send_message(&response).await;
                    }
                }

                // Wait for user reply? Or stop? 
                // For "Task" execution, we usually expect actions.
                // If it's just talking, we can consider the loop "paused" or "waiting for user".
                // But this run_task is a blocking loop. 
                // We'll break for now to release control.
                break;
            }

            // 4. Execute Actions
            for action in actions {
                // Poll for Stop Request between actions
                {
                    let mut guard = self.state.lock().await;
                    let room = guard.get_room_state(&chat.room_id());
                    if room.stop_requested {
                        room.stop_requested = false;
                        let _ = chat.send_notification("🛑 **Task Stopped by User (Interrupted)**").await;
                        {
                            let mut feed = self.feed.lock().await;
                            feed.update_last_entry("Task Stopped".to_string(), false);
                            let _ = feed.update_feed(chat).await;
                        }
                        return Ok(None);
                    }
                }

                match action {
                    crate::domain::types::AgentAction::Done => {
                        // Only squash if we are truly done (Execution Phase)
                        // Or if we want to signal "Phase Complete" in feed?
                        // User dislikes split feed.
                        
                        match task_phase {
                            crate::application::state::TaskPhase::Planning | crate::application::state::TaskPhase::NewProject => {
                                // Transition to Plan Review Mode
                                // We need to re-read the files to get the LATEST content generated by the agent.
                                let active_task_rel_path = {
                                     let mut guard = self.state.lock().await;
                                     guard.get_room_state(&chat.room_id()).active_task.clone()
                                };
                                
                                let (roadmap, architecture, plan) = if let Some(wd) = &working_dir {
                                    let client = self.tools.lock().await;
                                    let r = client.read_file(&format!("{}/specs/roadmap.md", wd)).await.unwrap_or_else(|_| "(No roadmap.md)".into());
                                    let a = client.read_file(&format!("{}/specs/architecture.md", wd)).await.unwrap_or_else(|_| "(No architecture.md)".into());
                                    // features removed
                                    
                                    let p = if let Some(task_rel) = &active_task_rel_path {
                                         let plan_path = format!("{}/{}/plan.md", wd, task_rel);
                                         client.read_file(&plan_path).await.unwrap_or_else(|_| "(No plan.md)".into())
                                    } else {
                                         "(No active task plan)".into()
                                    };
                                    (r, a, p)
                                } else {
                                    ("(No roadmap)".into(), "(No architecture)".into(), "(No plan)".into())
                                };

                                {
                                     let mut feed = self.feed.lock().await;
                                     // We want to keep the "Written X" logs visible.
                                     // We can just squash it to finalize the "Active" state into "Squashed" (Summary).
                                     // But squashing usually hides the recent activity log?
                                     // Let's check format_squashed. If it hides logs, we might just leave it Active?
                                     // Or implement a "Planning Complete" log.
                                     feed.add_activity("Planning Complete".to_string());
                                     let _ = feed.update_feed(chat).await;
                                }

                                // Don't spam chat with full docs.
                                // The user can read files if needed, or we rely on the Plan summary.
                                
                                // If New Project OR refining the initial plan (001-init), show full roadmap and architecture
                                let is_init_task = active_task_rel_path.as_deref().map(|s| s.contains("001-init")).unwrap_or(false);
                                
                                if matches!(task_phase, crate::application::state::TaskPhase::NewProject) || is_init_task {
                                    let _ = chat.send_message(&architecture).await;
                                    let _ = chat.send_message(&roadmap).await;
                                    // features removed
                                }

                                // Send Plan + Footer only
                                let _ = chat.send_message(&format!("{}\n\n✅ **Plan Generated**: Type `.start` to proceed or `.ask` to refine.", plan)).await;

                                return Ok(Some("Planning Completed. Plan available for review.".to_string()));
                            },
                            crate::application::state::TaskPhase::Execution | crate::application::state::TaskPhase::Conversational => {
                                {
                                    let mut feed = self.feed.lock().await;
                                    feed.process_action(&crate::domain::types::AgentAction::Done).await; // This squashes
                                    let _ = feed.update_feed(chat).await;
                                }
                                return Ok(Some("Task Completed.".to_string())); 
                            }
                        }
                    }
                    crate::domain::types::AgentAction::ListDir(path) => {
                         let projects_root = {
                                let f = self.feed.lock().await;
                                f.projects_root()
                         };
                         let sanitized = crate::application::utils::sanitize_path(&path, projects_root.as_deref());

                         {
                             let mut feed = self.feed.lock().await;
                             feed.add_activity(format!("Listing dir `{}`", sanitized));
                             let _ = feed.update_feed(chat).await;
                         }

                         let client = self.tools.lock().await;
                         // Resolve path
                         let resolved_path = if Path::new(&path).is_absolute() {
                             // Smart Resolution:
                             // If path matches projects_root prefix, use it.
                             // If not, check if prepending projects_root works (Sandbox View).
                             if let Some(root) = projects_root.as_deref() {
                                 if path.starts_with(root) {
                                     path.clone()
                                 } else {
                                     // Try treating it as relative to root
                                     let stripped = path.trim_start_matches('/');
                                     let candidate = format!("{}/{}", root.trim_end_matches('/'), stripped);
                                     tracing::info!("Sandbox Resolution: Mapped virtual path '{}' to real path '{}'", path, candidate);
                                     candidate
                                 }
                             } else {
                                 path.clone()
                             }
                         } else {
                             if let Some(wd) = &working_dir {
                                 format!("{}/{}", wd, path)
                             } else {
                                 path.clone()
                             }
                         };

                         let result = client.list_dir(&resolved_path).await;
                         let (out, success) = match result {
                             Ok(listing) => (listing, true),
                             Err(e) => (format!("Error listing directory: {}", e), false),
                         };
                         
                         {
                             let mut feed = self.feed.lock().await;
                             if success {
                                 feed.replace_last_activity(format!("Listed `{}`", sanitized), true);
                             } else {
                                 feed.replace_last_activity(format!("Listed `{}`", sanitized), false);
                             }
                             let _ = feed.update_feed(chat).await;
                         }
                         
                         history.push_str(&format!("\nSystem: {}\n", out));
                    }
                    crate::domain::types::AgentAction::WriteFile(path, content) => {
                         // SAFETY CHECK: Enforce Planning constraints
                         // If in Planning phase, ONLY allow .md (or .txt/yaml/json?) files.
                         // Strictly forbid .rs, .py, etc.
                         if task_phase == crate::application::state::TaskPhase::Planning || task_phase == crate::application::state::TaskPhase::NewProject {
                             if !path.ends_with(".md") && !path.ends_with(".txt") && !path.ends_with(".yaml") && !path.ends_with(".json") {
                                 let err_msg = format!("PERMISSION DENIED: You are in the PLANNING phase. You cannot write code files (`{}`) yet. You can only write documentation (.md). If you have finished the plan, output `NO_MORE_STEPS`.", path);
                                 history.push_str(&format!("\nSystem: {}\n", err_msg));
                                 
                                 // Update feed to show the rejection?
                                 {
                                     let mut feed = self.feed.lock().await;
                                     feed.add_activity(format!("⚠️ Blocked write to `{}` (Planning Only)", path));
                                     let _ = feed.update_feed(chat).await;
                                 }
                                 continue;
                             }
                         }

                         let projects_root = {
                                let f = self.feed.lock().await;
                                f.projects_root()
                         };
                         let sanitized = crate::application::utils::sanitize_path(&path, projects_root.as_deref());

                         {
                             let mut feed = self.feed.lock().await;
                             feed.add_activity(format!("Writing file `{}`", sanitized));
                             let _ = feed.update_feed(chat).await;
                         }

                         let client = self.tools.lock().await;
                         // Resolve path against working_dir
                         let resolved_path = if Path::new(&path).is_absolute() {
                             if let Some(root) = projects_root.as_deref() {
                                 if path.starts_with(root) {
                                     path.clone()
                                 } else {
                                     let stripped = path.trim_start_matches('/');
                                     format!("{}/{}", root.trim_end_matches('/'), stripped)
                                 }
                             } else {
                                 path.clone()
                             }
                         } else {
                             if let Some(wd) = &working_dir {
                                 format!("{}/{}", wd, path)
                             } else {
                                 path.clone()
                             }
                         };

                         let result = client.write_file(&resolved_path, &content).await;
                         let (out, success) = match result {
                             Ok(_) => ("File written successfully".to_string(), true),
                             Err(e) => (format!("Error writing file: {}", e), false),
                         };
                         
                         {
                             let mut feed = self.feed.lock().await;
                             if success {
                                 feed.replace_last_activity(format!("Written `{}`", sanitized), true);
                             } else {
                                 feed.replace_last_activity(format!("Failed to write `{}`", sanitized), false);
                                 // Keep error details for failure
                                 feed.update_last_entry(out.clone(), false);
                             }
                             let _ = feed.update_feed(chat).await;
                         }
                         history.push_str(&format!("\nOutput: {}\n", out));
                    }
                    crate::domain::types::AgentAction::ReadFile(path) => {
                         {
                             let mut feed = self.feed.lock().await;
                             feed.add_activity(format!("Reading file `{}`", path));
                             let _ = feed.update_feed(chat).await;
                         }
                         let client = self.tools.lock().await;
                         
                         let projects_root = {
                                let f = self.feed.lock().await;
                                f.projects_root()
                         };

                         let resolved_path = if Path::new(&path).is_absolute() {
                             if let Some(root) = projects_root.as_deref() {
                                 if path.starts_with(root) {
                                     path.clone()
                                 } else {
                                     let stripped = path.trim_start_matches('/');
                                     format!("{}/{}", root.trim_end_matches('/'), stripped)
                                 }
                             } else {
                                 path.clone()
                             }
                         } else {
                             if let Some(wd) = &working_dir {
                                 format!("{}/{}", wd, path)
                             } else {
                                 path.clone()
                             }
                         };
                         
                         let result = client.read_file(&resolved_path).await;
                         let (out, success) = match result {
                             Ok(c) => (c, true),
                             Err(e) => (format!("Error reading file: {}", e), false),
                         };
                         {
                             let mut feed = self.feed.lock().await;
                             if success {
                                 // Don't show full content in feed, maybe irrelevant
                                 feed.update_last_entry(format!("Read {} bytes", out.len()), true);
                             } else {
                                 feed.update_last_entry(out.clone(), false);
                             }
                             let _ = feed.update_feed(chat).await;
                         }
                         history.push_str(&format!("\nOutput:\n{}\n", out));
                    }
                    crate::domain::types::AgentAction::ShellCommand(cmd) => {
                        // SAFETY CHECK: Enforce Planning constraints
                        // ABSOLUTELY NO COMMANDS in Planning/New Project phase.
                        if task_phase == crate::application::state::TaskPhase::Planning || task_phase == crate::application::state::TaskPhase::NewProject {
                             let err_msg = format!("PERMISSION DENIED: You are in the PLANNING phase. You cannot run commands (`{}`) yet. You are strictly limited to documentation. Output `NO_MORE_STEPS` if you are done.", cmd);
                             history.push_str(&format!("\nSystem: {}\n", err_msg));
                             
                             // Silent Rejection in Feed
                             continue;
                        }

                        // Update Feed (Only if safe/allowed phase)
                        {
                            let mut feed = self.feed.lock().await;
                            feed.process_action(&crate::domain::types::AgentAction::ShellCommand(cmd.clone())).await;
                            let _ = feed.update_feed(chat).await;
                        }

                        // Safety Check
                        let projects_root = {
                             let f = self.feed.lock().await;
                             f.projects_root()
                        };
                        
                        // If checking safety fails, ask for permission
                        if !crate::application::utils::check_command_safety(&cmd, projects_root.as_deref()) {
                             let (tx, rx) = tokio::sync::oneshot::channel();
                             
                             {
                                  let mut guard = self.state.lock().await;
                                  let room = guard.get_room_state(&chat.room_id());
                                  room.pending_approval_tx = Some(Arc::new(Mutex::new(Some(tx))));
                             }
                             
                             let _ = chat.send_notification(&format!("⚠️ **Security Alert**: Command `{}` uses absolute path outside project root.\nReply `.approve` to allow, `.deny` to skip.", cmd)).await;
                             
                             // Wait for approval
                             match rx.await {
                                 Ok(true) => {
                                      let _ = chat.send_notification("✅ Command Approved.").await;
                                 },
                                 Ok(false) | Err(_) => {
                                      let _ = chat.send_message("🚫 Command Denied or Cancelled.").await;
                                      history.push_str(&format!("\nAction Skipped: Command `{}` denied by user.\n", cmd));
                                      
                                      // Update feed to show skipped
                                      {
                                          let mut feed = self.feed.lock().await;
                                          feed.update_last_entry("Command Denied".to_string(), false);
                                          let _ = feed.update_feed(chat).await;
                                      }
                                      continue;
                                 }
                             }
                        }

                        // Execute via ToolExecutor
                        // We use a simplified direct execution for now, assuming ToolExecutor handles safety/timeouts logic
                        let client = self.tools.lock().await;
                        let output = client.execute_command(&cmd, Path::new(working_dir.as_deref().unwrap_or("."))).await;
                        
                        let (out_str, success) = match output {
                            Ok(o) => (o, true), // We need to check if output contains error codes? 
                            Err(e) => (format!("Error: {}", e), false),
                        };
                        
                        let refined_success = success && !out_str.contains("[Exit Code:") && !out_str.contains("Failed:");

                        // Update Feed Result
                        {
                            let mut feed = self.feed.lock().await;
                            feed.update_last_entry(out_str.clone(), refined_success);
                            let _ = feed.update_feed(chat).await;
                        }

                         history.push_str(&format!("\nOutput:\n{}\n", out_str));
                    }
                     crate::domain::types::AgentAction::SwitchMode(phase) => {
                          tracing::info!("DEBUG: SwitchMode action triggered with phase raw: '{}'", phase);
                          let new_phase = match phase.to_lowercase().as_str() {
                              "planning" | "architect" => crate::application::state::TaskPhase::Planning,
                              "execution" | "developer" => crate::application::state::TaskPhase::Execution,
                              "conversational" => crate::application::state::TaskPhase::Conversational,
                              _ => {
                                  tracing::warn!("DEBUG: Invalid SwitchMode phase: '{}'", phase);
                                  history.push_str(&format!("\nSystem: Invalid mode '{}'. Use 'planning' or 'execution'.\n", phase));
                                  continue;
                              }
                          };
                          tracing::info!("DEBUG: SwitchMode resolved to phase: {:?}", new_phase);
                          
                          // If New Project, show full roadmap and architecture BEFORE switching
                          if matches!(task_phase, crate::application::state::TaskPhase::NewProject) {
                                let (roadmap, architecture, plan) = if let Some(wd) = &working_dir {
                                    let client = self.tools.lock().await;
                                    let r = client.read_file(&format!("{}/specs/roadmap.md", wd)).await.unwrap_or_else(|_| "(No roadmap.md)".into());
                                    let a = client.read_file(&format!("{}/specs/architecture.md", wd)).await.unwrap_or_else(|_| "(No architecture.md)".into());
                                    
                                    let p = if let Some(task_rel) = &active_task_rel_path {
                                         let plan_path = format!("{}/{}/plan.md", wd, task_rel);
                                         client.read_file(&plan_path).await.unwrap_or_else(|_| "(No plan.md)".into())
                                    } else {
                                         "(No active task plan)".into()
                                    };
                                    (r, a, p)
                                } else {
                                    ("(No roadmap)".into(), "(No architecture)".into(), "(No plan)".into())
                                };

                                let _ = chat.send_message(&architecture).await;
                                let _ = chat.send_message(&roadmap).await;
                                // features removed
                                let _ = chat.send_message(&format!("{}\n", plan)).await;
                          }

                          {
                              let mut guard = self.state.lock().await;
                              let room = guard.get_room_state(&chat.room_id());
                              room.task_phase = new_phase.clone();
                          }

                          // Notification removed to reduce feed noise
                          // let _ = chat.send_notification(&format!("🔄 **Switching to {:?} Mode**", new_phase)).await;
                          
                          // Break action loop to re-prompt with new phase immediately
                          break; 
                     }
                }
            }
        }
        
        Ok(None) // Loop finished
    }
}
