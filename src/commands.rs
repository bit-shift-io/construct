use crate::agent::{AgentContext, get_agent};
use crate::config::AppConfig;
use crate::state::BotState;
use crate::util::run_command;
use matrix_sdk::{room::Room, ruma::events::room::message::RoomMessageEventContent};
use std::fs;
use std::sync::Arc;
use tokio::sync::Mutex;

use chrono; // Ensure chrono is available

/// Helper to execute an agent with fallback logic for "Out of usage" errors.
async fn execute_with_fallback(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    room: &Room,
    mut context: AgentContext,
    initial_agent_name: &str,
) -> Result<String, String> {
    let mut current_agent_name = initial_agent_name.to_string();
    let mut current_model = context.model.clone();

    // Safety break to prevent infinite loops
    let mut attempts = 0;
    const MAX_ATTEMPTS: usize = 10;

    loop {
        attempts += 1;
        if attempts > MAX_ATTEMPTS {
            return Err("Maximum fallback attempts reached.".to_string());
        }

        let agent = get_agent(&current_agent_name, config);

        // Update context with current model (might have changed in loop)
        context.model = current_model.clone();

        let result = agent.execute(&context).await;

        match result {
            Ok(output) => return Ok(output),
            Err(err) => {
                // Check if error is related to usage/quota/rate limits
                let err_lower = err.to_lowercase();
                if err_lower.contains("out of usage") 
                    || err_lower.contains("quota") 
                    || err_lower.contains("rate limit") 
                    || err_lower.contains("429") 
                    || err_lower.contains("insufficient") {
                    
                    let mut bot_state = state.lock().await;
                    let room_state = bot_state.get_room_state(room.room_id().as_str());

                    // Get config for current agent to resolve default model if needed
                    let agent_conf = config.agents.get(&current_agent_name);
                    
                    // Resolve ACTUAL model name for cooldown key
                    // If current_model is None, we need to know what the agent actually used (its default)
                    let resolved_model_name = current_model.clone().or_else(|| {
                        agent_conf.map(|c| c.model.clone())
                    }).unwrap_or_else(|| "default".to_string());

                    // Record Cooldown using the RESOLVED name
                    let cooldown_key = format!(
                        "{}:{}",
                        current_agent_name,
                        resolved_model_name
                    );
                    
                    let now = chrono::Utc::now().timestamp();
                    room_state.model_cooldowns.insert(cooldown_key, now);
                    // Clean up old cooldowns (older than 1 hour)
                    room_state.model_cooldowns.retain(|_, ts| now - *ts < 3600);

                    if let Some(agent_conf) = agent_conf {
                        // 1. Try next model in list
                        let models_list = if let Some(models) = &agent_conf.models {
                            if !models.is_empty() {
                                Some(models.clone())
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        // Filter out models in cooldown
                        let next_model = if let Some(models) = models_list {
                            // Find current index to start searching from
                            let start_idx = models
                                .iter()
                                .position(|m| m == &resolved_model_name)
                                .map(|i| i + 1)
                                .unwrap_or(0);

                            let mut found = None;
                            // Search forward from current
                            for i in start_idx..models.len() {
                                let candidate = &models[i];
                                let key = format!("{}:{}", current_agent_name, candidate);

                                // Check cooldown
                                if let Some(ts) = room_state.model_cooldowns.get(&key) {
                                    if now - *ts < 3600 {
                                        continue;
                                    }
                                }
                                found = Some(candidate.clone());
                                break;
                            }
                            
                            // If we started mid-list and didn't find one, wrap around and search from 0 to start_idx
                            if found.is_none() && start_idx > 0 {
                                for i in 0..start_idx {
                                    let candidate = &models[i];
                                    // Avoid selecting the one we just failed on (resolved_model_name) if it's in the list
                                    if candidate == &resolved_model_name {
                                        continue;
                                    }
                                    
                                    let key = format!("{}:{}", current_agent_name, candidate);
                                    if let Some(ts) = room_state.model_cooldowns.get(&key) {
                                        if now - *ts < 3600 {
                                            continue;
                                        }
                                    }
                                    found = Some(candidate.clone());
                                    break;
                                }
                            }

                            found
                        } else {
                            None
                        };

                        if let Some(next) = next_model {
                            let msg = format!(
                                "⚠️ Provider Error: {} \n🔄 Switching model: {} -> {}",
                                err,
                                resolved_model_name,
                                next
                            );
                            // Notify
                            let _ = room.send(RoomMessageEventContent::text_plain(&msg)).await;

                            // Update state
                            room_state.active_model = Some(next.clone());
                            current_model = Some(next);

                            // Continue loop
                            continue;
                        }

                        // 2. Fallback Agent
                        if let Some(fallback) = &agent_conf.fallback_agent {
                            let msg = format!(
                                "⚠️ Provider Error: {} \n🔄 Switching Agent: {} -> {}",
                                err, current_agent_name, fallback
                            );
                            let _ = room.send(RoomMessageEventContent::text_plain(&msg)).await;

                            // Update state
                            room_state.active_agent = Some(fallback.clone());
                            room_state.active_model = None; // Reset model for new agent

                            current_agent_name = fallback.clone();
                            current_model = None;
                            continue;
                        }
                    }
                }

                // If not handled or other error, return it
                return Err(err);
            }
        }
    }
}

/// Displays help text with available commands.
/// Displays help text with available commands.
/// Displays help text with available commands.
/// Displays help text with available commands.
pub async fn handle_help(
    _config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    room: &Room,
    is_admin: bool,
) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    let _current_project_full = room_state.current_project_path.as_deref().unwrap_or("None");

    let mut response = String::from("**🤖 Construct Help**\n");
    response.push_str("Usage: .command _args_\n\n");

    response.push_str("**📂 Project**\n");
    response.push_str("* project [path]: Set active project\n");
    response.push_str("* new [name]: Reset/create project\n");
    response.push_str("* set [k] [v]: Set config var\n");
    response.push_str("* list: List projects\n");
    response.push_str("* agents: List agents\n");
    response.push_str("* model [name]: Set active model\n");
    response.push_str("* read [files]: Read file contents\n");
    response.push_str("* status: Show bot state\n\n");

    response.push_str("**📝 Tasks**\n");
    response.push_str("* task [desc]: Start new task\n");
    response.push_str("* modify [text]: Refine current plan\n");
    response.push_str("* start: Execute/resume plan\n");
    response.push_str("* stop: Stop execution\n");
    response.push_str("* ask [msg]: Chat with agent\n");
    response.push_str("* reject: Clear current plan\n\n");

    response.push_str("**🛠️ Dev**\n");
    response.push_str("* changes: Show git diff\n");
    response.push_str("* commit [msg]: Commit changes\n");
    response.push_str("* discard: Revert changes\n");
    response.push_str("* build: Run build\n");
    response.push_str("* deploy: Run deploy\n\n");

    if is_admin {
        response.push_str("**⚡ Admin**\n");
        response.push_str("* , [cmd]: Terminal command\n");
    }

    // Dont want to list allowed commands from new config
    //for key in &config.commands.allowed {
    //    response.push_str(&format!("* {}\n", key));
    //}

    let content = RoomMessageEventContent::text_markdown(response);
    let _ = room.send(content).await;
}

/// Handles project-related commands (setting or viewing the path).
pub async fn handle_project(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    argument: &str,
    room: &Room,
) {
    let mut bot_state = state.lock().await;
    let room_id = room.room_id().as_str();

    if argument.is_empty() {
        let state = bot_state.get_room_state(room_id);
        let resp = match &state.current_project_path {
            Some(path) => {
                let name = crate::util::get_project_name(path);
                format!("📂 **Current project**: `{}`", name)
            }
            None => "📂 **No project set**. Use `.project _path_`".to_string(),
        };
        let _ = room
            .send(RoomMessageEventContent::text_markdown(resp))
            .await;
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

        let projects_root = config
            .system
            .projects_dir
            .clone()
            .unwrap_or_else(|| ".".to_string());
        let sandbox = crate::sandbox::Sandbox::new(projects_root);
        let display_path = sandbox.virtualize_output(&final_path);

        let _ = room
            .send(RoomMessageEventContent::text_markdown(format!(
                "✅ **Project set to**: `{}`",
                display_path
            )))
            .await;
    }
}

/// Handles the `.set` command, supporting generic key-value pairs.
pub async fn handle_set(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    argument: &str,
    room: &Room,
) {
    let mut parts = argument.splitn(2, ' ');
    let key = parts.next().unwrap_or("").trim();
    let value = parts.next().unwrap_or("").trim();

    if key.is_empty() || value.is_empty() {
        let _ = room
            .send(RoomMessageEventContent::text_markdown(
                "⚠️ **Usage**: `.set _key_ _value_`",
            ))
            .await;
        return;
    }

    match key {
        "project" | "workdir" => handle_project(config, state, value, room).await,
        "agent" => {
            let mut bot_state = state.lock().await;
            let room_state = bot_state.get_room_state(room.room_id().as_str());
            room_state.active_agent = Some(value.to_string());
            bot_state.save();
            let _ = room
                .send(RoomMessageEventContent::text_markdown(format!(
                    "✅ **Agent set to**: `{}`",
                    value
                )))
                .await;
        }
        _ => {
            let _ = room
                .send(RoomMessageEventContent::text_markdown(format!(
                    "⚠️ Unknown variable `{}`. Supported: `project`, `agent`",
                    key
                )))
                .await;
        }
    }
}

/// Reads one or more files and prints their contents.
pub async fn handle_read(state: Arc<Mutex<BotState>>, argument: &str, room: &Room) {
    if argument.is_empty() {
        let _ = room
            .send(RoomMessageEventContent::text_markdown(
                "⚠️ **Please specify files**: `.read _file1_ _file2_`",
            ))
            .await;
        return;
    }

    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    let mut response = String::new();

    for file in argument.split_whitespace() {
        let path = room_state
            .current_project_path
            .as_ref()
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

    let _ = room
        .send(RoomMessageEventContent::text_markdown(response))
        .await;
}

/// Lists available projects in the configured projects directory.
pub async fn handle_list(config: &AppConfig, room: &Room) {
    let projects_dir = match &config.system.projects_dir {
        Some(dir) => dir,
        None => {
            let _ = room
                .send(RoomMessageEventContent::text_markdown(
                    "⚠️ No `projects_dir` configured.",
                ))
                .await;
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
        let _ = room
            .send(RoomMessageEventContent::text_markdown(
                "📂 **No projects found**.",
            ))
            .await;
    } else {
        let mut response = String::from("**📂 Available Projects**\n");
        for project in projects {
            response.push_str(&format!("* `{}`\n", project));
        }
        let _ = room
            .send(RoomMessageEventContent::text_markdown(response))
            .await;
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
        let current = if cfg.model.is_empty() {
            "default"
        } else {
            &cfg.model
        };
        response.push_str(&format!(
            "* {} (Protocol: `{}`, Default: `{}`)\n",
            name, cfg.protocol, current
        ));

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
                }
                Err(e) => response.push_str(&format!("  - ⚠️ Failed to list models: {}\n", e)),
            }
        } else if cfg.protocol == "claude" || cfg.protocol == "anthropic" {
            match &claude_res {
                Ok(models) => {
                    response.push_str("  - Available Models:\n");
                    for m in models {
                        add_model(m, &mut response);
                    }
                }
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

        let active = room_state
            .active_agent
            .as_deref()
            .unwrap_or("auto")
            .to_string();
        let override_model = room_state.active_model.clone();

        bot_state.save();

        (active, override_model)
    };

    response.push_str(&format!("\nActive in this room: `{}`", active));

    if let Some(model) = override_model {
        response.push_str(&format!("\nModel Override: `{}`", model));
    }

    let _ = room
        .send(RoomMessageEventContent::text_markdown(response))
        .await;
}

/// Sets the active model for the current room.
/// Sets the active model for the current room.
pub async fn handle_model(state: Arc<Mutex<BotState>>, argument: &str, room: &Room) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    let arg = argument.trim();

    if arg.is_empty() {
        room_state.active_model = None;
        let _ = room
            .send(RoomMessageEventContent::text_plain(
                "🔄 Model override cleared. Using agent defaults.",
            ))
            .await;
    } else {
        // Try parsing as number
        if let Ok(idx) = arg.parse::<usize>() {
            if idx > 0 && idx <= room_state.last_model_list.len() {
                let model_name = room_state.last_model_list[idx - 1].clone();
                room_state.active_model = Some(model_name.clone());
                let _ = room
                    .send(RoomMessageEventContent::text_markdown(format!(
                        "🎯 Active model set to `{}` (Index {})",
                        model_name, idx
                    )))
                    .await;
            } else {
                let _ = room
                    .send(RoomMessageEventContent::text_plain(format!(
                        "⚠️ Invalid index {}. Use `.agents` to see list.",
                        idx
                    )))
                    .await;
            }
        } else {
            room_state.active_model = Some(arg.to_string());
            let _ = room
                .send(RoomMessageEventContent::text_markdown(format!(
                    "🎯 Active model set to `{}`",
                    arg
                )))
                .await;
        }
    }
    bot_state.save();
}

/// Entry point for .new command. STARTS the wizard if not internal.
pub async fn handle_new(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    argument: &str,
    room: &Room,
) {
    let wizard_active = {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(room.room_id().as_str());
        room_state.wizard.active
    };

    if !wizard_active {
        let name_arg = if argument.is_empty() {
            None
        } else {
            Some(argument.to_string())
        };
        crate::wizard::start_wizard(state.clone(), room, name_arg).await;
        return;
    }

    create_new_project(config, state, argument, room).await;
}

pub async fn create_new_project(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    argument: &str,
    room: &Room,
) {
    let mut bot_state = state.lock().await;
    let mut response = String::from("🧹 **Active task cleared**.");

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
                // Initialize Project Documentation
                let _ = fs::write(format!("{}/roadmap.md", final_path), "# Project Roadmap\n\n- [ ] Initial Setup");
                let _ = fs::write(format!("{}/architecture.md", final_path), "# System Architecture\n\nTo be defined.");
                let _ = fs::write(format!("{}/changelog.md", final_path), "# Changelog\n\n## Unreleased");

                let room_state = bot_state.get_room_state(room.room_id().as_str());
                room_state.current_project_path = Some(final_path.clone());
                response.push_str(&format!(
                    "\n📂 **Created and set project directory to**: `{}`\n📄 **Initialized specs**: `roadmap.md`, `architecture.md`, `changelog.md`",
                    final_path
                ));
            }
            Err(e) => {
                response.push_str(&format!(
                    "\n❌ **Failed to create directory** `{}`: {}",
                    final_path, e
                ));
            }
        }
    }

    let room_state = bot_state.get_room_state(room.room_id().as_str());
    room_state.active_task = None;
    room_state.is_task_completed = false;
    room_state.execution_history = None;
    bot_state.save();

    response.push_str("\n\nUse `.task` to start a new workflow.");
    let _ = room
        .send(RoomMessageEventContent::text_markdown(response))
        .await;
}

// Helper to resolve the active agent name with smart fallback.
pub fn resolve_agent_name(active_agent: Option<&String>, config: &AppConfig) -> String {
    if let Some(agent) = active_agent {
        return agent.clone();
    }

    // Defaulting to smart detection if no explicit agent is active

    // Smart Fallbacks based on configured services
    // Smart Fallbacks based on configured services - Updated for HashMap
    if config.agents.contains_key("gemini") {
        return "gemini".to_string();
    }
    if config.agents.contains_key("claude") {
        return "claude".to_string();
    }
    if config.agents.contains_key("copilot") {
        return "copilot".to_string();
    }

    "deepai".to_string()
}

/// Starts a new task by generating a plan using an agent.
pub async fn handle_task(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    argument: &str,
    room: &Room,
) {
    if argument.is_empty() {
        let wizard_active = {
            let mut bot_state = state.lock().await;
            let room_state = bot_state.get_room_state(room.room_id().as_str());
            room_state.wizard.active
        };

        if !wizard_active {
            crate::wizard::start_task_wizard(state.clone(), room).await;
            return;
        }
    }

    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    room_state.active_task = Some(argument.to_string());
    room_state.is_task_completed = false;
    room_state.execution_history = None;

    let working_dir = room_state.current_project_path.clone();
    let active_agent = room_state.active_agent.clone();
    let active_model = room_state.active_model.clone();

    // Crucial: Drop state reference before saving to avoid borrow conflict
    bot_state.save();
    drop(bot_state);

    let task_desc = argument.to_string();
    let room_clone = room.clone();
    let config_clone = config.clone();

    tokio::spawn(async move {
        let _ = room_clone
            .send(RoomMessageEventContent::text_markdown(format!(
                "📝 Task started: **{}**\nGenerating plan...",
                task_desc
            )))
            .await;

        let system_prompt = fs::read_to_string("system_prompt.md").unwrap_or_default();
        
        // Read Project Context and Detect New Project
        let mut project_context = String::new();
        let mut is_new_project = false;

        if let Some(wd) = &working_dir {
            if let Ok(roadmap) = fs::read_to_string(format!("{}/roadmap.md", wd)) {
                if roadmap.contains("- [ ] Initial Setup") {
                    is_new_project = true;
                }
                project_context.push_str(&format!("\n\n### Project Roadmap (Context Only)\n{}\n", roadmap));
            }
            if let Ok(arch) = fs::read_to_string(format!("{}/architecture.md", wd)) {
                project_context.push_str(&format!("\n\n### Architecture (Context Only)\n{}\n", arch));
            }
        }

        let mut instructions = String::from("1. Use the 'Project Roadmap' and 'Architecture' above for understanding the big picture and constraints.\n2. Your scope is STRICTLY limited to the 'Task' described above. Do NOT try to complete other roadmap items.\n3. Generate two files:\n   - `plan.md`: A detailed technical plan for THIS specific task.\n   - `tasks.md`: A checklist of the subtasks for THIS specific task.");

        let mut return_format = String::from("plan.md\n```markdown\n...content...\n```\n\ntasks.md\n```markdown\n...content...\n```");

        if is_new_project {
             instructions.push_str("\n4. **NEW PROJECT DETECTED**: You MUST also generate `roadmap.md` and `architecture.md` based on the task requirements to replace the default placeholders.");
             return_format.push_str("\n\nroadmap.md\n```markdown\n...content...\n```\n\narchitecture.md\n```markdown\n...content...\n```");
        }

        let prompt = format!(
            "{}\n{}\n\nTask: {}\n\nINSTRUCTIONS:\n{}\n\nIMPORTANT: Return the content of each file in a separate code block. Precede each code block with the filename. format:\n\n{}",
            system_prompt, project_context, task_desc, instructions, return_format
        );

        let agent_name = resolve_agent_name(active_agent.as_ref(), &config_clone);

        let context = AgentContext {
            prompt,
            working_dir: working_dir.clone(),
            model: active_model,
        };

        let result = execute_with_fallback(
            &config_clone,
            state.clone(),
            &room_clone,
            context,
            &agent_name,
        )
        .await;

        match result {
            Ok(output) => {
                let _ = room_clone
                    .send(RoomMessageEventContent::text_markdown(
                        "📜 **Plan Phase Complete**.",
                    ))
                    .await;

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

                let plan_path = working_dir
                    .as_ref()
                    .map(|p| format!("{}/plan.md", p))
                    .unwrap_or_else(|| "plan.md".to_string());
                if let Err(e) = fs::write(&plan_path, &plan_content) {
                    let _ = room_clone
                        .send(RoomMessageEventContent::text_plain(format!(
                            "⚠️ Failed to write plan.md: {}",
                            e
                        )))
                        .await;
                }

                if let Some(tasks) = tasks_content {
                    let tasks_path = working_dir
                        .as_ref()
                        .map(|p| format!("{}/tasks.md", p))
                        .unwrap_or_else(|| "tasks.md".to_string());
                    if let Err(e) = fs::write(&tasks_path, &tasks) {
                        let _ = room_clone
                            .send(RoomMessageEventContent::text_plain(format!(
                                "⚠️ Failed to write tasks.md: {}",
                                e
                            )))
                            .await;
                    }
                }

                // Check for extra files if new project
                let mut extra_msg = String::new();
                if let Some(roadmap) = parse_file("roadmap.md", &output) {
                     if let Some(wd) = &working_dir {
                         let _ = fs::write(format!("{}/roadmap.md", wd), &roadmap);
                         extra_msg.push_str("\n### Roadmap Updated\n");
                         extra_msg.push_str(&roadmap);
                     }
                }
                if let Some(arch) = parse_file("architecture.md", &output) {
                     if let Some(wd) = &working_dir {
                         let _ = fs::write(format!("{}/architecture.md", wd), &arch);
                         extra_msg.push_str("\n### Architecture Updated\n");
                         extra_msg.push_str(&arch);
                     }
                }

                let _ = room_clone
                    .send(RoomMessageEventContent::text_markdown(format!(
                        "### Plan\n\n{}\n\n### Tasks generated.{}\n",
                        plan_content, extra_msg
                    )))
                    .await;
            }
            Err(e) => {
                let _ = room_clone
                    .send(RoomMessageEventContent::text_markdown(format!(
                        "⚠️ **Failed to generate plan**:\n{}",
                        e
                    )))
                    .await;
            }
        }
    });
}

/// Refines the current plan based on user feedback.
pub async fn handle_modify(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    argument: &str,
    room: &Room,
) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    let task_desc = match &room_state.active_task {
        Some(t) => t.clone(),
        None => {
            let _ = room
                .send(RoomMessageEventContent::text_markdown(
                    "⚠️ No active task to modify. Use `.task` first.",
                ))
                .await;
            return;
        }
    };

    let working_dir = room_state.current_project_path.clone();
    let active_agent = room_state.active_agent.clone();
    let active_model = room_state.active_model.clone();
    let feedback = argument.to_string();
    let room_clone = room.clone();
    let config_clone = config.clone();

    drop(bot_state);

    tokio::spawn(async move {
        let _ = room_clone
            .send(RoomMessageEventContent::text_markdown(format!(
                "🔄 Modifying plan with feedback: *{}*",
                feedback
            )))
            .await;

        let system_prompt = fs::read_to_string("system_prompt.md").unwrap_or_default();
        let plan_path = working_dir
            .as_ref()
            .map(|p| format!("{}/plan.md", p))
            .unwrap_or_else(|| "plan.md".to_string());
        let current_plan =
            fs::read_to_string(&plan_path).unwrap_or_else(|_| "No plan found.".to_string());

        let prompt = format!(
            "{}\n\nOriginal Task: {}\n\nCurrent Plan:\n{}\n\nFeedback: {}\n\nPlease update the plan.md based on the feedback.\n\nIMPORTANT: Return the content of plan.md in a code block.",
            system_prompt, task_desc, current_plan, feedback
        );

        let agent_name = resolve_agent_name(active_agent.as_ref(), &config_clone);

        let context = AgentContext {
            prompt,
            working_dir: working_dir.clone(),
            model: active_model,
        };

        let result = execute_with_fallback(
            &config_clone,
            state.clone(),
            &room_clone,
            context,
            &agent_name,
        )
        .await;

        match result {
            Ok(output) => {
                if let Err(e) = fs::write(&plan_path, &output) {
                    let _ = room_clone
                        .send(RoomMessageEventContent::text_markdown(format!(
                            "⚠️ Failed to write updated plan.md: {}",
                            e
                        )))
                        .await;
                }
                let _ = room_clone
                    .send(RoomMessageEventContent::text_markdown(format!(
                        "📜 **Plan Updated**:\n\n{}",
                        output
                    )))
                    .await;
            }
            Err(e) => {
                let _ = room_clone
                    .send(RoomMessageEventContent::text_markdown(format!(
                        "⚠️ **Failed to modify plan**:\n{}",
                        e
                    )))
                    .await;
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
                let _ = room_clone
                    .send(RoomMessageEventContent::text_markdown(
                        "🛑 **Execution stopped by user.**",
                    ))
                    .await;
                break;
            }
        }

        step_count += 1;
        if step_count > max_steps {
            let _ = room_clone
                .send(RoomMessageEventContent::text_markdown(
                    "⚠️ **Limit Reached**: Stopped to prevent infinite loop.",
                ))
                .await;
            break;
        }

        let cwd_msg = working_dir
            .as_deref()
            .map(|d| format!(" (You are in `{}`)", d))
            .unwrap_or_default();
        let prompt = format!(
            "Based on the plan and previous outputs, what is the NEXT single command to run?{} \n\nRULES:\n1. Return the command in a code block, e.g., ```bash\nls -la\n```.\n2. To create/edit a file, return the content in a code block and place `WRITE_FILE: <filename>` on the line BEFORE the code block.\n3. If you are finished with the entire plan, return a code block with the text `DONE`.\n4. Do not output multiple commands in one turn unless chained with `&&`.\n5. Wait for the result before proceeding.",
            cwd_msg
        );

        let context = AgentContext {
            prompt: format!(
                "{}\n\nHistory:\n{}\n\nUser: {}",
                system_prompt, conversation_history, prompt
            ),
            working_dir: working_dir.clone(),
            model: active_model.clone(),
        };

        let _ = room_clone.typing_notice(true).await;

        let result =
            execute_with_fallback(&config, state.clone(), &room_clone, context, &agent_name).await;

        match result {
            Ok(response) => {
                let _ = room_clone.typing_notice(false).await;

                // Check for stop request again after long generation
                {
                    let mut bot_state = state.lock().await;
                    let room_state = bot_state.get_room_state(room.room_id().as_str());
                    if room_state.stop_requested {
                        room_state.stop_requested = false;
                        bot_state.save();
                        let _ = room_clone
                            .send(RoomMessageEventContent::text_markdown(
                                "🛑 **Execution stopped by user.**",
                            ))
                            .await;
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
                    let _ = room_clone
                        .send(RoomMessageEventContent::text_markdown(format!(
                            "💾 **Writing File**: `{}`",
                            filename
                        )))
                        .await;

                    let path = working_dir
                        .as_ref()
                        .map(|d| format!("{}/{}", d, filename))
                        .unwrap_or_else(|| filename.clone());

                    // Ensure parent dirs exist
                    if let Some(parent) = std::path::Path::new(&path).parent() {
                        let _ = fs::create_dir_all(parent);
                    }

                    match fs::write(&path, &content) {
                        Ok(_) => {
                            let _ = room_clone
                                .send(RoomMessageEventContent::text_plain(
                                    "✅ File written successfully.",
                                ))
                                .await;
                            conversation_history.push_str(&format!(
                                "\n\nAgent: {}\n\nSystem: File `{}` written successfully.",
                                response, filename
                            ));
                        }
                        Err(e) => {
                            let _ = room_clone
                                .send(RoomMessageEventContent::text_plain(format!(
                                    "❌ Failed to write file: {}",
                                    e
                                )))
                                .await;
                            conversation_history.push_str(&format!(
                                "\n\nAgent: {}\n\nSystem: Failed to write file `{}`: {}",
                                response, filename, e
                            ));
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
                                let completed: Vec<&str> = tasks_content
                                    .lines()
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
                        let final_comment = response
                            .replace("```\nDONE\n```", "")
                            .replace("```DONE```", "")
                            .trim()
                            .to_string();

                        // 3. Construct Final Message
                        let final_msg = format!(
                            "🏁 **Execution Complete**\n\n{}{}\n### 📋 Result\n{}",
                            tasks_summary,
                            if tasks_summary.is_empty() {
                                "*(No tasks.md found or no tasks marked complete)*\n"
                            } else {
                                ""
                            },
                            if final_comment.is_empty() {
                                "*(No final comment provided)*"
                            } else {
                                &final_comment
                            }
                        );

                        let _ = room_clone
                            .send(RoomMessageEventContent::text_markdown(final_msg))
                            .await;

                        // Mark task as completed but keep the description
                        let mut bot_state = state.lock().await;
                        let room_state = bot_state.get_room_state(room.room_id().as_str());
                        room_state.is_task_completed = true;
                        room_state.execution_history = None;
                        bot_state.save();
                        break;
                    }

                    let _ = room_clone
                        .send(RoomMessageEventContent::text_markdown(format!(
                            "🤖 **Agent wants to run**:\n```\n{}\n```",
                            content
                        )))
                        .await;

                    let cmd_result = match run_command(&content, working_dir.as_deref()).await {
                        Ok(o) => o,
                        Err(e) => e,
                    };

                    let display_output = if cmd_result.len() > 1000 {
                        format!("{}... (truncated)", &cmd_result[..1000])
                    } else {
                        cmd_result.clone()
                    };
                    let _ = room_clone
                        .send(RoomMessageEventContent::text_markdown(format!(
                            "✅ **Output**:\n```\n{}\n```",
                            display_output
                        )))
                        .await;

                    // Update history
                    conversation_history.push_str(&format!(
                        "\n\nAgent: {}\n\nSystem Command Output: {}",
                        response, cmd_result
                    ));

                    // Persist history
                    {
                        let mut bot_state = state.lock().await;
                        let room_state = bot_state.get_room_state(room.room_id().as_str());
                        room_state.execution_history = Some(conversation_history.clone());
                        bot_state.save();
                    }
                } else {
                    let _ = room_clone
                        .send(RoomMessageEventContent::text_markdown(format!(
                            "🤔 **Agent says**:\n{}",
                            response
                        )))
                        .await;
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
                let _ = room_clone
                    .send(RoomMessageEventContent::text_plain(format!(
                        "⚠️ Agent error: {}",
                        e
                    )))
                    .await;
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
    let _ = room
        .send(RoomMessageEventContent::text_markdown(
            "🛑 **Stop requested**. Waiting for current step to finish...",
        ))
        .await;
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
        let plan_path = working_dir
            .as_ref()
            .map(|p| format!("{}/plan.md", p))
            .unwrap_or_else(|| "plan.md".to_string());
        let plan = fs::read_to_string(&plan_path).unwrap_or_default();
        let tasks_path = working_dir
            .as_ref()
            .map(|p| format!("{}/tasks.md", p))
            .unwrap_or_else(|| "tasks.md".to_string());
        let tasks = fs::read_to_string(&tasks_path).unwrap_or_default();

        let initial_history = format!(
            "{}\n\nTask: {}\n\nPlan:\n{}\n\nTasks Checklist:\n{}\n\nYou are executing this plan. We will do this step-by-step.\n\nYou are currently in directory: `{}`",
            system_prompt,
            task_desc,
            plan,
            tasks,
            working_dir.as_deref().unwrap_or("unknown")
        );

        // Initialize state history
        room_state.execution_history = Some(initial_history.clone());
        bot_state.save();

        let room_clone = room.clone();
        let config_clone = config.clone();
        let state_clone = state.clone();

        tokio::spawn(async move {
            let _ = room_clone
                .send(RoomMessageEventContent::text_markdown(format!(
                    "✅ Plan approved for: **{}**\nStarting interactive execution...",
                    task_desc
                )))
                .await;
            run_interactive_loop(
                config_clone,
                state_clone,
                room_clone,
                initial_history,
                working_dir,
                active_agent,
                active_model,
            )
            .await;
        });
    } else {
        let _ = room
            .send(RoomMessageEventContent::text_markdown(
                "⚠️ **No task to approve**.",
            ))
            .await;
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
            let _ = room_clone
                .send(RoomMessageEventContent::text_markdown(
                    "🔄 **Resuming execution**...",
                ))
                .await;
            run_interactive_loop(
                config_clone,
                state_clone,
                room_clone,
                history_clone,
                working_dir,
                active_agent,
                active_model,
            )
            .await;
        });
    } else {
        let _ = room
            .send(RoomMessageEventContent::text_markdown(
                "⚠️ **No execution history found to continue**. Start a new task.",
            ))
            .await;
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
pub async fn handle_ask(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    argument: &str,
    room: &Room,
) {
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
    drop(bot_state);

    tokio::spawn(async move {
        let _ = room_clone.typing_notice(true).await;

        let agent_name = resolve_agent_name(active_agent.as_ref(), &config_clone);

        let context = AgentContext {
            prompt,
            working_dir: working_dir.clone(),
            model: active_model,
        };

        let response = match execute_with_fallback(
            &config_clone,
            state.clone(),
            &room_clone,
            context,
            &agent_name,
        )
        .await
        {
            Ok(out) => out,
            Err(e) => format!("⚠️ **Error**: {}", e),
        };

        let _ = room_clone
            .send(RoomMessageEventContent::text_markdown(response))
            .await;
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
    let _ = room
        .send(RoomMessageEventContent::text_markdown(
            "❌ **Plan rejected**. Task cleared.",
        ))
        .await;
}

/// Shows current git changes in the active project.
pub async fn handle_changes(state: Arc<Mutex<BotState>>, room: &Room) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    let response = match run_command("git diff", room_state.current_project_path.as_deref()).await {
        Ok(o) => o,
        Err(e) => e,
    };
    let _ = room
        .send(RoomMessageEventContent::text_markdown(format!(
            "🔍 **Current Changes**:\n```diff\n{}\n```",
            response
        )))
        .await;
}

/// Commits changes in the active project.
pub async fn handle_commit(state: Arc<Mutex<BotState>>, argument: &str, room: &Room) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    if argument.is_empty() {
        let _ = room
            .send(RoomMessageEventContent::text_markdown(
                "⚠️ **Please provide a commit message**: `.commit _message_`",
            ))
            .await;
    } else {
        let cmd = format!("git add . && git commit -m \"{}\"", argument);
        let resp = match run_command(&cmd, room_state.current_project_path.as_deref()).await {
            Ok(o) => o,
            Err(e) => e,
        };
        let _ = room
            .send(RoomMessageEventContent::text_markdown(format!(
                "🚀 **Committed**:\n{}",
                resp
            )))
            .await;
    }
}

/// Discards uncommitted changes in the active project.
pub async fn handle_discard(state: Arc<Mutex<BotState>>, room: &Room) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    let _ = run_command("git checkout .", room_state.current_project_path.as_deref()).await;
    let _ = room
        .send(RoomMessageEventContent::text_markdown(
            "🧹 **Changes discarded**.",
        ))
        .await;
}

/// Triggers a build of the project.
pub async fn handle_build(_config: &AppConfig, state: Arc<Mutex<BotState>>, room: &Room) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    let cmd = "cargo build";

    let _ = room
        .send(RoomMessageEventContent::text_markdown("🔨 **Building**..."))
        .await;
    let response = match run_command(cmd, room_state.current_project_path.as_deref()).await {
        Ok(o) => o,
        Err(e) => e,
    };
    let _ = room
        .send(RoomMessageEventContent::text_markdown(format!(
            "🔨 **Build Result**:\n{}",
            response
        )))
        .await;
}

/// Triggers a deployment of the project.
pub async fn handle_deploy(_config: &AppConfig, state: Arc<Mutex<BotState>>, room: &Room) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    // Use standard docker deploy if allowed, or hardcoded default
    let cmd = "docker compose up -d --build";

    let _ = room
        .send(RoomMessageEventContent::text_markdown(
            "🚀 **Deploying**...",
        ))
        .await;
    let response = match run_command(cmd, room_state.current_project_path.as_deref()).await {
        Ok(o) => o,
        Err(e) => e,
    };
    let _ = room
        .send(RoomMessageEventContent::text_markdown(format!(
            "🚀 **Deploy Result**:\n{}",
            response
        )))
        .await;
}

/// Shows current status of the bot.
pub async fn handle_status(state: Arc<Mutex<BotState>>, room: &Room) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(room.room_id().as_str());
    let mut status = String::new();

    let current_path = room_state.current_project_path.as_deref().unwrap_or("None");
    let project_name = crate::util::get_project_name(current_path);

    status.push_str("Use `.help` to list commands.\n");
    status.push_str(&format!("**Project**: `{}`\n", project_name));
    status.push_str(&format!(
        "**Agent**: `{}` | `{}`\n",
        room_state.active_agent.as_deref().unwrap_or("None"),
        room_state.active_model.as_deref().unwrap_or("None")
    ));
    let task_display = if room_state.is_task_completed {
        "None"
    } else {
        room_state.active_task.as_deref().unwrap_or("None")
    };

    status.push_str(&format!("**Task**: `{}`\n\n", task_display));

    let _ = room
        .send(RoomMessageEventContent::text_markdown(status))
        .await;
}
