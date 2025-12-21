use crate::config::AppConfig;
use crate::state::BotState;
use crate::agent::{run_agent_process, get_agent, AgentContext};
use matrix_sdk::{
    room::Room,
    ruma::events::room::message::RoomMessageEventContent,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::fs;

/// Displays help text with available commands.
/// Displays help text with available commands.
/// Displays help text with available commands.
pub async fn handle_help(config: &AppConfig, state: Arc<Mutex<BotState>>, room: &Room) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    let current_project = room_state.current_project_path.as_deref().unwrap_or("None");

    let mut response = String::from("**🤖 Construct Help**\n");
    response.push_str(&format!("**Current Project**: `{}`\n\n", current_project));
    response.push_str("Usage: .command <args> or !command <args>\n\n");
    
    response.push_str("**📂 Project Management**\n");
    response.push_str("* .project <path>: Shortcut to set project/workdir\n");
    response.push_str("* .set <key> <value>: Set a configuration variable (e.g., `project`, `agent`)\n");
    response.push_str("* .list: List available projects\n");
    response.push_str("* .agents: List available agents\n");
    response.push_str("* .model <name>: Set/Override active model for this room\n");
    response.push_str("* .read <file1> <file2>: Read file contents\n");
    response.push_str("* .status: Show current state\n");
    response.push_str("* .new [name]: Reset task, optionally create new project dir\n\n");

    response.push_str("**📝 Task Workflow**\n");
    response.push_str("* .task <desc>: Start a new task & generate plan\n");
    response.push_str("* .modify <feedback>: Refine the current plan\n");
    response.push_str("* .start: Execute or Resume plan\n");
    response.push_str("* .stop: Stop execution loop\n");
    response.push_str("* .ask <msg>: Talk to the agent\n");
    response.push_str("* .reject: Clear the plan\n\n");

    response.push_str("**🛠️ Git & DevOps**\n");
    response.push_str("* .changes: Show `git diff`\n");
    response.push_str("* .commit <msg>: Add & commit all changes\n");
    response.push_str("* .discard: Revert changes (`git checkout .`)\n");
    response.push_str("* .rebuild: Run build/rebuild command\n");
    response.push_str("* .deploy: Run deploy command\n\n");

    response.push_str("**⚡ Custom Commands**\n");

    if let Some(allowed) = config.commands.get("allowed") {
        for key in allowed.keys() {
            response.push_str(&format!("* {}\n", key));
        }
    }

    let content = RoomMessageEventContent::text_markdown(response);
    let _ = room.send(content).await;
}

/// Handles project-related commands (setting or viewing the path).
pub async fn handle_project(config: &AppConfig, state: Arc<Mutex<BotState>>, argument: &str, room: &Room) {
    let mut bot_state = state.lock().await;
    let room_id = room.room_id().as_str();
    
    if argument.is_empty() {
        let state = bot_state.get_room_state(room_id);
        let resp = match &state.current_project_path {
            Some(path) => format!("📂 Current project: `{}`", path),
            None => "📂 No project set. Use `.project <path>`".to_string(),
        };
        let _ = room.send(RoomMessageEventContent::text_plain(resp)).await;
    } else {
        let final_path = if !argument.starts_with('/') {
            if let Some(base) = &config.system.projects_dir {
                format!("{}/{}", base, argument)
            } else {
                argument.to_string()
            }
        } else {
            argument.to_string()
        };
        
        let state = bot_state.get_room_state(room_id);
        state.current_project_path = Some(final_path.clone());
        bot_state.save();
        let _ = room.send(RoomMessageEventContent::text_plain(format!("✅ Project set to: `{}`", final_path))).await;
    }
}

/// Handles the `.set` command, supporting generic key-value pairs.
pub async fn handle_set(config: &AppConfig, state: Arc<Mutex<BotState>>, argument: &str, room: &Room) {
    let mut parts = argument.splitn(2, ' ');
    let key = parts.next().unwrap_or("").trim();
    let value = parts.next().unwrap_or("").trim();

    if key.is_empty() || value.is_empty() {
        let _ = room.send(RoomMessageEventContent::text_plain("⚠️ Usage: `.set <key> <value>`")).await;
        return;
    }

    match key {
        "project" | "workdir" => handle_project(config, state, value, room).await,
        "agent" => {
            let mut bot_state = state.lock().await;
            let room_state = bot_state.get_room_state(room.room_id().as_str());
            room_state.active_agent = Some(value.to_string());
            bot_state.save();
            let _ = room.send(RoomMessageEventContent::text_plain(format!("✅ Agent set to: `{}`", value))).await;
        }
        _ => {
            let _ = room.send(RoomMessageEventContent::text_plain(format!("⚠️ Unknown variable `{}`. Supported: `project`, `agent`", key))).await;
        }
    }
}

/// Reads one or more files and prints their contents.
pub async fn handle_read(state: Arc<Mutex<BotState>>, argument: &str, room: &Room) {
    if argument.is_empty() {
        let _ = room.send(RoomMessageEventContent::text_plain("⚠️ Please specify files: `.read <file1> <file2>`")).await;
        return;
    }

    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    let mut response = String::new();

    for file in argument.split_whitespace() {
        let path = room_state.current_project_path.as_ref()
            .map(|p| format!("{}/{}", p, file))
            .unwrap_or_else(|| file.to_string());

        match fs::read_to_string(&path) {
            Ok(content) => {
                response.push_str(&format!("**📄 `{}`**\n```\n{}\n```\n\n", file, content));
            }
            Err(e) => {
                response.push_str(&format!("❌ Failed to read `{}`: {}\n\n", file, e));
            }
        }
    }

    let _ = room.send(RoomMessageEventContent::text_markdown(response)).await;
}

/// Lists available projects in the configured projects directory.
pub async fn handle_list(config: &AppConfig, room: &Room) {
    let projects_dir = match &config.system.projects_dir {
        Some(dir) => dir,
        None => {
            let _ = room.send(RoomMessageEventContent::text_plain("⚠️ No `projects_dir` configured.")).await;
            return;
        }
    };

    let mut projects = Vec::new();
    if let Ok(entries) = fs::read_dir(projects_dir) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    if let Ok(name) = entry.file_name().into_string() {
                        if !name.starts_with('.') {
                            projects.push(name);
                        }
                    }
                }
            }
        }
    }

    projects.sort();

    if projects.is_empty() {
        let _ = room.send(RoomMessageEventContent::text_plain("📂 No projects found.")).await;
    } else {
        let mut response = String::from("**📂 Available Projects**\n");
        for project in projects {
            response.push_str(&format!("* `{}`\n", project));
        }
        let _ = room.send(RoomMessageEventContent::text_markdown(response)).await;
    }
}

/// Lists available agents configured in the bot.
/// Lists available agents configured in the bot.
/// Lists available agents configured in the bot.
pub async fn handle_agents(config: &AppConfig, state: Arc<Mutex<BotState>>, room: &Room) {
    let mut response = String::from("🕵️ Available Agents\n");
    let mut model_list = Vec::new();

    // Fetch models concurrently - using discovery logic
    // We assume discovery.rs logic is compatible or we catch errors in the match blocks
    use crate::agent::discovery;

    let (gemini_res, claude_res) = tokio::join!(
        discovery::list_gemini_models(config),
        discovery::list_anthropic_models(config)
    );

    // Sort keys for stable output
    let mut agents: Vec<_> = config.agents.iter().collect();
    agents.sort_by_key(|(k, _)| *k);

    for (name, cfg) in agents {
        let current = if cfg.model.is_empty() { "default" } else { &cfg.model };
        response.push_str(&format!("* {} (Protocol: `{}`, Default: `{}`)\n", name, cfg.protocol, current));
        
        // Helper to add model
        let mut add_model = |m: &str, output: &mut String| {
            model_list.push(m.to_string());
            output.push_str(&format!("    - [{}] `{}`\n", model_list.len(), m));
        };

        // Check if we have discovery results for this agent type
        if cfg.protocol == "gemini" {
             match &gemini_res {
                Ok(models) => {
                    response.push_str("  - Available Models:\n");
                    for m in models {
                        add_model(m, &mut response);
                    }
                },
                Err(e) => response.push_str(&format!("  - ⚠️ Failed to list models: {}\n", e)),
            }
        } else if cfg.protocol == "claude" || cfg.protocol == "anthropic" {
             match &claude_res {
                Ok(models) => {
                    response.push_str("  - Available Models:\n");
                    for m in models {
                         add_model(m, &mut response);
                    }
                },
                Err(e) => response.push_str(&format!("  - ⚠️ Failed to list models: {}\n", e)),
            }
        }
    }
    
    // Check if map is empty, showing fallback
    if config.agents.is_empty() {
        response.push_str("* (No specific agents configured)\n");
    }
    
    response.push_str("* deepai (universal fallback)\n");

    // Scope the borrow of room_state
    let (active, override_model) = {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(room.room_id().as_str());
        
        // Save the list for index selection
        room_state.last_model_list = model_list;
        
        let active = room_state.active_agent.as_deref().unwrap_or("auto").to_string();
        let override_model = room_state.active_model.clone();
        
        bot_state.save();
        
        (active, override_model)
    };

    response.push_str(&format!("\nActive in this room: `{}`", active));
    
    if let Some(model) = override_model {
         response.push_str(&format!("\nModel Override: `{}`", model));
    }
    
    let _ = room.send(RoomMessageEventContent::text_markdown(response)).await;
}

/// Sets the active model for the current room.
/// Sets the active model for the current room.
pub async fn handle_model(state: Arc<Mutex<BotState>>, argument: &str, room: &Room) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    let arg = argument.trim();

    if arg.is_empty() {
        room_state.active_model = None;
        let _ = room.send(RoomMessageEventContent::text_plain("🔄 Model override cleared. Using agent defaults.")).await;
    } else {
        // Try parsing as number
        if let Ok(idx) = arg.parse::<usize>() {
            if idx > 0 && idx <= room_state.last_model_list.len() {
                let model_name = room_state.last_model_list[idx - 1].clone();
                room_state.active_model = Some(model_name.clone());
                 let _ = room.send(RoomMessageEventContent::text_markdown(format!("🎯 Active model set to `{}` (Index {})", model_name, idx))).await;
            } else {
                let _ = room.send(RoomMessageEventContent::text_plain(format!("⚠️ Invalid index {}. Use `.agents` to see list.", idx))).await;
            }
        } else {
             room_state.active_model = Some(arg.to_string());
             let _ = room.send(RoomMessageEventContent::text_markdown(format!("🎯 Active model set to `{}`", arg))).await;
        }
    }
    bot_state.save();
}

/// Resets the active task for the current room.
/// If an argument is provided, creates a new project directory and sets it as the working directory.
pub async fn handle_new(config: &AppConfig, state: Arc<Mutex<BotState>>, argument: &str, room: &Room) {
    let mut bot_state = state.lock().await;
    let mut response = String::from("🧹 Active task cleared.");

    if !argument.is_empty() {
        let final_path = if !argument.starts_with('/') {
            if let Some(base) = &config.system.projects_dir {
                format!("{}/{}", base, argument)
            } else {
                argument.to_string()
            }
        } else {
            argument.to_string()
        };

        // Attempt to create the directory
        match fs::create_dir_all(&final_path) {
            Ok(_) => {
                let room_state = bot_state.get_room_state(room.room_id().as_str());
                room_state.current_project_path = Some(final_path.clone());
                response.push_str(&format!("\n📂 Created and set project directory to: `{}`", final_path));
            }
            Err(e) => {
                response.push_str(&format!("\n❌ Failed to create directory `{}`: {}", final_path, e));
            }
        }
    }

    let room_state = bot_state.get_room_state(room.room_id().as_str());
    room_state.active_task = None;
    bot_state.save();
    
    response.push_str("\n\nUse `.task` to start a new workflow.");
    let _ = room.send(RoomMessageEventContent::text_plain(response)).await;
}

// Helper to resolve the active agent name with smart fallback.
pub fn resolve_agent_name(active_agent: Option<&String>, config: &AppConfig) -> String {
    if let Some(agent) = active_agent {
        return agent.clone();
    }
    
    // Check for explicit default in allowed commands
    if let Some(agent) = config.commands.get("allowed").and_then(|m| m.get("agent")) {
        return agent.clone();
    }
    
    // Smart Fallbacks based on configured services
    // Smart Fallbacks based on configured services - Updated for HashMap
    if config.agents.contains_key("gemini") { return "gemini".to_string(); }
    if config.agents.contains_key("claude") { return "claude".to_string(); }
    if config.agents.contains_key("copilot") { return "copilot".to_string(); }
    
    "deepai".to_string()
}

/// Starts a new task by generating a plan using an agent.
pub async fn handle_task(config: &AppConfig, state: Arc<Mutex<BotState>>, argument: &str, room: &Room) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    room_state.active_task = Some(argument.to_string());
    
    let working_dir = room_state.current_project_path.clone();
    let active_agent = room_state.active_agent.clone();
    let active_model = room_state.active_model.clone();
    
    // Crucial: Drop state reference before saving to avoid borrow conflict
    bot_state.save();
    
    let task_desc = argument.to_string();
    let room_clone = room.clone();
    let config_clone = config.clone();

    tokio::spawn(async move {
        let _ = room_clone.send(RoomMessageEventContent::text_markdown(format!("📝 Task started: **{}**\nGenerating plan...", task_desc))).await;
        
        let system_prompt = fs::read_to_string("system_prompt.md").unwrap_or_default();
        let prompt = format!("{}\n\nTask: {}\n\nPlease generate two files:\n1. `plan.md`: A detailed technical plan.\n2. `tasks.md`: A checklist of the tasks.\n\nIMPORTANT: Return the content of each file in a separate code block. Precede each code block with the filename (e.g., `plan.md` or `tasks.md`) so I can distinguish them. format:\n\nplan.md\n```markdown\n...content...\n```\n\ntasks.md\n```markdown\n...content...\n```", system_prompt, task_desc);
        
        let agent_name = resolve_agent_name(active_agent.as_ref(), &config_clone);

        let agent = get_agent(&agent_name, &config_clone);
        let context = AgentContext {
            prompt,
            working_dir: working_dir.clone(),
            model: active_model,
        };
        
        match agent.execute(&context).await {
            Ok(output) => {
                let _ = room_clone.send(RoomMessageEventContent::text_markdown("📜 **Plan Phase Complete**.")).await;
                
                // Simple parsing helper
                let parse_file = |name: &str, text: &str| -> Option<String> {
                     let start_marker = format!("{}\n```", name);
                     let alt_marker = format!("{}\n```markdown", name);
                     
                     let start_idx = text.find(&start_marker).or_else(|| text.find(&alt_marker));
                     
                     if let Some(idx) = start_idx {
                         // Find the start of the code block content
                         let after_marker = if text[idx..].starts_with(&start_marker) {
                             idx + start_marker.len()
                         } else {
                             idx + alt_marker.len()
                         };

                         // Find the end of the code block
                         if let Some(end_idx) = text[after_marker..].find("```") {
                             let content = &text[after_marker..after_marker + end_idx];
                             // Trim language identifier if present (e.g. `markdown`) just in case regex didn't catch it cleanly depending on format
                             let content = content.trim_start_matches("markdown").trim();
                             return Some(content.to_string());
                         }
                     }
                     None
                };

                let plan_content = parse_file("plan.md", &output).unwrap_or_else(|| output.clone()); // Fallback to whole output for plan if not strict
                let tasks_content = parse_file("tasks.md", &output);

                let plan_path = working_dir.as_ref().map(|p| format!("{}/plan.md", p)).unwrap_or_else(|| "plan.md".to_string());
                if let Err(e) = fs::write(&plan_path, &plan_content) {
                     let _ = room_clone.send(RoomMessageEventContent::text_plain(format!("⚠️ Failed to write plan.md: {}", e))).await;
                }

                if let Some(tasks) = tasks_content {
                    let tasks_path = working_dir.as_ref().map(|p| format!("{}/tasks.md", p)).unwrap_or_else(|| "tasks.md".to_string());
                    if let Err(e) = fs::write(&tasks_path, &tasks) {
                         let _ = room_clone.send(RoomMessageEventContent::text_plain(format!("⚠️ Failed to write tasks.md: {}", e))).await;
                    }
                }

                let _ = room_clone.send(RoomMessageEventContent::text_markdown(format!("### Plan\n\n{}\n\n### Tasks generated.", plan_content))).await;
            }
            Err(e) => {
                let _ = room_clone.send(RoomMessageEventContent::text_markdown(format!("⚠️ **Failed to generate plan**:\n{}", e))).await;
            }
        }
    });
}

/// Refines the current plan based on user feedback.
pub async fn handle_modify(config: &AppConfig, state: Arc<Mutex<BotState>>, argument: &str, room: &Room) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    let task_desc = match &room_state.active_task {
        Some(t) => t.clone(),
        None => {
            let _ = room.send(RoomMessageEventContent::text_plain("⚠️ No active task to modify. Use `.task` first.")).await;
            return;
        }
    };

    let working_dir = room_state.current_project_path.clone();
    let active_agent = room_state.active_agent.clone(); 
    let active_model = room_state.active_model.clone();
    let feedback = argument.to_string();
    let room_clone = room.clone();
    let config_clone = config.clone();

    tokio::spawn(async move {
        let _ = room_clone.send(RoomMessageEventContent::text_markdown(format!("🔄 Modifying plan with feedback: *{}*", feedback))).await;
        
        let system_prompt = fs::read_to_string("system_prompt.md").unwrap_or_default();
        let plan_path = working_dir.as_ref().map(|p| format!("{}/plan.md", p)).unwrap_or_else(|| "plan.md".to_string());
        let current_plan = fs::read_to_string(&plan_path).unwrap_or_else(|_| "No plan found.".to_string());

        let prompt = format!(
            "{}\n\nOriginal Task: {}\n\nCurrent Plan:\n{}\n\nFeedback: {}\n\nPlease update the plan.md based on the feedback.\n\nIMPORTANT: Return the content of plan.md in a code block.",
            system_prompt, task_desc, current_plan, feedback
        );

        let agent_name = resolve_agent_name(active_agent.as_ref(), &config_clone);

        let agent = get_agent(&agent_name, &config_clone);
        let context = AgentContext {
            prompt,
            working_dir: working_dir.clone(),
            model: active_model,
        };

        match agent.execute(&context).await {
            Ok(output) => {
                 if let Err(e) = fs::write(&plan_path, &output) {
                     let _ = room_clone.send(RoomMessageEventContent::text_plain(format!("⚠️ Failed to write updated plan.md: {}", e))).await;
                }
                let _ = room_clone.send(RoomMessageEventContent::text_markdown(format!("📜 **Plan Updated**:\n\n{}", output))).await;
            }
             Err(e) => {
                let _ = room_clone.send(RoomMessageEventContent::text_markdown(format!("⚠️ **Failed to modify plan**:\n{}", e))).await;
            }
        }
    });
}

/// Shared logic for the interactive execution loop.
/// Can be called from `handle_approve` (start new) or `handle_continue` (resume old).
async fn run_interactive_loop(
    config: AppConfig,
    state: Arc<Mutex<BotState>>,
    room: Room,
    mut conversation_history: String,
    working_dir: Option<String>,
    active_agent: Option<String>,
    active_model: Option<String>,
) {
    let mut step_count = 0;
    let max_steps = 20;

    // Resolve tools once
    let agent_name = resolve_agent_name(active_agent.as_ref(), &config);
    let agent = get_agent(&agent_name, &config);
    let system_prompt = fs::read_to_string("system_prompt.md").unwrap_or_default();
    let room_clone = room.clone();

    loop {
        // Check for stop request
        {
            let mut bot_state = state.lock().await;
            let room_state = bot_state.get_room_state(room.room_id().as_str());
            if room_state.stop_requested {
                room_state.stop_requested = false;
                bot_state.save();
                let _ = room_clone.send(RoomMessageEventContent::text_plain("🛑 **Execution stopped by user.**")).await;
                break;
            }
        }

        step_count += 1;
        if step_count > max_steps {
            let _ = room_clone.send(RoomMessageEventContent::text_plain("⚠️ **Limit Reached**: Stopped to prevent infinite loop.")).await;
            break;
        }

        let cwd_msg = working_dir.as_deref().map(|d| format!(" (You are in `{}`)", d)).unwrap_or_default();
        let prompt = format!("Based on the plan and previous outputs, what is the NEXT single command to run?{} \n\nRULES:\n1. Return the command in a code block, e.g., ```bash\nls -la\n```.\n2. To create/edit a file, return the content in a code block and place `WRITE_FILE: <filename>` on the line BEFORE the code block.\n3. If you are finished with the entire plan, return a code block with the text `DONE`.\n4. Do not output multiple commands in one turn unless chained with `&&`.\n5. Wait for the result before proceeding.", cwd_msg);

        let context = AgentContext {
            prompt: format!("{}\n\nHistory:\n{}\n\nUser: {}", system_prompt, conversation_history, prompt),
            working_dir: working_dir.clone(),
            model: active_model.clone(),
        };

        let _ = room_clone.typing_notice(true).await;

        match agent.execute(&context).await {
            Ok(response) => {
                let _ = room_clone.typing_notice(false).await;
                
                // Check for stop request again after long generation
                {
                    let mut bot_state = state.lock().await;
                    let room_state = bot_state.get_room_state(room.room_id().as_str());
                    if room_state.stop_requested {
                        room_state.stop_requested = false;
                        bot_state.save();
                        let _ = room_clone.send(RoomMessageEventContent::text_plain("🛑 **Execution stopped by user.**")).await;
                        break;
                    }
                }

                // Save state immediately after getting response (checkpoint)
                {
                   let mut bot_state = state.lock().await;
                   let _room_state = bot_state.get_room_state(room.room_id().as_str());
                   // We append the agent's response to history before saving, strictly speaking we should finish the turn but saving early helps
                   // However, for clean history, we update history locally then push to state
                }

                // Helper to extract code
                let extract_code = |text: &str| -> Option<(String, String)> {
                    if let Some(start) = text.find("```") {
                        if let Some(end) = text[start + 3..].find("```") {
                            let block = &text[start + 3..start + 3 + end];
                            let mut lines = block.lines();
                            let lang = lines.next().unwrap_or("").trim();
                            let content = lines.collect::<Vec<&str>>().join("\n");
                            if content.is_empty() {
                                return Some(("".to_string(), lang.to_string()));
                            }
                            return Some((lang.to_string(), content.trim().to_string()));
                        }
                    }
                    None
                };

                // Helper to parse WRITE_FILE
                let parse_write_file = |text: &str| -> Option<(String, String)> {
                    // Check for "WRITE_FILE: <filename>" pattern
                    if let Some(idx) = text.find("WRITE_FILE:") {
                        let rest = &text[idx + 11..]; // skip "WRITE_FILE:"
                        let end_line = rest.find('\n').unwrap_or(rest.len());
                        let filename = rest[..end_line].trim().to_string();
                        
                        // Extract content from code block
                        if let Some((_, content)) = extract_code(text) {
                            return Some((filename, content));
                        }
                    }
                    None
                };

                if let Some((filename, content)) = parse_write_file(&response) {
                     let _ = room_clone.send(RoomMessageEventContent::text_markdown(format!("💾 **Writing File**: `{}`", filename))).await;
                     
                     let path = working_dir.as_ref()
                        .map(|d| format!("{}/{}", d, filename))
                        .unwrap_or_else(|| filename.clone());

                     // Ensure parent dirs exist
                     if let Some(parent) = std::path::Path::new(&path).parent() {
                         let _ = fs::create_dir_all(parent);
                     }

                     match fs::write(&path, &content) {
                         Ok(_) => {
                             let _ = room_clone.send(RoomMessageEventContent::text_plain("✅ File written successfully.")).await;
                             conversation_history.push_str(&format!("\n\nAgent: {}\n\nSystem: File `{}` written successfully.", response, filename));
                         },
                         Err(e) => {
                             let _ = room_clone.send(RoomMessageEventContent::text_plain(format!("❌ Failed to write file: {}", e))).await;
                             conversation_history.push_str(&format!("\n\nAgent: {}\n\nSystem: Failed to write file `{}`: {}", response, filename, e));
                         }
                     }
                     // Save state
                    {
                        let mut bot_state = state.lock().await;
                        let room_state = bot_state.get_room_state(room.room_id().as_str());
                        room_state.execution_history = Some(conversation_history.clone());
                        bot_state.save();
                    }

                } else if let Some((_lang, content)) = extract_code(&response) {
                    if content == "DONE" {
                        // 1. Read and parse tasks.md
                        let mut tasks_summary = String::new();
                        if let Some(wd) = &working_dir {
                            let tasks_path = format!("{}/tasks.md", wd);
                            if let Ok(tasks_content) = fs::read_to_string(tasks_path) {
                                let completed: Vec<&str> = tasks_content.lines()
                                    .filter(|l| l.trim().to_lowercase().starts_with("- [x]"))
                                    .collect();
                                
                                if !completed.is_empty() {
                                    tasks_summary.push_str("### 📝 Completed Tasks\n");
                                    for task in completed {
                                        tasks_summary.push_str(&format!("{}\n", task));
                                    }
                                    tasks_summary.push('\n');
                                }
                            }
                        }

                        // 2. Extract final comment (everything except the DONE block)
                        // We do a simple replace or split to get the text part
                        let final_comment = response.replace("```\nDONE\n```", "").replace("```DONE```", "").trim().to_string();
                        
                        // 3. Construct Final Message
                        let final_msg = format!("🏁 **Execution Complete**\n\n{}{}\n### 📋 Result\n{}", 
                            tasks_summary,
                            if tasks_summary.is_empty() { "*(No tasks.md found or no tasks marked complete)*\n" } else { "" },
                            if final_comment.is_empty() { "*(No final comment provided)*" } else { &final_comment }
                        );

                        let _ = room_clone.send(RoomMessageEventContent::text_markdown(final_msg)).await;

                        // Clear active task to be clean
                        let mut bot_state = state.lock().await;
                        let room_state = bot_state.get_room_state(room.room_id().as_str());
                        room_state.active_task = None;
                        room_state.execution_history = None;
                        bot_state.save();
                        break;
                    }

                    let _ = room_clone.send(RoomMessageEventContent::text_markdown(format!("🤖 **Agent wants to run**:\n```\n{}\n```", content))).await;

                    let cmd_result = run_agent_process(&content, working_dir.as_deref()).await;

                    let display_output = if cmd_result.len() > 1000 {
                        format!("{}... (truncated)", &cmd_result[..1000])
                    } else {
                        cmd_result.clone()
                    };
                    let _ = room_clone.send(RoomMessageEventContent::text_markdown(format!("✅ **Output**:\n```\n{}\n```", display_output))).await;

                    // Update history
                    conversation_history.push_str(&format!("\n\nAgent: {}\n\nSystem Command Output: {}", response, cmd_result));
                    
                    // Persist history
                    {
                        let mut bot_state = state.lock().await;
                        let room_state = bot_state.get_room_state(room.room_id().as_str());
                        room_state.execution_history = Some(conversation_history.clone());
                        bot_state.save();
                    }

                } else {
                    let _ = room_clone.send(RoomMessageEventContent::text_markdown(format!("🤔 **Agent says**:\n{}", response))).await;
                    conversation_history.push_str(&format!("\n\nAgent: {}", response));
                     // Persist history even if just chatting
                    {
                        let mut bot_state = state.lock().await;
                        let room_state = bot_state.get_room_state(room.room_id().as_str());
                        room_state.execution_history = Some(conversation_history.clone());
                        bot_state.save();
                    }
                }
            }
            Err(e) => {
                let _ = room_clone.send(RoomMessageEventContent::text_plain(format!("⚠️ Agent error: {}", e))).await;
                break;
            }
        }
    }
}

/// Stops the current interactive execution loop.
pub async fn handle_stop(state: Arc<Mutex<BotState>>, room: &Room) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    room_state.stop_requested = true;
    bot_state.save();
    let _ = room.send(RoomMessageEventContent::text_plain("🛑 Stop requested. Waiting for current step to finish...")).await;
}

/// Approves the current plan and executes it using an agent in an interactive loop.
pub async fn handle_approve(config: &AppConfig, state: Arc<Mutex<BotState>>, room: &Room) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    if let Some(task) = &room_state.active_task {
        let working_dir = room_state.current_project_path.clone();
        let active_agent = room_state.active_agent.clone();
        let active_model = room_state.active_model.clone();
        let task_desc = task.clone();
        
        let system_prompt = fs::read_to_string("system_prompt.md").unwrap_or_default();
        let plan_path = working_dir.as_ref().map(|p| format!("{}/plan.md", p)).unwrap_or_else(|| "plan.md".to_string());
        let plan = fs::read_to_string(&plan_path).unwrap_or_default();
        let tasks_path = working_dir.as_ref().map(|p| format!("{}/tasks.md", p)).unwrap_or_else(|| "tasks.md".to_string());
        let tasks = fs::read_to_string(&tasks_path).unwrap_or_default();

        let initial_history = format!("{}\n\nTask: {}\n\nPlan:\n{}\n\nTasks Checklist:\n{}\n\nYou are executing this plan. We will do this step-by-step.\n\nYou are currently in directory: `{}`", system_prompt, task_desc, plan, tasks, working_dir.as_deref().unwrap_or("unknown"));
        
        // Initialize state history
        room_state.execution_history = Some(initial_history.clone());
        bot_state.save();

        let room_clone = room.clone();
        let config_clone = config.clone();
        let state_clone = state.clone();

        tokio::spawn(async move {
            let _ = room_clone.send(RoomMessageEventContent::text_markdown(format!("✅ Plan approved for: **{}**\nStarting interactive execution...", task_desc))).await;
            run_interactive_loop(config_clone, state_clone, room_clone, initial_history, working_dir, active_agent, active_model).await;
        });

    } else {
        let _ = room.send(RoomMessageEventContent::text_plain("⚠️ No task to approve.")).await;
    }
}

/// Resumes the interactive execution loop from where it left off.
pub async fn handle_continue(config: &AppConfig, state: Arc<Mutex<BotState>>, room: &Room) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());

    if let Some(history) = &room_state.execution_history {
        let working_dir = room_state.current_project_path.clone();
        let active_agent = room_state.active_agent.clone();
        let active_model = room_state.active_model.clone();
        let history_clone = history.clone();
        
        let room_clone = room.clone();
        let config_clone = config.clone();
        let state_clone = state.clone();

        tokio::spawn(async move {
             let _ = room_clone.send(RoomMessageEventContent::text_markdown("🔄 **Resuming execution**...")).await;
             run_interactive_loop(config_clone, state_clone, room_clone, history_clone, working_dir, active_agent, active_model).await;
        });
    } else {
         let _ = room.send(RoomMessageEventContent::text_plain("⚠️ No execution history found to continue. Start a new task.")).await;
    }
}

/// Unified start command.
/// If history exists, it continues. If not, it approves/starts the current task.
pub async fn handle_start(config: &AppConfig, state: Arc<Mutex<BotState>>, room: &Room) {
    let should_continue = {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(room.room_id().as_str());
        room_state.execution_history.is_some()
    };

    if should_continue {
        handle_continue(config, state, room).await;
    } else {
        handle_approve(config, state, room).await;
    }
}

/// Sends a direct chat message to the active agent.
pub async fn handle_ask(config: &AppConfig, state: Arc<Mutex<BotState>>, argument: &str, room: &Room) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    
    let working_dir = room_state.current_project_path.clone();
    let prompt = argument.to_string();
    let room_clone = room.clone();
    let config_clone = config.clone();
    
    let active_agent = room_state.active_agent.clone();
    let active_model = room_state.active_model.clone();

    // Drop lock before spawn
    bot_state.save();

    tokio::spawn(async move {
        let _ = room_clone.typing_notice(true).await;
        
        let agent_name = resolve_agent_name(active_agent.as_ref(), &config_clone);
        
        let agent = get_agent(&agent_name, &config_clone);
        let context = AgentContext {
            prompt,
            working_dir: working_dir.clone(),
            model: active_model,
        };
        
        let response = match agent.execute(&context).await {
            Ok(out) => out,
            Err(e) => format!("⚠️ **Error**: {}", e),
        };
        
        let _ = room_clone.send(RoomMessageEventContent::text_markdown(response)).await;
        let _ = room_clone.typing_notice(false).await;
    });
}

/// Rejects the current plan and clears the active task for the current room.
pub async fn handle_reject(state: Arc<Mutex<BotState>>, room: &Room) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    room_state.active_task = None;
    room_state.execution_history = None;
    bot_state.save();
    let _ = room.send(RoomMessageEventContent::text_plain("❌ Plan rejected. Task cleared.")).await;
}

/// Shows current git changes in the active project.
pub async fn handle_changes(state: Arc<Mutex<BotState>>, room: &Room) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    let response = run_agent_process("git diff", room_state.current_project_path.as_deref()).await;
    let _ = room.send(RoomMessageEventContent::text_markdown(format!("🔍 **Current Changes**:\n```diff\n{}\n```", response))).await;
}

/// Commits changes in the active project.
pub async fn handle_commit(state: Arc<Mutex<BotState>>, argument: &str, room: &Room) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    if argument.is_empty() {
        let _ = room.send(RoomMessageEventContent::text_plain("⚠️ Please provide a commit message: `.commit <message>`")).await;
    } else {
        let cmd = format!("git add . && git commit -m \"{}\"", argument);
        let resp = run_agent_process(&cmd, room_state.current_project_path.as_deref()).await;
        let _ = room.send(RoomMessageEventContent::text_markdown(format!("🚀 **Committed**:\n{}", resp))).await;
    }
}

/// Discards uncommitted changes in the active project.
pub async fn handle_discard(state: Arc<Mutex<BotState>>, room: &Room) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    let _ = run_agent_process("git checkout .", room_state.current_project_path.as_deref()).await;
    let _ = room.send(RoomMessageEventContent::text_plain("🧹 Changes discarded.")).await;
}

/// Triggers a rebuild of the project.
pub async fn handle_rebuild(config: &AppConfig, state: Arc<Mutex<BotState>>, room: &Room) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    let cmd = config.commands.get("allowed").and_then(|m| m.get("rebuild"))
        .map(|s| s.as_str())
        .unwrap_or("cargo build"); // Default to cargo build if not set

    let _ = room.send(RoomMessageEventContent::text_plain("🔨 Rebuilding...")).await;
    let response = run_agent_process(cmd, room_state.current_project_path.as_deref()).await;
    let _ = room.send(RoomMessageEventContent::text_markdown(format!("🔨 **Rebuild Result**:\n{}", response))).await;
}

/// Triggers a deployment of the project.
pub async fn handle_deploy(config: &AppConfig, state: Arc<Mutex<BotState>>, room: &Room) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    let cmd = config.commands.get("allowed").and_then(|m| m.get("deploy"))
        .map(|s| s.as_str())
        .unwrap_or("docker compose up -d --build");

    let _ = room.send(RoomMessageEventContent::text_plain("🚀 Deploying...")).await;
    let response = run_agent_process(cmd, room_state.current_project_path.as_deref()).await;
    let _ = room.send(RoomMessageEventContent::text_markdown(format!("🚀 **Deploy Result**:\n{}", response))).await;
}


/// Shows current status of the bot.
pub async fn handle_status(state: Arc<Mutex<BotState>>, room: &Room) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    let mut status = String::from("**Bot Status**\n");
    status.push_str(&format!("* **Room ID**: `{}`\n", room.room_id().as_str()));
    status.push_str(&format!("* **Project**: `{}`\n", room_state.current_project_path.as_deref().unwrap_or("None")));
    status.push_str(&format!("* **Active Task**: `{}`\n", room_state.active_task.as_deref().unwrap_or("None")));
    
    let _ = room.send(RoomMessageEventContent::text_markdown(status)).await;
}

/// Handles any command defined in the allowed section of the config.
pub async fn handle_custom_command(config: &AppConfig, state: Arc<Mutex<BotState>>, trigger: &str, argument: &str, room: &Room) {
    if let Some(command_binary) = config.commands.get("allowed").and_then(|m| m.get(trigger)) {
        let _ = room.typing_notice(true).await;

        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(room.room_id().as_str());
        let cmd = if argument.is_empty() {
            command_binary.to_string()
        } else {
            format!("{} {}", command_binary, argument)
        };
        
        let response = run_agent_process(&cmd, room_state.current_project_path.as_deref()).await;

        let content = RoomMessageEventContent::text_markdown(response);
        let _ = room.send(content).await;
        let _ = room.typing_notice(false).await;
    }
}
