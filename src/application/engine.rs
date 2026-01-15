//! # Execution Engine
//!
//! The core loop that drives the agent's autonomous behavior.
//! It manages the cycle of thinking, acting, and observing, interfacing with the LLM and MCP.

use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::application::feed::FeedManager;
use crate::domain::config::AppConfig;
use crate::domain::traits::ChatProvider;
use crate::domain::traits::LlmProvider;
use crate::infrastructure::tools::executor::SharedToolExecutor; // Keep ChatProvider for run_task method

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
    pub async fn run_task(
        &self,
        chat: &impl ChatProvider,
        task: &str,
        display_task: Option<&str>,
        agent_name: &str,
        working_dir: Option<String>,
        override_phase: Option<crate::application::state::TaskPhase>,
        conversation_history: Option<String>,
    ) -> Result<Option<String>> {
        // Initialize Feed
        {
            let mut feed = self.feed.lock().await;
            // Use display_task if provided, otherwise task
            let feed_task = display_task.unwrap_or(task).to_string();
            feed.initialize(feed_task);

            if matches!(
                override_phase,
                Some(crate::application::state::TaskPhase::Assistant)
            ) {
                feed.mode = crate::application::feed::FeedMode::Assistant;
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
                // If max steps reached, consider the task as potentially incomplete or requiring manual intervention.
                // We don't have a clear "final_msg" here, so we return None.
                let mut feed = self.feed.lock().await;
                feed.add_completion_message("Task reached max steps without explicit completion.".to_string());
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

            // Update Feed Identity based on Phase
            {
                let role_title = match task_phase {
                    crate::application::state::TaskPhase::Planning => "Architect",
                    crate::application::state::TaskPhase::Execution => "Developer",
                    crate::application::state::TaskPhase::Assistant => "Assistant",
                    _ => "Engineer",
                };
                let mut feed = self.feed.lock().await;
                feed.set_agent_name(role_title.to_string());
            }

            // 1. Build Context
            // Resolve Active Task directory relative to CWD
            let active_task_rel_path = {
                let mut guard = self.state.lock().await;
                let room = guard.get_room_state(&chat.room_id());
                room.active_task.clone()
            };

            let (
                tasks_checklist_content,
                roadmap_content,
                architecture_content,
                progress_content,
                plan_content,
                guidelines_content,
            ) = if let Some(wd) = &working_dir {
                let client = self.tools.lock().await;
                // Specs
                let roadmap = client
                    .read_file(&crate::domain::paths::roadmap_path(wd))
                    .await
                    .unwrap_or_else(|_| "(No roadmap.md)".into());
                let architecture = client
                    .read_file(&crate::domain::paths::architecture_path(wd))
                    .await
                    .unwrap_or_else(|_| "(No architecture.md)".into());
                let progress = client
                    .read_file(&crate::domain::paths::progress_path(wd))
                    .await
                    .unwrap_or_else(|_| "(No progress history yet)".into());
                let guidelines = client
                    .read_file(&crate::domain::paths::guidelines_path(wd))
                    .await
                    .unwrap_or_else(|_| "(No guidelines.md)".into());

                // Active Task Context
                let (tasks_checklist, plan) = if let Some(task_rel) = &active_task_rel_path
                {
                    let tasks_path = format!("{}/{}/tasks.md", wd, task_rel);
                    let plan_path = format!("{}/{}/plan.md", wd, task_rel);
                    let t = client
                        .read_file(&tasks_path)
                        .await
                        .unwrap_or_else(|_| "(No tasks.md)".into());
                    let p = client
                        .read_file(&plan_path)
                        .await
                        .unwrap_or_else(|_| "(No plan.md)".into());
                    (t, p)
                } else {
                    (
                        "(No active task checklist)".into(),
                        "(No active task plan)".into(),
                    )
                };

                (
                    tasks_checklist,
                    roadmap,
                    architecture,
                    progress,
                    plan,
                    guidelines,
                )
            } else {
                (
                    "(No context)".into(),
                    "(No context)".into(),
                    "(No context)".into(),
                    "(No context)".into(),
                    "(No context)".into(),
                    "(No context)".into(),
                )
            };

            let projects_root = {
                let f = self.feed.lock().await;
                // Agent name is set based on Phase earlier in the loop
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
                    crate::strings::prompts::planning_mode_turn(
                        &cwd_msg,
                        &roadmap_content,
                        &tasks_checklist_content,
                        &plan_content,
                        &architecture_content,
                        &progress_content,
                        task_path,
                        &history,
                        &current_date,
                        &guidelines_content,
                    )
                }
                crate::application::state::TaskPhase::Execution => {
                    let task_path = active_task_rel_path.as_deref().unwrap_or("tasks/CURRENT");
                    crate::strings::prompts::execution_mode_turn(
                        &cwd_msg,
                        &roadmap_content,
                        &tasks_checklist_content,
                        &plan_content,
                        &architecture_content,
                        &progress_content,
                        task_path,
                        &history,
                        &current_date,
                        &guidelines_content,
                    )
                }
                crate::application::state::TaskPhase::NewProject => {
                    crate::strings::prompts::new_project_prompt(
                        "Project",
                        &tasks_checklist_content,
                        &cwd_msg,
                        &current_date,
                    )
                }
                crate::application::state::TaskPhase::Assistant => {
                    {
                        let mut f = self.feed.lock().await;
                        f.set_agent_name("Assistant".to_string());
                    }
                    crate::strings::prompts::assistant_mode_turn(
                        &cwd_msg,
                        &roadmap_content,
                        &tasks_checklist_content,
                        &plan_content,
                        &architecture_content,
                        &progress_content,
                        &history,
                        &guidelines_content,
                    )
                }
            };

            // Allow history to grow?
            let full_prompt = format!(
                "History:\n{}\n\nUser Question/Task: {}\n\n{}",
                history, task, prompt
            );

            // DEBUG: Log the full prompt to verify formatting
            tracing::info!("DEBUG COMPOSITE PROMPT:\n{}", full_prompt);

            // 2. LLM Completion
            let _ = chat.typing(true).await;

            // Pass agent_name directly to LlmProvider (which routes via Client)
            let start = std::time::Instant::now();
            let response = match self.llm.completion(&full_prompt, agent_name).await {
                Ok(r) => {
                    let duration = start.elapsed();
                    tracing::info!(
                        "[PERF] LLM Request took {}ms for prompt length {}",
                        duration.as_millis(),
                        full_prompt.len()
                    );
                    tracing::info!("DEBUG RAW LLM RESPONSE:\n{}", r);
                    r
                }
                Err(e) => {
                    let _ = chat.send_notification(&format!("LLM Error: {}", e)).await;
                    break;
                }
            };
            let _ = chat.typing(false).await;

            // 3. Parse Actions
            history.push_str(&format!("\n\nAgent: {}\n", response));
            let actions_with_indices = crate::application::parsing::parse_actions(&response);

            // Extract Agent Thought (text before the first code block) for the feed initially
            // This ensures the first thought is shown immediately even before the loop starts
            let thought = response.split("```").next().unwrap_or(&response).trim().to_string();
            if !thought.is_empty() {
                let mut feed = self.feed.lock().await;
                feed.set_agent_thought(thought);
                // We update the feed here so the thought appears immediately
                let _ = feed.update_feed(chat).await;
            }

            if actions_with_indices.is_empty() {
                // Conversational response
                match task_phase {
                    crate::application::state::TaskPhase::Assistant => {
                        {
                            let mut feed = self.feed.lock().await;
                            feed.set_completion(response.clone());
                            let _ = feed.update_feed(chat).await;
                        }
                        return Ok(Some(response));
                    }
                    _ => {
                        // In Execution/Planning, treat raw text as a thought/activity 
                        // instead of dumping it into the chat.
                        {
                            let mut feed = self.feed.lock().await;
                            // Clean up the response (optional, or rely on feed formatter truncating)
                            let cleaned = clean_agent_thought(&response);
                            if !cleaned.is_empty() {
                                feed.set_agent_thought(cleaned);
                                let _ = feed.update_feed(chat).await;
                            }
                        }
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
            let mut last_response_index = 0;
            for (action_ref, start_idx, end_idx) in &actions_with_indices {
                let action = action_ref.clone();
                let start_idx = *start_idx;
                let end_idx = *end_idx;
                // Update Feed with interleaving thought if present
                if start_idx > last_response_index {
                    let pre_text = &response[last_response_index..start_idx];
                    
                    // Clean the thought: 
                    let trimmed = clean_agent_thought(pre_text);

                     // Only update if there is meaningful text (ignore small punctuation or whitespace)
                    if !trimmed.is_empty() && trimmed.len() > 3 { 
                        let mut feed = self.feed.lock().await;
                        feed.set_agent_thought(trimmed.clone());
                        let _ = feed.update_feed(chat).await;
                    }
                }
                last_response_index = end_idx;
                // Poll for Stop Request between actions
                {
                    let mut guard = self.state.lock().await;
                    let room = guard.get_room_state(&chat.room_id());
                    if room.stop_requested {
                        room.stop_requested = false;
                        let _ = chat
                            .send_notification("🛑 **Task Stopped by User (Interrupted)**")
                            .await;
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
                            crate::application::state::TaskPhase::Planning
                            | crate::application::state::TaskPhase::NewProject => {
                                // Transition to Plan Review Mode
                                // We need to re-read the files to get the LATEST content generated by the agent.
                                    // Artifacts reading logic removed (replaced by menu)

                                {
                                    let mut feed = self.feed.lock().await;
                                    // We want to keep the "Written X" logs visible.
                                    // We can just squash it to finalize the "Active" state into "Squashed" (Summary).
                                    // But squashing usually hides the recent activity log?
                                    // Let's check format_squashed. If it hides logs, we might just leave it Active?
                                    // Or implement a "Planning Complete" log.
                                    feed.add_activity("✅ Planning Complete".to_string());
                                    let _ = feed.update_feed(chat).await;
                                }

                                // Don't spam chat with full docs.
                                // The user can read files if needed, or we rely on the Plan summary.

                                // If New Project OR refining the initial plan (001-init), show full roadmap and architecture
                                // Menu replaces logic

                                {
                                    let mut feed = self.feed.lock().await;

                                    // 1. Add "Planning Complete" to the feed history (last item)
                                    // Using the check-mark style requested
                                    feed.add_activity("✅ Planning Complete".to_string());
                                    
                                    // 2. Set the Menu as the completion message (Footer)
                                    let menu = "Open: .1 Architecture | .2 Roadmap | .3 Plan | .4 Tasks";
                                    feed.add_completion_message(menu.to_string());

                                    // START AUTO-CONTINUE TIMER
                                    let now = chrono::Utc::now().timestamp();
                                    let delay_minutes = self._config.system.auto_start_delay_minutes.unwrap_or(30);
                                    let target = now + (delay_minutes as i64 * 60);
                                    
                                    {
                                        let mut guard = self.state.lock().await;
                                        if let Some(room) = guard.rooms.get_mut(&chat.room_id()) {
                                            room.task_completion_time = Some(now);
                                        }
                                        feed.auto_start_timestamp = Some(target);
                                    }
                                    
                                    // 3. Squash and Update
                                    feed.squash();
                                    let _ = feed.update_feed(chat).await;
                                }

                                return Ok(Some(
                                    "Planning Completed. Plan available for review.".to_string(),
                                ));
                            }
                            crate::application::state::TaskPhase::Execution
                            | crate::application::state::TaskPhase::Assistant => {
                                {
                                    let mut feed = self.feed.lock().await;
                                    
                                    // STRIP ACTIONS from the response to avoid dumping code blocks (artifacts) into the feed summary
                                    let mut sorted_actions = actions_with_indices.clone();
                                    // Correct Tuple: (Action, start_index, end_index)
                                    sorted_actions.sort_by_key(|(_, start, _)| *start);

                                    let mut clean_msg = String::new();
                                    let mut last_idx = 0;

                                    for (_, start, end) in sorted_actions {
                                        if start > last_idx {
                                            clean_msg.push_str(&response[last_idx..start]);
                                        }
                                        last_idx = end;
                                    }
                                    if last_idx < response.len() {
                                        clean_msg.push_str(&response[last_idx..]);
                                    }

                                    let final_msg = clean_msg.replace("NO_MORE_STEPS", "").trim().to_string();
                                    
                                    // Only add completion text if we are in Conversational mode, OR if it's a very short specific message.
                                    // For Execution, the "Thinking" block usually covers the intent, and the actions show the result.
                                    // But we want to show the final "Milestone X Complete" message if present.
                                    if !final_msg.is_empty() && final_msg.len() < 1000 {
                                        feed.add_completion_message(final_msg);
                                    }

                                    // START AUTO-CONTINUE TIMER
                                    let now = chrono::Utc::now().timestamp();

                                    let delay_minutes = self._config.system.auto_start_delay_minutes.unwrap_or(30);
                                    let target = now + (delay_minutes as i64 * 60);
                                    
                                    {
                                        let mut guard = self.state.lock().await;
                                        if let Some(room) = guard.rooms.get_mut(&chat.room_id()) {
                                            room.task_completion_time = Some(now);
                                        }
                                        feed.auto_start_timestamp = Some(target);
                                    }

                                    feed.process_action(&crate::domain::types::AgentAction::Done)
                                        .await; // This squashes
                                    let _ = feed.update_feed(chat).await;
                                }
                                return Ok(None);
                            }
                        }
                    }
                    crate::domain::types::AgentAction::ListDir(path) => {
                        let projects_root = {
                            let f = self.feed.lock().await;
                            f.projects_root()
                        };
                        let sanitized = crate::application::utils::sanitize_path(
                            &path,
                            projects_root.as_deref(),
                        );

                        {
                            let mut feed = self.feed.lock().await;
                            feed.add_activity(format!("Listing dir {}", sanitized));
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
                                    let candidate =
                                        format!("{}/{}", root.trim_end_matches('/'), stripped);
                                    tracing::info!(
                                        "Sandbox Resolution: Mapped virtual path '{}' to real path '{}'",
                                        path,
                                        candidate
                                    );
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
                                feed.replace_last_activity(format!("Listed {}", sanitized), true);
                            } else {
                                feed.replace_last_activity(
                                    format!("List {}", sanitized),
                                    false,
                                );
                            }
                            let _ = feed.update_feed(chat).await;
                        }

                        history.push_str(&format!("\nSystem: {}\n", out));
                    }
                    crate::domain::types::AgentAction::Find(path, pattern) => {
                        let projects_root = {
                            let f = self.feed.lock().await;
                            f.projects_root()
                        };
                        let root_to_use = working_dir.as_deref().or(projects_root.as_deref());
                        let sanitized_path = crate::application::utils::sanitize_path(
                            &path,
                            root_to_use,
                        );
                        
                        {
                            let mut feed = self.feed.lock().await;
                            feed.add_activity(format!("Finding {} {}", sanitized_path, pattern));
                            let _ = feed.update_feed(chat).await;
                        }

                        let client = self.tools.lock().await;
                        // Resolve path
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
                        
                        let result = client.find_files(&resolved_path, &pattern).await;
                        let (out, success) = match result {
                            Ok(listing) => (listing, true),
                            Err(e) => (format!("Error finding files: {}", e), false),
                        };

                        {
                            let mut feed = self.feed.lock().await;
                            if success {
                                feed.replace_last_activity(format!("Found {} {}", sanitized_path, pattern), true);
                            } else {
                                feed.replace_last_activity(
                                    format!("Find {} {}", sanitized_path, pattern),
                                    false,
                                );
                            }
                            let _ = feed.update_feed(chat).await;
                        }

                        history.push_str(&format!("\nSystem: {}\n", out));
                    }
                    crate::domain::types::AgentAction::WriteFile(path, content) => {
                        // SAFETY CHECK: Enforce Planning constraints
                        // If in Planning phase, ONLY allow .md (or .txt/yaml/json?) files.
                        // Strictly forbid .rs, .py, etc.
                        if task_phase == crate::application::state::TaskPhase::Planning
                            || task_phase == crate::application::state::TaskPhase::NewProject
                        {
                            if !path.ends_with(".md")
                                && !path.ends_with(".txt")
                                && !path.ends_with(".yaml")
                                && !path.ends_with(".json")
                            {
                                let err_msg = format!(
                                    "PERMISSION DENIED: You are in the PLANNING phase. You cannot write code files (`{}`) yet. You can only write documentation (.md, .txt, .yaml, .json). If you have finished the plan, output `NO_MORE_STEPS`.",
                                    path
                                );
                                history.push_str(&format!("\nSystem: {}\n", err_msg));

                                // Update feed to show the rejection?
                                {
                                    let mut feed = self.feed.lock().await;
                                    feed.add_activity(format!(
                                        "⚠️ Blocked write to {} (Planning Only)",
                                        path
                                    ));
                                    let _ = feed.update_feed(chat).await;
                                }
                                continue;
                            }
                        }

                        let projects_root = {
                            let f = self.feed.lock().await;
                            f.projects_root()
                        };
                        let root_to_use = working_dir.as_deref().or(projects_root.as_deref());
                        let sanitized = crate::application::utils::sanitize_path(
                            &path,
                            root_to_use,
                        );

                        {
                            let mut feed = self.feed.lock().await;
                            feed.add_activity(format!("Writing {}", sanitized));
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
                                feed.replace_last_activity(
                                    format!("Wrote {}", sanitized),
                                    true,
                                );
                            } else {
                                feed.replace_last_activity(
                                    format!("Write {}", sanitized),
                                    false,
                                );
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
                            feed.add_activity(format!("Reading file {}", path));
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
                            
                            // Sanitize for display
                            // Prefer working_dir (current project root) to sanitize redundant project prefixes
                            let root_to_use = working_dir.as_deref().or(projects_root.as_deref());
                            let sanitized = crate::application::utils::sanitize_path(
                                &path, 
                                root_to_use
                            );

                            if success {
                                feed.replace_last_activity(format!("Read {}", sanitized), true);
                                // Don't show full content in feed, maybe irrelevant
                                // Don't show full content or byte count in feed

                            } else {
                                feed.replace_last_activity(format!("Read {}", sanitized), false);
                                feed.update_last_entry(out.clone(), false);
                            }
                            let _ = feed.update_feed(chat).await;
                        }
                        history.push_str(&format!("\nOutput:\n{}\n", out));
                    }
                    crate::domain::types::AgentAction::ShellCommand(cmd) => {
                        // SAFETY CHECK: Enforce Planning constraints
                        // ABSOLUTELY NO COMMANDS in Planning/New Project phase.
                        if task_phase == crate::application::state::TaskPhase::Planning
                            || task_phase == crate::application::state::TaskPhase::NewProject
                        {
                            let err_msg = format!(
                                "PERMISSION DENIED: You are in the PLANNING phase. You cannot run commands (`{}`) yet. You are strictly limited to documentation. Output `NO_MORE_STEPS` if you are done.",
                                cmd
                            );
                            history.push_str(&format!("\nSystem: {}\n", err_msg));

                            // Silent Rejection in Feed
                            continue;
                        }

                        // Update Feed (Only if safe/allowed phase)
                        {
                            let mut feed = self.feed.lock().await;
                            feed.process_action(&crate::domain::types::AgentAction::ShellCommand(
                                cmd.clone(),
                            ))
                            .await;
                            let _ = feed.update_feed(chat).await;
                        }

                        // Safety Check
                        let projects_root = {
                            let f = self.feed.lock().await;
                            f.projects_root()
                        };

                        // If checking safety fails, ask for permission
                        if !crate::application::utils::check_command_safety(
                            &cmd,
                            projects_root.as_deref(),
                        ) {
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
                                }
                                Ok(false) | Err(_) => {
                                    let _ =
                                        chat.send_message("🚫 Command Denied or Cancelled.").await;
                                    history.push_str(&format!(
                                        "\nAction Skipped: Command `{}` denied by user.\n",
                                        cmd
                                    ));

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
                        let output = client
                            .execute_command(&cmd, Path::new(working_dir.as_deref().unwrap_or(".")))
                            .await;

                        let (out_str, success) = match output {
                            Ok(o) => (o, true), // We need to check if output contains error codes?
                            Err(e) => (format!("Error: {}", e), false),
                        };

                        let refined_success = success
                            && !out_str.contains("[Exit Code:")
                            && !out_str.contains("Failed:");

                        // Update Feed Result
                        {
                            let mut feed = self.feed.lock().await;
                            
                            let root_to_use = working_dir.as_deref().or(projects_root.as_deref());
                            let sanitized_cmd = crate::application::utils::sanitize_path(
                                &cmd,
                                root_to_use
                            ).replace('`', "");

                            if refined_success {
                                feed.replace_last_activity(format!("Ran {}", sanitized_cmd), true);
                            } else {
                                // Content is just the command. Label "Failed" will handle the prefix.
                                feed.replace_last_activity(sanitized_cmd, false);
                            }

                            let _ = feed.update_feed(chat).await;
                        }

                        history.push_str(&format!("\nOutput:\n{}\n", out_str));
                    }
                    crate::domain::types::AgentAction::SwitchMode(phase) => {
                        tracing::info!(
                            "DEBUG: SwitchMode action triggered with phase raw: '{}'",
                            phase
                        );
                        let new_phase = match phase.to_lowercase().as_str() {
                            "planning" | "architect" => {
                                crate::application::state::TaskPhase::Planning
                            }
                            "execution" | "developer" => {
                                crate::application::state::TaskPhase::Execution
                            }
                            "conversational" => {
                                crate::application::state::TaskPhase::Assistant
                            }
                            _ => {
                                tracing::warn!("DEBUG: Invalid SwitchMode phase: '{}'", phase);
                                history.push_str(&format!(
                                    "\nSystem: Invalid mode '{}'. Use 'planning' or 'execution'.\n",
                                    phase
                                ));
                                continue;
                            }
                        };
                        tracing::info!("DEBUG: SwitchMode resolved to phase: {:?}", new_phase);

                        // If New Project, show full roadmap and architecture BEFORE switching
                        if matches!(task_phase, crate::application::state::TaskPhase::NewProject) {
                            let (roadmap, architecture, plan) = if let Some(wd) = &working_dir {
                                let client = self.tools.lock().await;
                                let r = client
                                    .read_file(&crate::domain::paths::roadmap_path(wd))
                                    .await
                                    .unwrap_or_else(|_| "(No roadmap.md)".into());
                                let a = client
                                    .read_file(&crate::domain::paths::architecture_path(wd))
                                    .await
                                    .unwrap_or_else(|_| "(No architecture.md)".into());

                                let p = if let Some(task_rel) = &active_task_rel_path {
                                    let plan_path = format!("{}/{}/plan.md", wd, task_rel);
                                    client
                                        .read_file(&plan_path)
                                        .await
                                        .unwrap_or_else(|_| "(No plan.md)".into())
                                } else {
                                    "(No active task plan)".into()
                                };
                                (r, a, p)
                            } else {
                                (
                                    "(No roadmap)".into(),
                                    "(No architecture)".into(),
                                    "(No plan)".into(),
                                )
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

fn clean_agent_thought(text: &str) -> String {


    // 1. Try to find ```thought ... ``` block (Standard Chain-of-Thought)
    if let Some(start) = text.find("```thought") {
        let remainder = &text[start + 10..]; // Skip ```thought
        if let Some(end) = remainder.find("```") {
             return remainder[..end].trim().to_string();
        }
    }

    // 2. Fallback: Take everything BEFORE the first code block (```)
    // This handles cases where the thought is just plain text before the tool call.
    let pre_code = if let Some(idx) = text.find("```") {
        &text[..idx]
    } else {
        text
    };

    // 3. Line Filtering
    // Filter out "Output:", compiler logs, and empty lines.
    let mut lines = Vec::new();
    
    for line in pre_code.lines() {
        let t = line.trim();
        
        // Hallucinated "Output:" blocks handling
        // If we see "Output:" on its own line or starting a line, stop capturing if it looks like tool output
        if t.starts_with("Output:") {
            // Check if what follows resembles tool output (or assume it is)
            // Ideally tool output shouldn't be in the thought section.
            // We'll skip this line and potentially subsequent lines if they look like logs?
            // Safer strategy: Just skip this line.
            continue; 
        }

        // Standard Filter
        if !t.starts_with("Compiling ")
            && !t.starts_with("Finished ")
            && !t.starts_with("Running ")
            && !t.starts_with("Checking ") 
            && !t.starts_with("Creating ") 
            && !t.starts_with("error")     
            && !t.starts_with("warning")   
            && !t.starts_with("|")         
            && !t.starts_with("=")         
            && !t.starts_with("^^")         
            && !t.starts_with("note:")     
            && !t.starts_with("help:")     
            && !t.starts_with("...")
            && !t.is_empty() 
        {
            lines.push(line);
        }
    }
    
    let cleaned = lines.join("\n");
    
    // 4. Post-processing: Remove "thought" or "Agent:" labels
    let mut trimmed = cleaned.trim();
    
    // Iteratively strip prefixes commonly used by LLMs (case-insensitive)
    let prefixes = ["thought", "agent", ":", ">"];
    let mut changed = true;
    while changed {
        changed = false;
        let lower = trimmed.to_lowercase();
        for prefix in &prefixes {
            if lower.starts_with(prefix) {
                trimmed = trimmed[prefix.len()..].trim();
                // Re-evaluate lower since we trimmed
                changed = true;
                break;
            }
        }
    }
    
    trimmed.to_string()
}
