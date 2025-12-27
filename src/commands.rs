use crate::agent::{AgentContext, get_agent};
use crate::config::AppConfig;
use crate::services::ChatService;
use crate::state::BotState;
use crate::util::run_command;
use std::fs;
use std::sync::Arc;
use tokio::sync::{Mutex, watch};

use chrono; // Ensure chrono is available

/// Pauses execution if `action_delay` is configured.
async fn action_delay(config: &AppConfig) {
    if let Some(ms) = config.system.action_delay {
        tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;
    }
}

/// Helper to execute an agent with fallback logic for "Out of usage" errors.
async fn execute_with_fallback(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    room: &impl ChatService,
    mut context: AgentContext,
    initial_agent_name: &str,
) -> Result<String, String> {
    action_delay(config).await;
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

        // GENERIC RATE LIMITING (Proactive)
        let agent_rpm = config
            .agents
            .get(&current_agent_name)
            .and_then(|c| c.requests_per_minute);

        if let Some(rpm) = agent_rpm {
            if rpm > 0 {
                // RPM -> Interval in ms
                // e.g. 30 RPM -> 60/30 = 2s = 2000ms
                let min_interval_ms = (60.0 / rpm as f64 * 1000.0) as u64;
                let now_ms = chrono::Utc::now().timestamp_millis() as u64;

                let sleep_ms = {
                    let bot_state = state.lock().await;
                    // Check global throttle map first
                    let last_time = bot_state
                        .last_provider_usage
                        .get(&current_agent_name)
                        .copied();

                    if let Some(last) = last_time {
                        // If (now - last) < interval, wait the difference
                        // Note: timestamps are u64, handle potential clock skew or restart (last > now)
                        if now_ms >= last {
                            let elapsed = now_ms - last;
                            if elapsed < min_interval_ms {
                                Some(min_interval_ms - elapsed)
                            } else {
                                None
                            }
                        } else {
                            // Clock went backwards or fresh restart with future generic timestamp?
                            // Safer to just wait min_interval if suspicious? Or just proceed.
                            None
                        }
                    } else {
                        None
                    }
                };

                if let Some(ms) = sleep_ms {
                    let duration = std::time::Duration::from_millis(ms);

                    // Only notify if significant wait (>100ms)
                    if ms >= 100 {
                        if let Some(cb) = &context.status_callback {
                            let msg =
                                format!("⏳ Proactive throttling ({:.1}s)...", ms as f64 / 1000.0);
                            cb(msg);
                        }
                    }

                    if let Some(mut rx) = context.abort_signal.clone() {
                        tokio::select! {
                            _ = tokio::time::sleep(duration) => {},
                            _ = rx.changed() => {
                                if *rx.borrow() {
                                    return Err("Cancelled by user".to_string());
                                }
                            }
                        }
                    } else {
                        tokio::time::sleep(duration).await;
                    }
                }

                // Update timestamp after waiting
                {
                    let mut bot_state = state.lock().await;
                    // We update 'last_provider_usage' to NOW (after wait)
                    bot_state.last_provider_usage.insert(
                        current_agent_name.clone(),
                        chrono::Utc::now().timestamp_millis() as u64,
                    );
                }
            }
        }

        let result = agent.execute(&context).await;

        match result {
            Ok(output) => return Ok(output),
            Err(err) => {
                // Check if error is related to usage/quota/rate limits
                let err_lower = err.to_lowercase();

                // Special handling for TPM/TPD rate limits (e.g. "try again in 7m59.52s" or "345ms")
                if err_lower.contains("rate limit") || err_lower.contains("429") {
                    if let Some(start) = err_lower.find("try again in ") {
                        let remainder = &err_lower[start + 13..];
                        // Extract the duration string up to the next space or end (assuming it ends with s/ms/m)
                        // Simple heuristic: take until next whitespace or end of string
                        let duration_str = remainder.split_whitespace().next().unwrap_or(remainder);

                        // Parse duration manually: 7m59.52s -> 7*60*1000 + 59.52*1000
                        // 345ms -> 345
                        let mut total_ms = 0u64;
                        let mut current_num_str = String::new();

                        let chars: Vec<char> = duration_str.chars().collect();
                        let mut i = 0;
                        while i < chars.len() {
                            let c = chars[i];
                            if c.is_digit(10) || c == '.' {
                                current_num_str.push(c);
                            } else {
                                // Found a unit char?
                                let val = current_num_str.parse::<f64>().unwrap_or(0.0);
                                current_num_str.clear();

                                if c == 'm' {
                                    // check next char for 's' (ms) vs 'm' (minutes)
                                    // Actually 'm' usually means minutes unless it's 'ms'
                                    if i + 1 < chars.len() && chars[i + 1] == 's' {
                                        // milliseconds
                                        total_ms += val as u64;
                                        i += 1; // skip 's'
                                    } else {
                                        // minutes
                                        total_ms += (val * 60.0 * 1000.0) as u64;
                                    }
                                } else if c == 's' {
                                    // seconds
                                    total_ms += (val * 1000.0) as u64;
                                } else if c == 'h' {
                                    // hours (just in case)
                                    total_ms += (val * 60.0 * 60.0 * 1000.0) as u64;
                                }
                            }
                            i += 1;
                        }

                        if total_ms > 0 {
                            // Add a small buffer
                            let wait_ms = total_ms + 250;
                            if let Some(cb) = &context.status_callback {
                                let total_seconds = wait_ms / 1000;
                                let hours = total_seconds / 3600;
                                let minutes = (total_seconds % 3600) / 60;
                                let seconds = total_seconds % 60;

                                let mut parts = Vec::new();
                                if hours > 0 {
                                    parts.push(format!("{}h", hours));
                                }
                                if minutes > 0 {
                                    parts.push(format!("{}m", minutes));
                                }
                                parts.push(format!("{}s", seconds));

                                cb(format!(
                                    "⏳ Rate Limit Hit: Retrying in {}...",
                                    parts.join(" ")
                                ));
                            }
                            tokio::time::sleep(tokio::time::Duration::from_millis(wait_ms)).await;

                            // Retry with SAME model
                            continue;
                        }
                    }
                }

                if err_lower.contains("out of usage")
                    || err_lower.contains("quota")
                    || err_lower.contains("rate limit")
                    || err_lower.contains("429")
                    || err_lower.contains("insufficient")
                {
                    let mut bot_state = state.lock().await;
                    let room_state = bot_state.get_room_state(&room.room_id());

                    // Get config for current agent to resolve default model if needed
                    let agent_conf = config.agents.get(&current_agent_name);

                    // Resolve ACTUAL model name for cooldown key
                    // If current_model is None, we need to know what the agent actually used (its default)
                    let resolved_model_name = current_model
                        .clone()
                        .or_else(|| agent_conf.map(|c| c.model.clone()))
                        .unwrap_or_else(|| "default".to_string());

                    // Record Cooldown using the RESOLVED name
                    let cooldown_key = format!("{}:{}", current_agent_name, resolved_model_name);

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
                            let msg = crate::prompts::STRINGS
                                .messages
                                .provider_error_model_switch
                                .replace("{err}", &err)
                                .replace("{from}", &resolved_model_name)
                                .replace("{to}", &next);
                            // Notify
                            let _ = room.send_markdown(&msg).await;

                            // Update state
                            room_state.active_model = Some(next.clone());
                            current_model = Some(next);

                            // Continue loop
                            continue;
                        }

                        // 2. Fallback Agent
                        if let Some(fallback) = &agent_conf.fallback_agent {
                            let msg = crate::prompts::STRINGS
                                .messages
                                .provider_error_agent_switch
                                .replace("{err}", &err)
                                .replace("{from}", &current_agent_name)
                                .replace("{to}", fallback);
                            let _ = room.send_markdown(&msg).await;

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
pub async fn handle_help(
    _config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    room: &impl ChatService,
    _is_admin: bool,
) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    let _current_project_full = room_state.current_project_path.as_deref().unwrap_or("None");

    let _ = room.send_markdown(&crate::prompts::STRINGS.help.main).await;
}

/// Handles project-related commands (setting or viewing the path).
pub async fn handle_project(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    argument: &str,
    room: &impl ChatService,
) {
    let mut bot_state = state.lock().await;
    let room_id = room.room_id();

    if argument.is_empty() {
        let room_state = bot_state.get_room_state(&room_id);
        let resp = match &room_state.current_project_path {
            Some(path) => {
                let name = crate::util::get_project_name(path);
                format!("📂 **Current project**: `{}`", name)
            }
            None => "📂 **No project set**. Use `.project _path_`".to_string(),
        };
        let _ = room.send_markdown(&resp).await;
        return;
    }

    let mut path = argument.to_string();
    if !path.starts_with('/') {
        if let Some(projects_dir) = &config.system.projects_dir {
            path = format!("{}/{}", projects_dir, argument);
        }
    }

    if let Ok(metadata) = fs::metadata(&path) {
        if metadata.is_dir() {
            let mut bot_state = state.lock().await;
            let room_state = bot_state.get_room_state(&room.room_id());
            room_state.current_project_path = Some(path.clone());
            // Clear task context when switching projects
            room_state.active_task = None;
            room_state.execution_history = None;
            room_state.is_task_completed = false;
            bot_state.save();

            let _ = room
                .send_markdown(&format!("📂 **Project info set to**: `{}`", path))
                .await;
        } else {
            let _ = room
                .send_markdown(&format!("⚠️ `{}` is not a directory.", path))
                .await;
        }
    } else {
        let _ = room
            .send_markdown(&format!("⚠️ Path `{}` not found.", path))
            .await;
    }
}

/// Handles the `.set` command, supporting generic key-value pairs.
pub async fn handle_set(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    argument: &str,
    room: &impl ChatService,
) {
    let mut parts = argument.splitn(2, ' ');
    let key = parts.next().unwrap_or("").trim();
    let value = parts.next().unwrap_or("").trim();

    if key.is_empty() || value.is_empty() {
        let _ = room
            .send_markdown("⚠️ **Usage**: `.set _key_ _value_`")
            .await;
        return;
    }

    match key {
        "project" | "workdir" => handle_project(config, state, value, room).await,
        "agent" => {
            let mut bot_state = state.lock().await;
            let room_state = bot_state.get_room_state(&room.room_id());
            room_state.active_agent = Some(value.to_string());
            bot_state.save();
            let _ = room
                .send_markdown(
                    &crate::prompts::STRINGS
                        .messages
                        .model_set
                        .replace("{}", value),
                )
                .await;
        }
        _ => {
            let _ = room
                .send_markdown(&format!(
                    "⚠️ Unknown variable `{}`. Supported: `project`, `agent`",
                    key
                ))
                .await;
        }
    }
}

/// Reads one or more files and prints their contents.
pub async fn handle_read(state: Arc<Mutex<BotState>>, argument: &str, room: &impl ChatService) {
    if argument.is_empty() {
        let _ = room
            .send_markdown("⚠️ **Please specify files**: `.read _file1_ _file2_`")
            .await;
        return;
    }

    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
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

    let _ = room.send_markdown(&response).await;
}

/// Lists available projects in the configured projects directory.
pub async fn handle_list(config: &AppConfig, room: &impl ChatService) {
    let projects_dir = match &config.system.projects_dir {
        Some(dir) => dir,
        None => {
            let _ = room.send_markdown("⚠️ No `projects_dir` configured.").await;
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
            .send_markdown(&crate::prompts::STRINGS.messages.no_projects_found)
            .await;
    } else {
        let mut response = crate::prompts::STRINGS
            .messages
            .available_projects_header
            .clone();
        for project in projects {
            response.push_str(&format!("* `{}`\n", project));
        }
        let _ = room.send_markdown(&response).await;
    }
}

/// Lists and sets the active agent for the current room.
/// If a bridge configuration restricts agents for this room, only those are shown/allowed.
pub async fn handle_agent(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    argument: &str,
    room: &impl ChatService,
) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());

    // 1. Determine Allowed Agents
    // We need to find the BridgeEntry that matches this room's ID (channel ID)
    // The agents list may be in a separate entry in the same bridge, so we need to
    // collect agents from ALL entries in the matching bridge
    let mut allowed_agents = None;
    for (_bridge_name, entries) in &config.bridges {
        let mut found_room = false;
        let mut agents_list = None;

        for entry in entries {
            // Check if this entry matches our room
            if let Some(chan) = &entry.channel {
                if chan == &room.room_id() {
                    found_room = true;
                }
            }

            // Collect agents from any entry in this bridge
            if entry.agents.is_some() {
                agents_list = entry.agents.clone();
            }
        }

        // If we found the room and there's an agents list in this bridge, use it
        if found_room && agents_list.is_some() {
            allowed_agents = agents_list;
            break;
        }
    }

    // 2. Build the full available list, filtering if needed
    let mut all_agents: Vec<_> = config.agents.keys().cloned().collect();
    all_agents.sort();

    let final_list = if let Some(allowed) = &allowed_agents {
        all_agents
            .into_iter()
            .filter(|a| allowed.contains(a))
            .collect::<Vec<_>>()
    } else {
        all_agents
    };

    // Store for index selection
    room_state.last_agent_list = final_list.clone();

    // 3. Handle Argument (Selection)
    if !argument.is_empty() {
        let selection = if let Ok(idx) = argument.parse::<usize>() {
            if idx > 0 && idx <= final_list.len() {
                Some(final_list[idx - 1].clone())
            } else {
                None
            }
        } else {
            // Name matching
            if final_list.contains(&argument.to_string()) {
                Some(argument.to_string())
            } else {
                None
            }
        };

        if let Some(agent_name) = selection {
            room_state.active_agent = Some(agent_name.clone());
            room_state.active_model = None; // Reset model override when switching agents
            bot_state.save();
            let _ = room
                .send_markdown(
                    &crate::prompts::STRINGS
                        .messages
                        .model_set
                        .replace("{}", &agent_name),
                )
                .await;
            return;
        } else {
            let _ = room.send_markdown("⚠️ Invalid agent selection.").await;
            // Fallthrough to show list? Or return?
            // Let's show list to be helpful
        }
    }

    // 4. Show List
    // We need to release the mutable borrow on room_state before calling bot_state.save()
    let current_agent = room_state
        .active_agent
        .clone()
        .unwrap_or_else(|| "auto".to_string());

    // Now we can save the state (which includes last_agent_list update)
    // Implicitly `room_state` borrow ends here if we don't use it anymore
    // But strict borrow checker might require explicit drop or just ensuring no usage after save
    // To be safe and clear:

    // Force end of mutable borrow
    let _ = room_state;
    bot_state.save();

    let mut response = "**🤖 Available Agents**\n\n".to_string();

    if final_list.is_empty() {
        response.push_str("No agents available.\n");
    } else {
        for (idx, name) in final_list.iter().enumerate() {
            let marker = if name == &current_agent { "✅" } else { "" };
            response.push_str(&format!("{} {}. **{}**\n", marker, idx + 1, name));
        }
    }

    response.push_str("\nUse `.agent <name|number>` to switch.");
    let _ = room.send_markdown(&response).await;
}

/// Lists available agents configured in the bot.
pub async fn handle_agents(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    room: &impl ChatService,
) {
    // Reuse handle_agent logic with empty argument to just list
    handle_agent(config, state, "", room).await;
}

/// Lists available models for the ACTIVE agent.
pub async fn handle_models(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    room: &impl ChatService,
) {
    let (mut response, active_agent_name) = {
        let mut bot_state = state.lock().await;
        // Acquire room state to specifically get/init default agent
        // We can't hold lock across await if possible, but simplest way first.
        // But we need active_agent name to look up config.
        let room_state = bot_state.get_room_state(&room.room_id());

        let active = room_state
            .active_agent
            .as_deref()
            .unwrap_or("auto")
            .to_string();

        (format!("**🤖 Models for Agent: {}**\n\n", active), active)
    };

    // Find the config for this agent
    let agent_config = if active_agent_name == "auto" {
        // Just pick the first one or a default?
        // "auto" usually implies some logic.
        // For now, let's just pick the first agent alphabetically if auto.
        let mut keys: Vec<_> = config.agents.keys().collect();
        keys.sort();
        keys.first().map(|k| config.agents.get(*k)).flatten()
    } else {
        config.agents.get(&active_agent_name)
    };

    let Some(cfg) = agent_config else {
        let _ = room
            .send_markdown("⚠️ Active agent configuration not found.")
            .await;
        return;
    };

    // Now fetch models (Network call - slow)
    // We already have logic for this in the old handle_agents.
    // It's specific to provider type.

    // Now fetch models (Network call - slow)
    use crate::agent::discovery;

    let model_list =
        if cfg.protocol == "openai" || cfg.protocol == "groq" || cfg.protocol == "gemini" {
            if cfg.protocol == "gemini" {
                match discovery::list_gemini_models(config).await {
                    Ok(models) => models,
                    Err(e) => {
                        response.push_str(&format!("⚠️ Failed to fetch models: {}\n", e));
                        Vec::new()
                    }
                }
            } else if cfg.protocol == "groq" {
                match discovery::list_groq_models(config).await {
                    Ok(models) => models,
                    Err(e) => {
                        response.push_str(&format!("⚠️ Failed to fetch models: {}\n", e));
                        Vec::new()
                    }
                }
            } else {
                Vec::new()
            }
        } else if cfg.protocol == "anthropic" || cfg.protocol == "claude" {
            match discovery::list_anthropic_models(config).await {
                Ok(models) => models,
                Err(e) => {
                    response.push_str(&format!("⚠️ Failed to fetch models: {}\n", e));
                    Vec::new()
                }
            }
        } else if cfg.protocol == "zai" {
            match discovery::list_zai_models(config).await {
                Ok(models) => models,
                Err(e) => {
                    response.push_str(&format!("⚠️ Failed to fetch models: {}\n", e));
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

    // Save to state for index selection
    {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());
        room_state.last_model_list = model_list.clone();

        let current_model = room_state
            .active_model
            .as_deref()
            .unwrap_or("default")
            .to_string();

        // Fix borrow checker error by dropping mutable borrow before save
        let _ = room_state;
        bot_state.save();

        if model_list.is_empty() {
            response.push_str("No models found or discovery not supported for this agent.\n");
        } else {
            for (idx, name) in model_list.iter().enumerate() {
                let marker = if name == &current_model { "✅" } else { "" };
                response.push_str(&format!("{} {}. **{}**\n", marker, idx + 1, name));
            }
        }
    }

    response.push_str("\nUse `.model <name|number>` to switch active model.");
    let _ = room.send_markdown(&response).await;
}

/// Sets the active model for the current room.
/// Sets the active model for the current room.
pub async fn handle_model(state: Arc<Mutex<BotState>>, argument: &str, room: &impl ChatService) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());

    // Check if argument is an index for selection
    let model_to_set = if let Ok(idx) = argument.parse::<usize>() {
        // 1-based index expected from UI
        if idx > 0 && idx <= room_state.last_model_list.len() {
            Some(room_state.last_model_list[idx - 1].clone())
        } else {
            None
        }
    } else if !argument.is_empty() {
        Some(argument.to_string())
    } else {
        None
    };

    if let Some(model) = model_to_set {
        room_state.active_model = Some(model.clone());
        bot_state.save();
        let _ = room
            .send_markdown(
                &crate::prompts::STRINGS
                    .messages
                    .model_set
                    .replace("{}", &model),
            )
            .await;
    } else if argument.is_empty() {
        room_state.active_model = None;
        bot_state.save();
        let _ = room
            .send_markdown(&crate::prompts::STRINGS.messages.model_reset)
            .await;
    } else {
        let _ = room
            .send_markdown(&crate::prompts::STRINGS.messages.invalid_model)
            .await;
    }
}

/// Entry point for .new command. STARTS the wizard if not internal.
pub async fn handle_new<S: ChatService + Clone + Send + 'static>(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    argument: &str,
    room: &S,
) {
    if argument.is_empty() {
        let wizard_active = {
            let mut bot_state = state.lock().await;
            let room_state = bot_state.get_room_state(&room.room_id());
            room_state.wizard.active
        };

        if !wizard_active {
            crate::wizard::start_new_project_wizard(state.clone(), room).await;
            return;
        }
    }

    let mut bot_state = state.lock().await;

    // We can't hold lock across await if we're not careful, but FS ops are blocking std fs here?
    // Wait, fs::create_dir_all is synchronous std::fs or tokio?
    // The import says 'use std::fs'.

    let projects_dir = match &config.system.projects_dir {
        Some(dir) => dir,
        None => {
            let _ = room
                .send_markdown(&crate::prompts::STRINGS.messages.no_projects_configured)
                .await;
            return;
        }
    };

    let project_name = argument.trim();
    if project_name.is_empty() {
        let _ = room
            .send_markdown(&crate::prompts::STRINGS.messages.provide_project_name)
            .await;
        return;
    }

    // Basic sanitization
    if project_name.contains('/') || project_name.contains('\\') || project_name.starts_with('.') {
        let _ = room
            .send_markdown(&crate::prompts::STRINGS.messages.invalid_project_name)
            .await;
        return;
    }

    let final_path = format!("{}/{}", projects_dir, project_name);

    if let Ok(metadata) = fs::metadata(&final_path) {
        if metadata.is_dir() {
            let room_state = bot_state.get_room_state(&room.room_id());
            room_state.current_project_path = Some(final_path.clone());
            room_state.active_task = None;
            room_state.is_task_completed = false;
            room_state.execution_history = None;
            bot_state.save();

            let _ = room
                .send_markdown(
                    &crate::prompts::STRINGS
                        .messages
                        .project_exists
                        .replace("{}", &final_path),
                )
                .await;
            return;
        }
    }

    // Create
    let mut response = String::new();
    if let Err(e) = fs::create_dir_all(&final_path) {
        response.push_str(
            &crate::prompts::STRINGS
                .messages
                .create_dir_failed
                .replace("{PATH}", &final_path)
                .replace("{ERR}", &e.to_string()),
        );
    } else {
        // Init specs
        let roadmap_path = format!("{}/roadmap.md", final_path);
        let changelog_path = format!("{}/changelog.md", final_path);

        // Populate with defaults if missing
        if !fs::metadata(&roadmap_path).is_ok() {
            let _ = fs::write(
                &roadmap_path,
                &crate::prompts::STRINGS.prompts.roadmap_template,
            );
        }
        if !fs::metadata(&changelog_path).is_ok() {
            let _ = fs::write(
                &changelog_path,
                &crate::prompts::STRINGS.prompts.changelog_template,
            );
        }

        let room_state = bot_state.get_room_state(&room.room_id());
        room_state.current_project_path = Some(final_path.clone());
        room_state.active_task = None;
        room_state.is_task_completed = false;
        room_state.execution_history = None;
        bot_state.save();

        response.push_str(
            &crate::prompts::STRINGS
                .messages
                .project_created
                .replace("{}", &final_path),
        );
    }

    response.push_str(&crate::prompts::STRINGS.messages.use_task_to_start);
    let _ = room.send_markdown(&response).await;
}

// Helper to resolve the active agent name with smart fallback.
pub fn resolve_agent_name(active_agent: Option<&str>, config: &AppConfig) -> String {
    if let Some(agent) = active_agent {
        return agent.to_string();
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
pub async fn handle_task<S: ChatService + Clone + Send + 'static>(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    argument: &str,
    room: &S,
) {
    if argument.is_empty() {
        let wizard_active = {
            let mut bot_state = state.lock().await;
            let room_state = bot_state.get_room_state(&room.room_id());
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
        // Line removed (duplicate): let _ = room_clone.send_markdown(&crate::prompts::STRINGS.messages.task_started.replace("{}", &task_desc)).await;

        let system_prompt = crate::prompts::STRINGS.prompts.system.clone();

        // Read Project Context and Detect New Project
        let mut project_context = String::new();
        let mut is_new_project = false;

        if let Some(wd) = &working_dir {
            if let Ok(roadmap) = fs::read_to_string(format!("{}/roadmap.md", wd)) {
                if roadmap.contains("- [ ] Initial Setup") {
                    is_new_project = true;
                }
                project_context.push_str(
                    &crate::prompts::STRINGS
                        .prompts
                        .roadmap_context
                        .replace("{}", &roadmap),
                );
            }
        }

        let mut instructions = crate::prompts::STRINGS.prompts.task_instructions.clone();

        let mut return_format = crate::prompts::STRINGS.prompts.task_format.clone();

        if is_new_project {
            instructions.push_str(&format!(
                "\n{}",
                crate::prompts::STRINGS.prompts.new_project_instructions
            ));
            return_format.push_str(&format!(
                "\n{}",
                crate::prompts::STRINGS.prompts.new_project_format
            ));
        }

        let prompt = format!(
            "{}\n{}\n\nTask: {}\n\nINSTRUCTIONS:\n{}\n\nIMPORTANT: Return the content of each file in a separate code block. Precede each code block with the filename. format:\n\n{}",
            system_prompt, project_context, task_desc, instructions, return_format
        );

        let _ = room_clone
            .send_markdown(
                &crate::prompts::STRINGS
                    .messages
                    .task_started
                    .replace("{}", &task_desc),
            )
            .await;

        // Send Typing Indicator
        let _ = room_clone.typing(true).await;

        // Resolve agent name
        let agent_name = resolve_agent_name(active_agent.as_deref(), &config_clone);

        let callback_room = room_clone.clone();
        let context = AgentContext {
            prompt,
            working_dir: working_dir.clone(),
            model: active_model,
            status_callback: Some(std::sync::Arc::new(move |msg| {
                let r = callback_room.clone();
                tokio::spawn(async move {
                    let _ = r.send_markdown(&msg).await;
                });
            })),
            abort_signal: None,
        };

        // We need to clone the room wrapper? No, room in signature is reference.
        // execute_with_fallback usually needs ref.
        // However, if we need to pass it to async block, we might need shared ownership or something.
        // Ah, execute_with_fallback takes `&impl ChatService`.
        // But earlier I just passed `&room`.

        let result = execute_with_fallback(
            &config_clone,
            state.clone(),
            &room_clone,
            context,
            &agent_name,
        )
        .await;

        let _ = room_clone.typing(false).await;

        match result {
            Ok(output) => {
                let _ = room_clone.send_markdown(&output).await;

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
                        .send_markdown(
                            &crate::prompts::STRINGS
                                .messages
                                .write_plan_error
                                .replace("{}", &e.to_string()),
                        )
                        .await;
                }

                if let Some(tasks) = tasks_content {
                    let tasks_path = working_dir
                        .as_ref()
                        .map(|p| format!("{}/tasks.md", p))
                        .unwrap_or_else(|| "tasks.md".to_string());
                    if let Err(e) = fs::write(&tasks_path, &tasks) {
                        let _ = room_clone
                            .send_markdown(
                                &crate::prompts::STRINGS
                                    .messages
                                    .write_tasks_error
                                    .replace("{}", &e.to_string()),
                            )
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

                let _ = room_clone
                    .send_markdown(
                        &crate::prompts::STRINGS
                            .messages
                            .plan_generated
                            .replace("{PLAN}", &plan_content)
                            .replace("{TASKS}", &extra_msg),
                    )
                    .await;
            }
            Err(e) => {
                let _ = room_clone
                    .send_markdown(
                        &crate::prompts::STRINGS
                            .messages
                            .plan_generation_failed
                            .replace("{}", &e.to_string()),
                    )
                    .await;
            }
        }
    });
}

/// Refines the current plan based on user feedback.
pub async fn handle_modify<S: ChatService + Clone + Send + 'static>(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    argument: &str,
    room: &S,
) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    let task_desc = match &room_state.active_task {
        Some(t) => t.clone(),
        None => {
            let _ = room
                .send_markdown(&crate::prompts::STRINGS.messages.no_active_task_modify)
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

    let feedback_clone = feedback.to_string();
    tokio::spawn(async move {
        let _ = room_clone
            .send_markdown(
                &crate::prompts::STRINGS
                    .messages
                    .feedback_modification
                    .replace("{FEEDBACK}", &feedback_clone),
            )
            .await;

        let system_prompt = crate::prompts::STRINGS.prompts.system.clone();
        let plan_path = working_dir
            .as_ref()
            .map(|p| format!("{}/plan.md", p))
            .unwrap_or_else(|| "plan.md".to_string());
        let current_plan =
            fs::read_to_string(&plan_path).unwrap_or_else(|_| "No plan found.".to_string());

        let prompt = crate::prompts::STRINGS
            .prompts
            .modify_plan
            .replace("{SYSTEM}", &system_prompt)
            .replace("{TASK}", &task_desc)
            .replace("{PLAN}", &current_plan)
            .replace("{FEEDBACK}", &feedback_clone);

        let agent_name = resolve_agent_name(active_agent.as_deref(), &config_clone);

        let callback_room = room_clone.clone();
        let context = AgentContext {
            prompt,
            working_dir: working_dir.clone(),
            model: active_model,
            status_callback: Some(std::sync::Arc::new(move |msg| {
                let r = callback_room.clone();
                tokio::spawn(async move {
                    let _ = r.send_markdown(&msg).await;
                });
            })),
            abort_signal: None,
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
                        .send_markdown(
                            &crate::prompts::STRINGS
                                .messages
                                .write_plan_error
                                .replace("{}", &e.to_string()),
                        )
                        .await;
                }
                let _ = room_clone
                    .send_markdown(
                        &crate::prompts::STRINGS
                            .messages
                            .plan_updated
                            .replace("{}", &output),
                    )
                    .await;
            }
            Err(e) => {
                let _ = room_clone
                    .send_markdown(
                        &crate::prompts::STRINGS
                            .messages
                            .failed_modify
                            .replace("{}", &e.to_string()),
                    )
                    .await;
            }
        }
    });
}

/// Shared logic for the interactive execution loop.
/// Can be called from `handle_approve` (start new) or `handle_continue` (resume old).
async fn run_interactive_loop<S: ChatService + Clone + Send + 'static>(
    config: AppConfig,
    state: Arc<Mutex<BotState>>,
    room: S,
    mut conversation_history: String,
    mut working_dir: Option<String>,
    active_agent: Option<String>,
    active_model: Option<String>,
) {
    let mut step_count = 0;
    let max_steps = 20;

    // Resolve tools once
    let agent_name = resolve_agent_name(active_agent.as_deref(), &config);
    let system_prompt = crate::prompts::STRINGS.prompts.system.clone();
    let room_clone = room.clone();

    // Initialize Abort Signal
    let (abort_tx, abort_rx) = watch::channel(false);
    {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());
        room_state.abort_handle = Some(Arc::new(abort_tx));
        // Reset stop requested flag on start
        room_state.stop_requested = false;
    }

    loop {
        // Check for stop request
        {
            let mut bot_state = state.lock().await;
            let room_state = bot_state.get_room_state(&room.room_id());
            if room_state.stop_requested {
                room_state.stop_requested = false;
                bot_state.save();
                let _ = room_clone
                    .send_markdown(&crate::prompts::STRINGS.messages.stop_requested)
                    .await;
                break;
            }
        }

        step_count += 1;
        if step_count > max_steps {
            let _ = room_clone
                .send_markdown(&crate::prompts::STRINGS.messages.limit_reached)
                .await;
            break;
        }

        // Read updated Tasks and Roadmap
        let tasks_content = if let Some(wd) = &working_dir {
            fs::read_to_string(format!("{}/tasks.md", wd))
                .unwrap_or_else(|_| "(No tasks.md found)".to_string())
        } else {
            "(No tasks.md found)".to_string()
        };

        let roadmap_content = if let Some(wd) = &working_dir {
            fs::read_to_string(format!("{}/roadmap.md", wd))
                .unwrap_or_else(|_| "(No roadmap.md found)".to_string())
        } else {
            "(No roadmap.md found)".to_string()
        };

        let cwd_msg = working_dir
            .as_deref()
            .map(|d| format!("\n**Context Info**:\n- Current Working Directory: `{}`\n- Project Root: `{}`\n", d, d))
            .unwrap_or_default();

        let prompt = crate::prompts::STRINGS
            .prompts
            .interactive_turn
            .replace("{CWD}", &cwd_msg)
            .replace("{ROADMAP}", &roadmap_content)
            .replace("{TASKS}", &tasks_content);

        let context = AgentContext {
            prompt: format!(
                "{}\n\nHistory:\n{}\n\nUser: {}",
                system_prompt, conversation_history, prompt
            ),
            working_dir: working_dir.clone(),
            model: active_model.clone(),
            status_callback: Some(std::sync::Arc::new({
                let r = room_clone.clone();
                move |msg| {
                    let r_inner = r.clone();
                    tokio::spawn(async move {
                        let _ = r_inner.send_markdown(&msg).await;
                    });
                }
            })),
            abort_signal: Some(abort_rx.clone()),
        };

        let _ = room_clone.typing(true).await;

        let result =
            execute_with_fallback(&config, state.clone(), &room_clone, context, &agent_name).await;

        match result {
            Ok(response) => {
                let _ = room_clone.typing(false).await;

                // Check for stop request again after long generation
                {
                    let mut bot_state = state.lock().await;
                    let room_state = bot_state.get_room_state(room.room_id().as_str());
                    if room_state.stop_requested {
                        room_state.stop_requested = false;
                        bot_state.save();
                        let _ = room_clone
                            .send_markdown(&crate::prompts::STRINGS.messages.stop_requested)
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

                // Clean response (strip hallucinations)
                let clean_response = if let Some(idx) = response.find("System Command Output:") {
                    response[..idx].trim().to_string()
                } else {
                    response.clone()
                };

                // Append Agent response ONCE
                conversation_history.push_str(&format!("\n\nAgent: {}", clean_response));

                // Save intermediate state (Agent part)
                {
                    let mut bot_state = state.lock().await;
                    let room_state = bot_state.get_room_state(room.room_id().as_str());
                    room_state.execution_history = Some(conversation_history.clone());
                    bot_state.save();
                }

                // Helper to extract code
                let actions = crate::util::parse_actions(&response);

                if actions.is_empty() {
                    let _ = room_clone
                        .send_markdown(
                            &crate::prompts::STRINGS
                                .messages
                                .agent_says
                                .replace("{}", &response),
                        )
                        .await;
                    // Already added to history above

                    // Save state
                    {
                        let mut bot_state = state.lock().await;
                        let room_state = bot_state.get_room_state(room.room_id().as_str());
                        room_state.execution_history = Some(conversation_history.clone());
                        bot_state.save();
                    }
                }

                for action in actions {
                    match action {
                        crate::util::AgentAction::WriteFile(filename, content) => {
                            let _ = room_clone
                                .send_markdown(
                                    &crate::prompts::STRINGS
                                        .messages
                                        .writing_file
                                        .replace("{}", &filename),
                                )
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
                                        .send_markdown(
                                            &crate::prompts::STRINGS.messages.file_written,
                                        )
                                        .await;
                                    conversation_history.push_str(&format!(
                                        "\n\nSystem: File `{}` written successfully.",
                                        filename
                                    ));
                                }
                                Err(e) => {
                                    let _ = room_clone
                                        .send_markdown(
                                            &crate::prompts::STRINGS
                                                .messages
                                                .write_failed
                                                .replace("{}", &e.to_string()),
                                        )
                                        .await;
                                    conversation_history.push_str(&format!(
                                        "\n\nSystem: Failed to write file `{}`: {}",
                                        filename, e
                                    ));
                                }
                            }
                        }
                        crate::util::AgentAction::Done => {
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
                            let final_comment = response
                                .replace("```\nDONE\n```", "")
                                .replace("```DONE```", "")
                                .trim()
                                .to_string();

                            // 3. Construct Final Message
                            let _ = room_clone.send_markdown(&crate::prompts::STRINGS.messages.execution_complete
                                .replace("{}", &tasks_summary)
                                .replace("{}", if tasks_summary.is_empty() {
                                    "*(No tasks.md found or no tasks marked complete)*\n"
                                } else {
                                    ""
                                })
                                .replace("{}", if final_comment.is_empty() {
                                    "*(No final comment provided)*"
                                } else {
                                    &final_comment
                                })
                            ).await;

                            // Mark task as completed but keep the description
                            {
                                let mut bot_state = state.lock().await;
                                let room_state = bot_state.get_room_state(room.room_id().as_str());
                                room_state.is_task_completed = true;
                                room_state.execution_history = None;
                                bot_state.save();
                            }
                            // Task completed - save state and return
                            return;
                        }
                        crate::util::AgentAction::ShellCommand(content) => {
                            let _ = room_clone
                                .send_markdown(
                                    &crate::prompts::STRINGS
                                        .messages
                                        .agent_run_code
                                        .replace("{}", &content),
                                )
                                .await;

                            // --- CIRCUIT BREAKER ---
                            let should_block_loop = {
                                let mut bot_state = state.lock().await;
                                let room_state = bot_state.get_room_state(room.room_id().as_str());

                                if let Some(last) = &room_state.last_command {
                                    if last == &content {
                                        room_state.command_retry_count += 1;
                                    } else {
                                        room_state.command_retry_count = 1;
                                        room_state.last_command = Some(content.clone());
                                    }
                                } else {
                                    room_state.command_retry_count = 1;
                                    room_state.last_command = Some(content.clone());
                                }

                                // If retried 3+ times (original + 2 retries), break
                                if room_state.command_retry_count >= 3 {
                                    true
                                } else {
                                    false
                                }
                            };

                            if should_block_loop {
                                let _ = room_clone
                                    .send_markdown(&crate::prompts::STRINGS.messages.limit_reached)
                                    .await;
                                conversation_history.push_str("\n\nSystem: Execution stopped to prevent infinite loop (command repeated 3 times).");
                                // Save history
                                {
                                    let mut bot_state = state.lock().await;
                                    let room_state =
                                        bot_state.get_room_state(room.room_id().as_str());
                                    room_state.execution_history =
                                        Some(conversation_history.clone());
                                    bot_state.save();
                                }
                                break;
                            }

                            // Sandbox Check
                            let sandbox = crate::sandbox::Sandbox::new(
                                std::env::current_dir().unwrap_or_default(),
                            );
                            let permission = sandbox.check_command(&content, &config.commands);

                            let cmd_result = match permission {
                                crate::sandbox::PermissionResult::Allowed => {
                                    let trimmed = content.trim();
                                    if trimmed.starts_with("cd ") {
                                        // Split by '&&' to support chaining
                                        let (cd_part, remainder) =
                                            if let Some((first, rest)) = trimmed.split_once("&&") {
                                                (first.trim(), Some(rest.trim()))
                                            } else {
                                                (trimmed, None)
                                            };

                                        let new_path = cd_part[3..].trim(); // skip "cd "
                                        let path = std::path::Path::new(new_path);
                                        let resolved_path = if path.is_absolute() {
                                            path.to_path_buf()
                                        } else {
                                            let current = working_dir.as_deref().unwrap_or(".");
                                            std::path::Path::new(current).join(path)
                                        };

                                        let cd_res = match std::fs::canonicalize(&resolved_path) {
                                            Ok(p) => {
                                                let p_str = p.to_string_lossy().to_string();
                                                working_dir = Some(p_str.clone());
                                                Ok(p_str) // Return valid path string
                                            }
                                            Err(e) => {
                                                Err(format!("Failed to change directory: {}", e))
                                            }
                                        };

                                        match cd_res {
                                            Ok(_new_dir) => {
                                                if let Some(rest_cmd) = remainder {
                                                    // Execute remainder in new directory
                                                    match crate::util::run_shell_command(
                                                        rest_cmd,
                                                        working_dir.as_deref(),
                                                    )
                                                    .await
                                                    {
                                                        Ok(o) => o,
                                                        Err(e) => e,
                                                    }
                                                } else {
                                                    format!("Changed directory to: {}", _new_dir)
                                                }
                                            }
                                            Err(e) => e,
                                        }
                                    } else {
                                        match crate::util::run_shell_command(
                                            &content,
                                            working_dir.as_deref(),
                                        )
                                        .await
                                        {
                                            Ok(o) => o,
                                            Err(e) => e,
                                        }
                                    }
                                }
                                crate::sandbox::PermissionResult::Blocked(reason) => {
                                    let msg = format!("🚫 **Command Blocked**: {}", reason);
                                    let _ = room_clone.send_markdown(&msg).await;
                                    msg
                                }
                                crate::sandbox::PermissionResult::Ask(_reason) => {
                                    // 1. Save pending command & history
                                    let mut bot_state = state.lock().await;
                                    let room_state =
                                        bot_state.get_room_state(room.room_id().as_str());
                                    room_state.pending_command = Some(content.clone());
                                    room_state.pending_agent_response = Some(response.clone());
                                    room_state.execution_history =
                                        Some(conversation_history.clone());
                                    bot_state.save();

                                    // 2. Notify
                                    let msg = crate::prompts::STRINGS
                                        .messages
                                        .command_approval_request
                                        .replace("{}", &content);
                                    let _ = room_clone.send_markdown(&msg).await;

                                    // 3. Pause Execution - return from entire function?
                                    // "return" here exits the match arm, but we need to exit the interactive loop.
                                    // If we are in a loop, we need to return from the async block/function.
                                    // Since we are iterating actions, if we pause, we should probably stop processing further actions?
                                    // Yes, if blocked on approval, we stop.
                                    return;
                                }
                            };

                            // Log full output for debugging
                            crate::util::log_interaction(
                                "SYSTEM_EXEC",
                                "shell",
                                &format!("Command: {}\nOutput:\n{}", content, cmd_result),
                            );

                            let display_output = if cmd_result.len() > 1000 {
                                crate::prompts::STRINGS
                                    .messages
                                    .output_truncated
                                    .replace("{}", &cmd_result[..1000])
                            } else {
                                cmd_result.clone()
                            };
                            let _ = room_clone
                                .send_markdown(
                                    &crate::prompts::STRINGS
                                        .messages
                                        .agent_output
                                        .replace("{}", &display_output),
                                )
                                .await;

                            // (Clean response logic moved up)

                            // Update history for THIS action (System output only)
                            conversation_history
                                .push_str(&format!("\n\nSystem Command Output: {}", cmd_result));

                            // Persist history after each command
                            {
                                let mut bot_state = state.lock().await;
                                let room_state = bot_state.get_room_state(room.room_id().as_str());
                                room_state.execution_history = Some(conversation_history.clone());
                                bot_state.save();
                            }
                        }
                        crate::util::AgentAction::ChangeDir(path) => {
                            // Resolve path relative to current working dir
                            let new_path = if path.starts_with('/') {
                                path.clone()
                            } else {
                                if let Some(wd) = &working_dir {
                                    format!("{}/{}", wd, path)
                                } else {
                                    path.clone()
                                }
                            };

                            // Verify path exists and is a directory
                            if std::path::Path::new(&new_path).is_dir() {
                                // Canonicalize to clean up .. and .
                                let canonical_path = std::fs::canonicalize(&new_path)
                                    .map(|p| p.to_string_lossy().to_string())
                                    .unwrap_or(new_path.clone());

                                // Update local working_dir
                                working_dir = Some(canonical_path.clone());

                                // Update RoomState for persistence
                                {
                                    let mut bot_state = state.lock().await;
                                    let room_state =
                                        bot_state.get_room_state(room.room_id().as_str());
                                    room_state.current_project_path = Some(canonical_path.clone());
                                    bot_state.save();
                                }

                                let _ = room_clone
                                    .send_markdown(
                                        &crate::prompts::STRINGS
                                            .messages
                                            .directory_changed
                                            .replace("{}", &canonical_path),
                                    )
                                    .await;
                                conversation_history.push_str(&format!(
                                    "\n\nSystem: Changed directory to `{}`",
                                    canonical_path
                                ));
                            } else {
                                let _ = room_clone
                                    .send_markdown(
                                        &crate::prompts::STRINGS.messages.directory_not_found,
                                    )
                                    .await;
                                conversation_history.push_str(&format!(
                                    "\n\nSystem: Directory not found or not a directory: `{}`",
                                    new_path
                                ));
                            }
                        }
                        crate::util::AgentAction::ReadFile(path) => {
                            let target_path = if path.starts_with('/') {
                                path.clone()
                            } else {
                                if let Some(wd) = &working_dir {
                                    format!("{}/{}", wd, path)
                                } else {
                                    path.clone()
                                }
                            };

                            match fs::read_to_string(&target_path) {
                                Ok(content) => {
                                    // Add line numbers
                                    let numbered_content = content
                                        .lines()
                                        .enumerate()
                                        .map(|(i, line)| format!("{} | {}", i + 1, line))
                                        .collect::<Vec<String>>()
                                        .join("\n");

                                    let output = format!(
                                        "File Content: {}\n```\n{}\n```",
                                        path, numbered_content
                                    );
                                    let _ = room_clone
                                        .send_markdown(
                                            &crate::prompts::STRINGS
                                                .messages
                                                .agent_output
                                                .replace("{}", &output),
                                        )
                                        .await;
                                    conversation_history
                                        .push_str(&format!("\n\nSystem: {}", output));
                                }
                                Err(e) => {
                                    let output = format!("Failed to read file `{}`: {}", path, e);
                                    let _ = room_clone
                                        .send_markdown(
                                            &crate::prompts::STRINGS
                                                .messages
                                                .agent_error
                                                .replace("{}", &output),
                                        )
                                        .await;
                                    conversation_history
                                        .push_str(&format!("\n\nSystem: {}", output));
                                }
                            }
                        }
                        crate::util::AgentAction::ListDir(path) => {
                            let target_path = if path.starts_with('/') {
                                path.clone()
                            } else {
                                if let Some(wd) = &working_dir {
                                    format!("{}/{}", wd, path)
                                } else {
                                    path.clone()
                                }
                            };

                            match fs::read_dir(&target_path) {
                                Ok(entries) => {
                                    let mut list = String::new();
                                    for entry in entries {
                                        if let Ok(entry) = entry {
                                            let file_name = entry.file_name();
                                            let name_str = file_name.to_string_lossy();
                                            // Skip hidden files/git
                                            if name_str.starts_with('.') {
                                                continue;
                                            }

                                            let is_dir = entry
                                                .file_type()
                                                .map(|t| t.is_dir())
                                                .unwrap_or(false);
                                            let suffix = if is_dir { "/" } else { "" };
                                            list.push_str(&format!("{}{}\n", name_str, suffix));
                                        }
                                    }
                                    if list.is_empty() {
                                        list.push_str("(empty directory)");
                                    }

                                    let output =
                                        format!("Directory Listing: {}\n```\n{}\n```", path, list);
                                    let _ = room_clone
                                        .send_markdown(
                                            &crate::prompts::STRINGS
                                                .messages
                                                .agent_output
                                                .replace("{}", &output),
                                        )
                                        .await;
                                    conversation_history
                                        .push_str(&format!("\n\nSystem: {}", output));
                                }
                                Err(e) => {
                                    let output =
                                        format!("Failed to list directory `{}`: {}", path, e);
                                    let _ = room_clone
                                        .send_markdown(
                                            &crate::prompts::STRINGS
                                                .messages
                                                .agent_error
                                                .replace("{}", &output),
                                        )
                                        .await;
                                    conversation_history
                                        .push_str(&format!("\n\nSystem: {}", output));
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let _ = room_clone
                    .send_plain(&format!("⚠️ Agent error: {}", e))
                    .await;
                break;
            }
        }
    }
}

/// Stops the current interactive execution loop.
pub async fn handle_stop(state: Arc<Mutex<BotState>>, room: &impl ChatService) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    room_state.stop_requested = true;

    // Trigger abort signal if present
    if let Some(handle) = &room_state.abort_handle {
        let _ = handle.send(true);
    }

    bot_state.save();
    let _ = room
        .send_markdown(&crate::prompts::STRINGS.messages.stop_request_wait)
        .await;
}

/// Approves the current plan and executes it using an agent in an interactive loop.
pub async fn handle_approve<S: ChatService + Clone + Send + 'static>(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    room: &S,
) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    if let Some(task) = &room_state.active_task {
        let working_dir = room_state.current_project_path.clone();
        let active_agent = room_state.active_agent.clone();
        let active_model = room_state.active_model.clone();
        let task_desc = task.clone();

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

        let initial_history = crate::prompts::STRINGS
            .prompts
            .initial_history_context
            .replace("{TASK}", &task_desc)
            .replace("{PLAN}", &plan)
            .replace("{TASKS}", &tasks)
            .replace("{WORKDIR}", working_dir.as_deref().unwrap_or("unknown"));

        // Initialize state history
        room_state.execution_history = Some(initial_history.clone());
        bot_state.save();

        let room_clone = room.clone();
        let config_clone = config.clone();
        let state_clone = state.clone();

        tokio::spawn(async move {
            let _ = room_clone
                .send_markdown(
                    &crate::prompts::STRINGS
                        .messages
                        .plan_approved
                        .replace("{}", &task_desc),
                )
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
            .send_markdown(&crate::prompts::STRINGS.messages.no_task_approve)
            .await;
    }
}

/// Resumes the interactive execution loop from where it left off.
pub async fn handle_continue<S: ChatService + Clone + Send + 'static>(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    room: &S,
) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());

    // Reset circuit breaker state on manual resume
    room_state.stop_requested = false;
    room_state.command_retry_count = 0;
    room_state.last_command = None; // clear this too so it doesn't match the very first command if identical

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
                .send_markdown(&crate::prompts::STRINGS.messages.resuming_execution)
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
            .send_markdown(&crate::prompts::STRINGS.messages.no_history_continue)
            .await;
    }
}

/// Unified start command.
/// If history exists, it continues. If not, it approves/starts the current task.
pub async fn handle_start<S: ChatService + Clone + Send + 'static>(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    room: &S,
) {
    let should_continue = {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());
        room_state.execution_history.is_some()
    };

    if should_continue {
        handle_continue(config, state, room).await;
    } else {
        handle_approve(config, state, room).await;
    }
}

/// Sends a direct chat message to the active agent.
pub async fn handle_ask<S: ChatService + Clone + Send + 'static>(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    argument: &str,
    room: &S,
) {
    use crate::message_helper::MessageHelper;

    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());

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
        let _ = room_clone.typing(true).await;

        let agent_name = resolve_agent_name(active_agent.as_deref(), &config_clone);

        let helper = MessageHelper::new(room_clone.room_id());
        let state_clone = state.clone();

        let callback_room = room_clone.clone();
        let callback_helper = helper.clone();
        let callback_state = state.clone();
        let context = AgentContext {
            prompt,
            working_dir: working_dir.clone(),
            model: active_model,
            status_callback: Some(std::sync::Arc::new(move |msg| {
                let r = callback_room.clone();
                let h = callback_helper.clone();
                let s = callback_state.clone();
                tokio::spawn(async move {
                    let mut st = s.lock().await;
                    // Use send_or_edit to update the same message
                    let _ = h.send_or_edit_markdown(&r, &mut st, &msg, false).await;
                });
            })),
            abort_signal: None,
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
            Err(e) => crate::prompts::STRINGS
                .messages
                .agent_error
                .replace("{}", &e),
        };

        // Send final response as a new message
        let mut st = state_clone.lock().await;
        let _ = helper
            .send_or_edit_markdown(&room_clone, &mut st, &response, true)
            .await;
        let _ = room_clone.typing(false).await;
    });
}

/// Rejects the current plan and clears the active task for the current room.
pub async fn handle_reject(state: Arc<Mutex<BotState>>, room: &impl ChatService) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    room_state.active_task = None;
    room_state.execution_history = None;
    bot_state.save();
    let _ = room
        .send_markdown(&crate::prompts::STRINGS.messages.plan_rejected)
        .await;
}

/// Shows current git changes in the active project.
pub async fn handle_changes(state: Arc<Mutex<BotState>>, room: &impl ChatService) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    let response = match run_command("git diff", room_state.current_project_path.as_deref()).await {
        Ok(o) => o,
        Err(e) => e,
    };
    let _ = room
        .send_markdown(
            &crate::prompts::STRINGS
                .messages
                .current_changes_header
                .replace("{}", &response),
        )
        .await;
}

/// Commits changes in the active project.
pub async fn handle_commit(state: Arc<Mutex<BotState>>, argument: &str, room: &impl ChatService) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    if argument.is_empty() {
        let _ = room
            .send_markdown(&crate::prompts::STRINGS.messages.please_commit_msg)
            .await;
    } else {
        // Note: Git command construction is internal info, kept as format!
        let cmd = format!("git add . && git commit -m \"{}\"", argument);
        let resp = match run_command(&cmd, room_state.current_project_path.as_deref()).await {
            Ok(o) => o,
            Err(e) => e,
        };
        let _ = room
            .send_markdown(
                &crate::prompts::STRINGS
                    .messages
                    .committed_msg
                    .replace("{}", &resp),
            )
            .await;
    }
}

/// Discards uncommitted changes in the active project.
pub async fn handle_discard(state: Arc<Mutex<BotState>>, room: &impl ChatService) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    let _ = run_command("git checkout .", room_state.current_project_path.as_deref()).await;
    let _ = room
        .send_markdown(&crate::prompts::STRINGS.messages.changes_discarded)
        .await;
}

/// Triggers a build of the project.
pub async fn handle_build(
    _config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    room: &impl ChatService,
) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    let cmd = "cargo build";

    let _ = room
        .send_markdown(&crate::prompts::STRINGS.messages.building_msg)
        .await;
    let response = match run_command(cmd, room_state.current_project_path.as_deref()).await {
        Ok(o) => o,
        Err(e) => e,
    };
    let _ = room
        .send_markdown(
            &crate::prompts::STRINGS
                .messages
                .build_result
                .replace("{}", &response),
        )
        .await;
}

/// Triggers a deployment of the project.
pub async fn handle_deploy(
    _config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    room: &impl ChatService,
) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    // Use standard docker deploy if allowed, or hardcoded default
    let cmd = "docker compose up -d --build";

    let _ = room
        .send_markdown(&crate::prompts::STRINGS.messages.deploying_msg)
        .await;
    let response = match run_command(cmd, room_state.current_project_path.as_deref()).await {
        Ok(o) => o,
        Err(e) => e,
    };
    let _ = room
        .send_markdown(
            &crate::prompts::STRINGS
                .messages
                .deploy_result
                .replace("{}", &response),
        )
        .await;
}

/// Triggers a check of the project (e.g., cargo check).
pub async fn handle_check(
    _config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    room: &impl ChatService,
) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    let cmd = "cargo check";

    let _ = room
        .send_markdown(&crate::prompts::STRINGS.messages.checking_msg)
        .await;
    let response = match run_command(cmd, room_state.current_project_path.as_deref()).await {
        Ok(o) => o,
        Err(e) => e,
    };
    let _ = room
        .send_markdown(
            &crate::prompts::STRINGS
                .messages
                .check_result
                .replace("{}", &response),
        )
        .await;
}

/// Shows current status of the bot.
pub async fn handle_status(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    room: &impl ChatService,
) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    let mut status = String::new();

    let current_path = room_state.current_project_path.as_deref().unwrap_or("None");
    let project_name = crate::util::get_project_name(current_path);

    status.push_str(&crate::prompts::STRINGS.messages.status_header);
    status.push_str(&format!("**Project**: `{}`\n", project_name));
    status.push_str(&format!(
        "**Agent**: `{}` | `{}`\n",
        resolve_agent_name(room_state.active_agent.as_deref(), config),
        room_state.active_model.as_deref().unwrap_or("None")
    ));
    let task_display = if room_state.is_task_completed {
        "None"
    } else {
        room_state.active_task.as_deref().unwrap_or("None")
    };

    status.push_str(&format!("**Task**: `{}`\n\n", task_display));

    let _ = room.send_markdown(&status).await;
}

/// Approves a pending command.
pub async fn handle_ok<S: ChatService + Clone + Send + 'static>(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    room: &S,
) {
    let (pending_cmd, pending_resp, history_opt, working_dir, active_agent, active_model) = {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());

        // Take pending command and response
        let cmd = room_state.pending_command.take();
        let resp = room_state.pending_agent_response.take();

        let history = room_state.execution_history.clone();
        let wd = room_state.current_project_path.clone();
        let agent = room_state.active_agent.clone();
        let model = room_state.active_model.clone();

        if cmd.is_some() {
            bot_state.save();
        }
        (cmd, resp, history, wd, agent, model)
    };

    if let Some(cmd) = pending_cmd {
        if let Some(history) = history_opt {
            let history_clone = history.clone();
            let cmd_clone = cmd.clone();

            let room_clone = room.clone();
            let config_clone = config.clone();
            let state_clone = state.clone();

            tokio::spawn(async move {
                // We need to inject the command execution into the loop or just resume?
                // Actually, run_interactive_loop expects a history string.
                // If we restart the loop, it will prompt the agent again with the history.
                // WE NEED TO MANUALLY RUN THE COMMAND FIRST, THEN UPDATE HISTORY, THEN RESUME LOOP.

                let _ = room_clone
                    .send_markdown(&crate::prompts::STRINGS.messages.resuming_execution)
                    .await;

                // Execute the pending command
                let cmd_result = match crate::util::run_shell_command(
                    &cmd_clone,
                    working_dir.as_deref(),
                )
                .await
                {
                    Ok(o) => o,
                    Err(e) => e,
                };

                // Log full output
                crate::util::log_interaction(
                    "SYSTEM_EXEC",
                    "shell",
                    &format!("Command (Resumed): {}\nOutput:\n{}", cmd_clone, cmd_result),
                );

                let display_output = if cmd_result.len() > 1000 {
                    crate::prompts::STRINGS
                        .messages
                        .output_truncated
                        .replace("{}", &cmd_result[..1000])
                } else {
                    cmd_result.clone()
                };
                let _ = room_clone
                    .send_markdown(
                        &crate::prompts::STRINGS
                            .messages
                            .agent_output
                            .replace("{}", &display_output),
                    )
                    .await;

                let mut conversation_history = history_clone;

                // RESTORE THE MISSING AGENT MESSAGE
                // The history we saved was from BEFORE the agent spoke.
                // We must append the agent's explanation (that triggered this confirmation)
                // so the bot "remembers" it told us to run this.
                if let Some(agent_text) = pending_resp {
                    conversation_history.push_str(&format!("\nAgent: {}\n", agent_text));
                } else {
                    // Fallback check if it's already in history? Uncommon.
                    // Or reconstruct minimal history
                    conversation_history
                        .push_str(&format!("\nAgent: [Resuming command]: {}\n", cmd_clone));
                }
                // We need to find the last agent message that proposed this command?
                // The agent proposed it, we paused. The agent's proposal IS in the history (last message hopefully).
                // Wait, if we paused, did we add the Agent's message to history?
                // In `run_interactive_loop`:
                // `let _ = room_clone.send_markdown(agent_run_code)...`
                // `PermissionResult::Ask` -> returns.
                // We did NOT update `conversation_history` with the agent's text yet in that function block!
                // The `conversation_history.push_str` happens at the END of the loop iteration.

                // Re-reading `run_interactive_loop`:
                // It calls `extract_code`. If `Some`, it sends `agent_run_code`.
                // Then logic... then `Sandbox Check`... `Ask` returns.
                // The `conversation_history` is LOCAL to the loop until saved.
                // We saved `execution_history = Some(conversation_history.clone())` inside `Ask`.
                // BUT that `conversation_history` DOES NOT contains the latest agent response yet!
                // Because `conversation_history.push_str` happens LATER (line 1092 in original file).

                // SO: We lost the Agent's text that triggered the command?
                // `handle_ok` needs to append:
                // 1. A reconstructed Agent message (since we might have lost the raw response if we didn't save it).
                //    Wait, we save `pending_command`. We don't save `pending_agent_response`.
                //    This uses `extract_code` from `response`.

                // CRITICAL FIX: We need to save the `response` (agent text) too?
                // Or we just fake it: "Agent: <command code block>"

                // Let's modify `run_interactive_loop` to save `last_agent_response` too?
                // PROBABLY BETTER: Append the Agent's part to history BEFORE checking sandbox?
                // But if we block it, we might want to say "Command blocked".

                // Let's append manually here:
                conversation_history.push_str(&format!(
                    "\n\nAgent: [Approved Command]\n```\n{}\n```\n\nSystem Command Output: {}",
                    cmd_clone, cmd_result
                ));

                // Now resume loop
                run_interactive_loop(
                    config_clone,
                    state_clone,
                    room_clone,
                    conversation_history,
                    working_dir,
                    active_agent,
                    active_model,
                )
                .await;
            });
        }
    } else {
        let _ = room
            .send_markdown(&crate::prompts::STRINGS.messages.no_pending_command)
            .await;
    }
}

/// Denies a pending command.
pub async fn handle_no<S: ChatService + Clone + Send + 'static>(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    room: &S,
) {
    let (pending_cmd, history_opt, working_dir, active_agent, active_model) = {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());

        let cmd = room_state.pending_command.take();

        let history = room_state.execution_history.clone();
        let wd = room_state.current_project_path.clone();
        let agent = room_state.active_agent.clone();
        let model = room_state.active_model.clone();

        if cmd.is_some() {
            bot_state.save();
        }
        (cmd, history, wd, agent, model)
    };

    if let Some(_) = pending_cmd {
        if let Some(history) = history_opt {
            let history_clone = history.clone();

            let room_clone = room.clone();
            let config_clone = config.clone();
            let state_clone = state.clone();

            tokio::spawn(async move {
                let _ = room_clone
                    .send_markdown(&crate::prompts::STRINGS.messages.command_denied_user)
                    .await;

                let mut conversation_history = history_clone;
                // Append denial
                conversation_history.push_str(&format!(
                    "\n\nSystem: User denied the command execution. Ask for an alternative or stop."
                ));

                run_interactive_loop(
                    config_clone,
                    state_clone,
                    room_clone,
                    conversation_history,
                    working_dir,
                    active_agent,
                    active_model,
                )
                .await;
            });
        }
    } else {
        let _ = room
            .send_markdown(&crate::prompts::STRINGS.messages.no_pending_command)
            .await;
    }
}
