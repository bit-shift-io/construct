use crate::config::AppConfig;
use crate::services::ChatService;
use crate::state::BotState;
use std::fs;
use std::sync::Arc;
use tokio::sync::Mutex;

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
            crate::commands::wizard::start_new_project_wizard(state.clone(), room).await;
            return;
        }
    }

    let mut bot_state = state.lock().await;

    let projects_dir = match &config.system.projects_dir {
        Some(dir) => dir,
        None => {
            let _ = room
                .send_markdown(&crate::strings::STRINGS.messages.no_projects_configured)
                .await;
            return;
        }
    };

    let project_name = argument.trim();
    if project_name.is_empty() {
        let _ = room
            .send_markdown(&crate::strings::STRINGS.messages.provide_project_name)
            .await;
        return;
    }

    // Basic sanitization
    if project_name.contains('/') || project_name.contains('\\') || project_name.starts_with('.') {
        let _ = room
            .send_markdown(&crate::strings::STRINGS.messages.invalid_project_name)
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
            room_state.cleanup_after_task();
            bot_state.save();

            let _ = room
                .send_markdown(
                    &crate::strings::STRINGS
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
            &crate::strings::STRINGS
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
                &crate::strings::STRINGS.prompts.roadmap_template,
            );
        }
        if !fs::metadata(&changelog_path).is_ok() {
            let _ = fs::write(
                &changelog_path,
                &crate::strings::STRINGS.prompts.changelog_template,
            );
        }

        let room_state = bot_state.get_room_state(&room.room_id());
        room_state.current_project_path = Some(final_path.clone());
        room_state.active_task = None;
        room_state.is_task_completed = false;
        room_state.cleanup_after_task();
        bot_state.save();

        response.push_str(
            &crate::strings::STRINGS
                .messages
                .project_created
                .replace("{}", &final_path),
        );
    }

    response.push_str(&crate::strings::STRINGS.messages.use_task_to_start);
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
            .send_markdown(&crate::strings::STRINGS.messages.no_projects_found)
            .await;
    } else {
        let mut response = crate::strings::STRINGS
            .messages
            .available_projects_header
            .to_string();
        for project in projects {
            response.push_str(&format!("* `{}`\n", project));
        }
        let _ = room.send_markdown(&response).await;
    }
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
                let name = crate::utils::get_project_name(path);
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
            // execution_history removed - now stored in project/state.md
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
                    &crate::strings::STRINGS
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
