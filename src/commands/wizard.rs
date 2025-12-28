use crate::config::AppConfig;
use crate::services::ChatService;
use crate::state::{BotState, WizardMode, WizardState, WizardStep};
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn start_new_project_wizard<S: ChatService + Clone + Send + 'static>(
    state: Arc<Mutex<BotState>>,
    room: &S,
) {
    {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());

        room_state.wizard = WizardState {
            active: true,
            mode: WizardMode::Project,
            step: Some(WizardStep::ProjectName),
            data: std::collections::HashMap::new(),
            buffer: String::new(),
        };

        bot_state.save();
    }

    // Initial Render
    render_step(state.clone(), room).await;
}

pub async fn start_task_wizard<S: ChatService + Clone + Send + 'static>(
    state: Arc<Mutex<BotState>>,
    room: &S,
) {
    {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());

        room_state.wizard = WizardState {
            active: true,
            mode: WizardMode::Task,
            step: Some(WizardStep::TaskDescription),
            data: std::collections::HashMap::new(),
            buffer: String::new(),
        };

        bot_state.save();
    }

    render_step(state.clone(), room).await;
}

pub async fn handle_input<S: ChatService + Clone + Send + 'static>(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    room: &S,
    input: &str,
) {
    // 1. Check control commands
    if input == ".cancel" {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());
        room_state.wizard = WizardState::default(); // Reset
        bot_state.save();
        let _ = room
            .send_markdown(&crate::strings::STRINGS.wizard.cancelled)
            .await;
        return;
    }

    // Check for explicit restart
    if input == ".new" {
        {
            let mut bot_state = state.lock().await;
            let room_state = bot_state.get_room_state(&room.room_id());
            room_state.wizard = WizardState {
                active: true,
                mode: WizardMode::Project,
                step: Some(WizardStep::ProjectName),
                data: std::collections::HashMap::new(),
                buffer: String::new(),
            };
            bot_state.save();
        }
        render_step(state.clone(), room).await;
        return;
    }

    // Check for task restart
    if input == ".task" {
        {
            let mut bot_state = state.lock().await;
            let room_state = bot_state.get_room_state(&room.room_id());
            room_state.wizard = WizardState {
                active: true,
                mode: WizardMode::Task,
                step: Some(WizardStep::TaskDescription),
                data: std::collections::HashMap::new(),
                buffer: String::new(),
            };
            bot_state.save();
        }
        render_step(state.clone(), room).await;
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
                finish_wizard(config, state.clone(), room).await;
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
                finish_wizard(config, state.clone(), room).await;
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

async fn render_step<S: ChatService>(state: Arc<Mutex<BotState>>, room: &S) {
    let (step, buffer_len, mode) = {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());
        (
            room_state.wizard.step.clone(),
            room_state.wizard.buffer.len(),
            room_state.wizard.mode.clone(),
        )
    };

    let msg = match step {
        Some(WizardStep::ProjectName) => crate::strings::STRINGS.wizard.project_name.to_string(),
        Some(WizardStep::ProjectType) => crate::strings::STRINGS.wizard.project_type.to_string(),
        Some(WizardStep::Stack) => crate::strings::STRINGS.wizard.stack.to_string(),
        Some(WizardStep::Description) => {
            if buffer_len == 0 {
                // Modified prompt to include tech stack info
                "**📝 Project Description**\n\nDescribe your project. **Please include the programming language and tech stack you want to use.**\n\nType `.ok` to finish and create the project.".to_string()
            } else {
                return;
            }
        }
        Some(WizardStep::TaskDescription) => {
            if buffer_len == 0 {
                crate::strings::STRINGS.wizard.task_description.to_string()
            } else {
                return;
            }
        }
        Some(WizardStep::Confirmation) => {
            let mut bot_state = state.lock().await;
            let room_state = bot_state.get_room_state(&room.room_id());
            let d = &room_state.wizard.data;
            let desc = &room_state.wizard.buffer;

            if mode == WizardMode::Task {
                format!(
                    "**✅ Confirm Task**\n\n**Requirements**:\n{}\n\nType `.ok` to start task.",
                    desc
                )
            } else {
                format!(
                    "**✅ Review Project**\n\n**Name**: {}\n**Type**: {}\n**Stack**: {}\n**Description**: {} chars\n\nType `.ok` to generate plan.",
                    d.get("name").unwrap_or(&"?".to_string()),
                    d.get("type").unwrap_or(&"?".to_string()),
                    d.get("stack").unwrap_or(&"?".to_string()),
                    desc.len()
                )
            }
        }
        None => return,
    };

    let _ = room.send_markdown(&msg).await;
}

async fn finish_wizard<S: ChatService + Clone + Send + 'static>(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
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

        let prompt = crate::strings::STRINGS
            .prompts
            .task_requirements_prompt
            .replace("{}", &desc);
        crate::commands::handle_task(config, state.clone(), &prompt, room).await;
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
    crate::commands::handle_new(config, state.clone(), &name, room).await;

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
    let prompt = crate::strings::STRINGS
        .prompts
        .new_project_prompt
        .replace("{NAME}", &name)
        .replace("{REQUIREMENTS}", &desc)
        .replace("{WORKDIR}", &display_path);

    // 2. Start Task
    crate::commands::handle_task(config, state.clone(), &prompt, room).await;
}
