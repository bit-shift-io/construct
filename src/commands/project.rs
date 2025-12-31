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
    mcp_manager: Option<Arc<crate::mcp::McpManager>>,
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
            crate::commands::wizard::start_new_project_wizard(state.clone(), mcp_manager, room)
                .await;
            return;
        }
    }

    let mut bot_state = state.lock().await;

    let projects_dir = match &config.system.projects_dir {
        Some(dir) => dir,
        None => {
            let _ = room
                .send_markdown(crate::strings::messages::NO_PROJECTS_CONFIGURED)
                .await;
            return;
        }
    };

    let project_name = argument.trim();
    if project_name.is_empty() {
        let _ = room
            .send_markdown(crate::strings::messages::PROVIDE_PROJECT_NAME)
            .await;
        return;
    }

    // Basic sanitization
    if project_name.contains('/') || project_name.contains('\\') || project_name.starts_with('.') {
        let _ = room
            .send_markdown(crate::strings::messages::INVALID_PROJECT_NAME)
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
                        &crate::strings::messages::project_exists(&final_path),
                )
                .await;
            return;
        }
    }

    // Create
    let mut response = String::new();

    let create_result = if let Some(mcp) = &mcp_manager {
        // Use MCP client
        let client = mcp.client();
        let mut locked_client = client.lock().await;
        match locked_client.create_directory(&final_path, true).await {
            Ok(()) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    } else {
        // Fallback to direct fs operations
        fs::create_dir_all(&final_path).map_err(|e| e.to_string())
    };

    if let Err(e) = create_result {
        response.push_str(
                &crate::strings::messages::create_dir_failed(&final_path, &e),
        );
    } else {
        // Init specs
        let roadmap_path = format!("{}/roadmap.md", final_path);
        let changelog_path = format!("{}/changelog.md", final_path);

        // Populate with defaults if missing
        if !fs::metadata(&roadmap_path).is_ok() {
            let write_result = if let Some(mcp) = &mcp_manager {
                let client = mcp.client();
                let mut locked_client = client.lock().await;
                locked_client
                    .write_file(
                        &roadmap_path,
                        crate::strings::prompts::ROADMAP_TEMPLATE,
                    )
                    .await
                    .map_err(|e| e.to_string())
            } else {
                fs::write(
                    &roadmap_path,
                    crate::strings::prompts::ROADMAP_TEMPLATE,
                )
                .map_err(|e| e.to_string())
            };

            if let Err(e) = write_result {
                response.push_str(&format!("⚠️ Failed to write roadmap.md: {}\n", e));
            }
        }

        if !fs::metadata(&changelog_path).is_ok() {
            let write_result = if let Some(mcp) = &mcp_manager {
                let client = mcp.client();
                let mut locked_client = client.lock().await;
                locked_client
                    .write_file(
                        &changelog_path,
                        crate::strings::prompts::CHANGELOG_TEMPLATE,
                    )
                    .await
                    .map_err(|e| e.to_string())
            } else {
                fs::write(
                    &changelog_path,
                    crate::strings::prompts::CHANGELOG_TEMPLATE,
                )
                .map_err(|e| e.to_string())
            };

            if let Err(e) = write_result {
                response.push_str(&format!("⚠️ Failed to write changelog.md: {}\n", e));
            }
        }

        let room_state = bot_state.get_room_state(&room.room_id());
        room_state.current_project_path = Some(final_path.clone());
        room_state.active_task = None;
        room_state.is_task_completed = false;
        room_state.cleanup_after_task();
        bot_state.save();

        response.push_str(
                &crate::strings::messages::project_created(&final_path),
        );
    }

    response.push_str(crate::strings::messages::USE_TASK_TO_START);
    let _ = room.send_markdown(&response).await;
}

/// Lists available projects in the configured projects directory.
pub async fn handle_list(
    config: &AppConfig,
    mcp_manager: Option<Arc<crate::mcp::McpManager>>,
    room: &impl ChatService,
) {
    let projects_dir = match &config.system.projects_dir {
        Some(dir) => dir,
        None => {
            let _ = room.send_markdown("⚠️ No `projects_dir` configured.").await;
            return;
        }
    };

    let mut projects = if let Some(mcp) = &mcp_manager {
        // Use MCP client
        let client = mcp.client();
        let mut locked_client = client.lock().await;
        match locked_client.list_directory(None).await {
            Ok(listing) => {
                // Parse directory listing
                listing
                    .lines()
                    .filter_map(|line| {
                        let line = line.trim();
                        if !line.is_empty() && !line.starts_with('.') {
                            Some(line.to_string())
                        } else {
                            None
                        }
                    })
                    .collect()
            }
            Err(e) => {
                let _ = room
                    .send_markdown(&format!("⚠️ Failed to list projects: {}", e))
                    .await;
                return;
            }
        }
    } else {
        // Fallback to direct fs operations
        match fs::read_dir(projects_dir) {
            Ok(entries) => {
                let mut dirs = Vec::new();
                for entry in entries.flatten() {
                    if let Ok(file_type) = entry.file_type() {
                        if file_type.is_dir() {
                            if let Ok(name) = entry.file_name().into_string() {
                                if !name.starts_with('.') {
                                    dirs.push(name);
                                }
                            }
                        }
                    }
                }
                dirs
            }
            Err(e) => {
                let _ = room
                    .send_markdown(&format!("⚠️ Failed to list projects: {}", e))
                    .await;
                return;
            }
        }
    };

    projects.sort();

    if projects.is_empty() {
        let _ = room
            .send_markdown(crate::strings::messages::NO_PROJECTS_FOUND)
            .await;
    } else {
        let mut response = crate::strings::messages::AVAILABLE_PROJECTS_HEADER
            .to_string();
        for project in projects {
            response.push_str(&format!("* `{}`\n", project));
        }
        let _ = room.send_markdown(&response).await;
    }
}

/// Handles project-related commands (setting or viewing current path).
pub async fn handle_project(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    mcp_manager: Option<Arc<crate::mcp::McpManager>>,
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

    let is_dir = if let Some(mcp) = &mcp_manager {
        // Use MCP client to check if path exists and is a directory
        let client = mcp.client();
        let mut locked_client = client.lock().await;
        // Try to list directory - if it succeeds, it's a directory
        match locked_client.list_directory(Some(&path)).await {
            Ok(_) => true,
            Err(_) => false,
        }
    } else {
        // Fallback to direct fs operations
        fs::metadata(&path).map(|m| m.is_dir()).unwrap_or(false)
    };

    if is_dir {
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
            .send_markdown(&format!(
                "⚠️ `{}` is not a directory or does not exist.",
                path
            ))
            .await;
    }
}

/// Reads one or more files and prints their contents.
pub async fn handle_read(
    state: Arc<Mutex<BotState>>,
    mcp_manager: Option<Arc<crate::mcp::McpManager>>,
    argument: &str,
    room: &impl ChatService,
) {
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

        let read_result = if let Some(mcp) = &mcp_manager {
            // Use MCP client
            let client = mcp.client();
            let mut locked_client = client.lock().await;
            locked_client
                .read_file(&path)
                .await
                .map_err(|e| e.to_string())
        } else {
            // Fallback to direct fs operations
            fs::read_to_string(&path).map_err(|e| e.to_string())
        };

        match read_result {
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

/// Handles `.set` command, supporting generic key-value pairs.
pub async fn handle_set(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    mcp_manager: Option<Arc<crate::mcp::McpManager>>,
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
        "project" | "workdir" => handle_project(config, state, mcp_manager, value, room).await,
        "agent" => {
            let mut bot_state = state.lock().await;
            let room_state = bot_state.get_room_state(&room.room_id());
            room_state.active_agent = Some(value.to_string());
            bot_state.save();
            let _ = room
                .send_markdown(
                        &crate::strings::messages::model_set(value),
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
