use crate::agent::{AgentContext, discovery};
use crate::commands::core::execute_with_fallback;
use crate::config::AppConfig;
use crate::services::ChatService;
use crate::services::message_helper::MessageHelper;
use crate::state::BotState;
use std::fs;
use std::sync::Arc;
use tokio::sync::Mutex;

// Helper to resolve the active agent name with smart fallback.
pub fn resolve_agent_name(active_agent: Option<&str>, config: &AppConfig) -> String {
    if let Some(agent) = active_agent {
        return agent.to_string();
    }

    // Defaulting to smart detection if no explicit agent is active

    // Smart Fallbacks based on configured services
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
                    &crate::strings::STRINGS
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
                &crate::strings::STRINGS
                    .messages
                    .model_set
                    .replace("{}", &model),
            )
            .await;
    } else if argument.is_empty() {
        room_state.active_model = None;
        bot_state.save();
        let _ = room
            .send_markdown(&crate::strings::STRINGS.messages.model_reset)
            .await;
    } else {
        let _ = room
            .send_markdown(&crate::strings::STRINGS.messages.invalid_model)
            .await;
    }
}

/// Sends a direct chat message to the active agent.
pub async fn handle_ask<S: ChatService + Clone + Send + 'static>(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    _mcp_manager: Option<Arc<crate::mcp::McpManager>>,
    argument: &str,
    room: &S,
) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());

    let working_dir = room_state.current_project_path.clone();
    let user_question = argument.to_string();
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

        // Build enhanced prompt with project context
        let system_prompt = crate::strings::STRINGS.prompts.system;

        let mut project_context = String::new();
        let mut execution_history = String::new();
        let mut conversation_context = String::new();
        let mut error_patterns_context = String::new();
        let mut feed_context = String::new();

        if let Some(ref wd) = working_dir {
            // Read roadmap.md
            if let Ok(roadmap) = fs::read_to_string(format!("{}/roadmap.md", wd)) {
                project_context = format!("\n\n### Project Roadmap\n{}\n", roadmap);
            }

            // Read tasks.md
            if let Ok(tasks) = fs::read_to_string(format!("{}/tasks.md", wd)) {
                project_context = format!("{}\n\n### Current Tasks\n{}\n", project_context, tasks);
            }

            // Read state.md for execution history AND conversations
            let state_manager = crate::state::project::ProjectStateManager::new(wd.clone());
            if let Ok(history) = state_manager.get_recent_history(5) {
                if !history.contains("No execution history yet") {
                    execution_history = format!("\n\n### Recent Execution History\n{}\n", history);
                }
            }

            // Get recent conversations
            if let Ok(conversations) = state_manager.get_recent_conversations(10) {
                if !conversations.contains("No conversation history yet") {
                    conversation_context =
                        format!("\n\n### Recent Conversations\n{}\n", conversations);
                }
            }

            // Detect error patterns from past failures
            if let Ok(patterns) = state_manager.detect_error_patterns() {
                if !patterns.is_empty() {
                    error_patterns_context = state_manager.format_error_patterns(&patterns);
                }
            }

            // Read feed.md for recent progress
            if let Ok(feed) = fs::read_to_string(format!("{}/feed.md", wd)) {
                if !feed.trim().is_empty() && !feed.contains("**✅ Execution Complete**") {
                    feed_context = format!(
                        "\n\n### Current Task Progress\n{}\n",
                        feed.lines().take(20).collect::<Vec<_>>().join("\n")
                    );
                }
            }
        }

        // Build enhanced prompt with all context including conversations
        let prompt = format!(
            "{}{}{}{}{}{}\n\n### User Question\n{}\n\nPlease answer based on the project context and conversation history above.",
            system_prompt,
            project_context,
            execution_history,
            conversation_context,
            error_patterns_context,
            feed_context,
            user_question
        );

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
            project_state_manager: working_dir.as_ref().map(|p| {
                std::sync::Arc::new(crate::state::project::ProjectStateManager::new(p.clone()))
            }),
        };

        let response = execute_with_fallback(
            &config_clone,
            state.clone(),
            &room_clone,
            context,
            &agent_name,
        )
        .await;

        let final_content = match response {
            Ok(s) => s,
            Err(s) => format!("Error: {}", s),
        };

        // Log conversation to state.md for future context
        if let Some(ref wd) = working_dir {
            let state_manager = crate::state::project::ProjectStateManager::new(wd.clone());
            let _ = state_manager.log_conversation(&user_question, &final_content);
        }

        // Send final response as a new message
        let mut st = state_clone.lock().await;
        let _ = helper
            .send_or_edit_markdown(&room_clone, &mut st, &final_content, true)
            .await;
    });
}
