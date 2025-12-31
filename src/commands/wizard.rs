use crate::core::config::AppConfig;
use crate::core::feed::FeedManager;
use crate::services::ChatService;
use crate::core::state::{BotState, WizardMode, WizardState, WizardStep};
use crate::core::feed_utils;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing;

pub async fn start_new_project_wizard<S: ChatService + Clone + Send + 'static>(
    state: Arc<Mutex<BotState>>,
    #[allow(unused_variables)] mcp_manager: Option<Arc<crate::mcp::McpManager>>,
    room: &S,
) {
    let (mut feed_manager, initial_msg) = {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());

        // Initialize FeedManager
        let feed = FeedManager::new(room_state.current_project_path.clone());
        room_state.feed_manager = Some(feed.clone());

        room_state.wizard = WizardState {
            active: true,
            mode: WizardMode::Project,
            step: Some(WizardStep::ProjectName),
            data: std::collections::HashMap::new(),
            buffer: String::new(),
        };

        let wizard_data = room_state.wizard.data.clone();

        bot_state.save();

        let msg = feed_utils::format_wizard_step(
            &WizardStep::ProjectName,
            &WizardMode::Project,
            "",
            &wizard_data,
        );

        (feed, msg)
    };

    // Send initial wizard message and store event ID
    if let Err(e) = feed_utils::start_feed(room, &mut feed_manager, &initial_msg).await {
        tracing::error!("Failed to start feed: {}", e);
        return;
    }

    // Store updated feed manager with event ID
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    room_state.feed_manager = Some(feed_manager);
    bot_state.save();
}

pub async fn start_task_wizard<S: ChatService + Clone + Send + 'static>(
    state: Arc<Mutex<BotState>>,
    #[allow(unused_variables)] mcp_manager: Option<Arc<crate::mcp::McpManager>>,
    room: &S,
) {
    let (mut feed_manager, initial_msg) = {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());

        // Initialize FeedManager
        let feed = FeedManager::new(room_state.current_project_path.clone());
        room_state.feed_manager = Some(feed.clone());

        room_state.wizard = WizardState {
            active: true,
            mode: WizardMode::Task,
            step: Some(WizardStep::TaskDescription),
            data: std::collections::HashMap::new(),
            buffer: String::new(),
        };

        let wizard_data = room_state.wizard.data.clone();

        bot_state.save();

        let msg = feed_utils::format_wizard_step(
            &WizardStep::TaskDescription,
            &WizardMode::Task,
            "",
            &wizard_data,
        );

        (feed, msg)
    };

    // Send initial wizard message and store event ID
    if let Err(e) = feed_utils::start_feed(room, &mut feed_manager, &initial_msg).await {
        tracing::error!("Failed to start feed: {}", e);
        return;
    }

    // Store updated feed manager with event ID
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    room_state.feed_manager = Some(feed_manager);
    bot_state.save();
}

pub async fn handle_input<S: ChatService + Clone + Send + 'static>(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    mcp_manager: Option<Arc<crate::mcp::McpManager>>,
    room: &S,
    input: &str,
) {
    // 1. Check control commands
    if input == ".cancel" {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());
        room_state.wizard = WizardState::default(); // Reset
        room_state.feed_manager = None; // Clear feed
        bot_state.save();
        let _ = room
            .send_markdown(crate::strings::wizard::CANCELLED)
            .await;
        return;
    }

    // Check for explicit restart
    if input == ".new" {
        start_new_project_wizard(state.clone(), mcp_manager, room).await;
        return;
    }

    // Check for task restart
    if input == ".task" {
        start_task_wizard(state.clone(), mcp_manager, room).await;
        return;
    }

    let (step, _mode) = {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());
        (
            room_state.wizard.step.clone(),
            room_state.wizard.mode.clone(),
        )
    };

    match step {
        Some(WizardStep::ProjectName) => {
            let name = input.trim().to_string();
            if name.is_empty() {
                let _ = room
                    .send_markdown("Please enter a valid project name or .cancel")
                    .await;
                return;
            }
            update_data(state.clone(), room, "name", &name).await;
            // SKIP Type/Stack, go straight to Description
            advance_step(state.clone(), room, WizardStep::Description).await;
        }
        // REMOVED: ProjectType and Stack steps
        Some(WizardStep::ProjectType) | Some(WizardStep::Stack) => {
            // Fallback if state somehow gets here
            advance_step(state.clone(), room, WizardStep::Description).await;
        }
        Some(WizardStep::Description) => {
            if input.trim() == ".ok" {
                // Finalize description
                // SKIP Confirmation, go straight to finish
                finish_wizard(config, state.clone(), mcp_manager, room).await;
            } else {
                // Append to buffer
                append_buffer(state.clone(), room, input).await;
            }
        }
        Some(WizardStep::TaskDescription) => {
            if input.trim() == ".ok" {
                // Finalize
                advance_step(state.clone(), room, WizardStep::Confirmation).await;
            } else {
                append_buffer(state.clone(), room, input).await;
            }
        }
        Some(WizardStep::Confirmation) => {
            if input.trim() == ".ok" {
                // Trigger generation
                finish_wizard(config, state.clone(), mcp_manager, room).await;
            } else {
                let _ = room
                    .send_markdown("Type .ok to generate or .cancel to abort.")
                    .await;
            }
        }
        None => {}
    }
}

async fn update_data<S: ChatService>(state: Arc<Mutex<BotState>>, room: &S, key: &str, val: &str) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    room_state
        .wizard
        .data
        .insert(key.to_string(), val.to_string());
    bot_state.save();
}

async fn append_buffer<S: ChatService>(state: Arc<Mutex<BotState>>, room: &S, input: &str) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    if !room_state.wizard.buffer.is_empty() {
        room_state.wizard.buffer.push('\n');
    }
    room_state.wizard.buffer.push_str(input);
    bot_state.save();
}

async fn advance_step<S: ChatService + Clone + Send + 'static>(
    state: Arc<Mutex<BotState>>,
    room: &S,
    next_step: WizardStep,
) {
    {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());
        room_state.wizard.step = Some(next_step);
        bot_state.save();
    }
    render_step(state, room).await;
}

async fn render_step<S: ChatService + Clone + Send + 'static>(
    state: Arc<Mutex<BotState>>,
    room: &S,
) {
    let (step, mode, buffer, data, mut feed_manager) = {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());

        let feed = match &room_state.feed_manager {
            Some(f) => f.clone(),
            None => {
                // Create new feed manager if none exists
                let new_feed = FeedManager::new(room_state.current_project_path.clone());
                room_state.feed_manager = Some(new_feed.clone());
                new_feed
            }
        };

        (
            room_state.wizard.step.clone(),
            room_state.wizard.mode.clone(),
            room_state.wizard.buffer.clone(),
            room_state.wizard.data.clone(),
            feed,
        )
    };

    let msg = feed_utils::format_wizard_step(
        &step.unwrap_or(WizardStep::ProjectName),
        &mode,
        &buffer,
        &data,
    );

    // Update feed message (edit existing or send new)
    if let Err(e) = feed_utils::update_feed_message(room, &mut feed_manager, &msg).await {
        tracing::error!("Failed to update feed message: {}", e);
        return;
    }

    // Save updated feed manager with event ID
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    room_state.feed_manager = Some(feed_manager);
    bot_state.save();
}

async fn finish_wizard<S: ChatService + Clone + Send + 'static>(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    mcp_manager: Option<Arc<crate::mcp::McpManager>>,
    room: &S,
) {
    let mode = {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());
        room_state.wizard.mode.clone()
    };

    if mode == WizardMode::Task {
        let desc = {
            let mut bot_state = state.lock().await;
            let room_state = bot_state.get_room_state(&room.room_id());
            let desc = room_state.wizard.buffer.clone();
            room_state.wizard = WizardState::default();
            bot_state.save();
            desc
        };

        let prompt = crate::strings::prompts::task_requirements_prompt(&desc);
        crate::commands::handle_task(config, state.clone(), mcp_manager.clone(), &prompt, room)
            .await;
        return;
    }

    let (name, desc) = {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());
        let d = &room_state.wizard.data;
        let desc = room_state.wizard.buffer.clone();

        let n = d.get("name").unwrap_or(&"unnamed".to_string()).clone();

        // Reset wizard
        room_state.wizard = WizardState::default();
        bot_state.save();

        (n, desc)
    };

    // 1. Create Project Dir
    // Replaced create_new_project with handle_new
    crate::commands::handle_new(config, state.clone(), mcp_manager.clone(), &name, room).await;

    // Retrieve the just-created project path
    let (project_path, projects_dir) = {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());
        (
            room_state
                .current_project_path
                .clone()
                .unwrap_or_else(|| ".".to_string()),
            config.system.projects_dir.clone().unwrap_or_default(),
        )
    };

    // Sanitize path to be relative to projects directory (fake sandbox path)
    // We want /r8 (root of projects dir) so we strip projects_dir
    let display_path = if !projects_dir.is_empty() {
        let prefix_to_strip = projects_dir.clone();

        if !prefix_to_strip.is_empty() && project_path.starts_with(&prefix_to_strip) {
            project_path[prefix_to_strip.len()..].to_string()
        } else {
            project_path.clone()
        }
    } else {
        project_path
    };

    // Ensure it looks like a path
    let display_path = if display_path.is_empty() {
        "/".to_string()
    } else {
        display_path
    };

    // Construct the task arguments for the agent
    let prompt = crate::strings::prompts::new_project_prompt(&name, &desc, &display_path);

    // 2. Start Task
    crate::commands::handle_task(config, state.clone(), mcp_manager.clone(), &prompt, room).await;
}
