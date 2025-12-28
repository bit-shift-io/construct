use crate::config::AppConfig;
use crate::state::BotState;

use crate::services::ChatService;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Handles admin terminal commands prefixed with `,`.
/// These commands are NOT sandboxed (except for `cd` restriction if desired, but user asked for full paths, so maybe less restricted?).
/// Actually, the user just said "show full path". The previous logic *did* use `Sandbox` validation for `perm` check (blocked/allowed) but maybe `bridge.rs` logic I just edited still had some of that.
/// Let's reproduce the exact logic from bridge.rs but in a clean function.
pub async fn handle_command(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    room: &impl ChatService,
    sender: &str,
    command_line: &str,
) {
    // Check permissions (case-insensitive)
    let sender_lower = sender.to_lowercase();
    let is_admin = config
        .system
        .admin
        .iter()
        .any(|u| u.to_lowercase() == sender_lower);

    if !is_admin {
        let _ = room
            .send_markdown(
                &crate::strings::STRINGS
                    .messages
                    .admin_permission_denied
                    .replace("{}", sender),
            )
            .await;
        return;
    }

    let command = command_line.trim();
    if command.is_empty() {
        return;
    }

    let _ = room.typing(true).await;

    // Get Current Working Directory
    let cwd = {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());
        room_state.current_project_path.clone()
    };

    // Handle 'cd' command specifically for persistence
    if command.starts_with("cd ") || command == "cd" {
        let target_dir = if command == "cd" {
            // Default to projects_dir or root?
            // Previous logic: config.system.projects_dir.clone().unwrap_or_else(|| ".".to_string())
            // BUT wait, in the REVERTED logic, did I verify what target_dir default was?
            // In bridge.rs:
            // if command == "cd" { "/" } else { ... } -> wait, that was the VIRTUALIZED logic.
            // I need to check what the logic was BEFORE I fully virtualized it, or what is currently in bridge.rs.
            // User asked to "revert sandbox output".
            // In step 442 (view bridge.rs), the logic was:
            // if command == "cd" { "/" } ...
            // projects_root = ...
            // Sandbox::new(projects_root)
            // ...
            // This logic was relying on Sandbox to interpret "/" as root.

            // If I am separating admin commands, I should probably keep the "Safe" `cd` behavior (staying in projects dir)
            // BUT show full paths.
            // The user said: "commands.rs can be for the agent/bot to use which are sandboxed. This seperates admin from agent".
            // This implies admin might NOT be sandboxed?
            // "Separate out the , command ... so commands.rs ... are sandboxed"
            // But earlier: "implement a robust sandboxing ... Restricting the bot's terminal commands to the projects_dir".
            // User likely wants the RESTRICTION to remain, but the OUTPUT to be explicit (full path).
            // So I will keep the Sandbox CHECK, just not the OUTPUT virtualization.

            // Let's re-implement the logic found in `bridge.rs` right now.
            // I will use `sandbox` for checking `cd` validity (jail) but use `canon` (real path) for output.

            config
                .system
                .projects_dir
                .clone()
                .unwrap_or_else(|| ".".to_string())
        } else {
            command[3..].trim().to_string()
        };

        // We use Sandbox logic manually or invoke Sandbox?
        // Let's use the logic provided in bridge.rs which used `std::fs::canonicalize` and checked `starts_with(root)`.

        // Current Real CWD
        let real_cwd = cwd.clone().unwrap_or_else(|| ".".to_string());

        let root_path = std::path::PathBuf::from(
            &config
                .system
                .projects_dir
                .clone()
                .unwrap_or(".".to_string()),
        );
        let root_canon = std::fs::canonicalize(&root_path).unwrap_or(root_path);

        let base = if target_dir.starts_with('/') {
            root_canon.clone()
        } else {
            std::path::PathBuf::from(&real_cwd)
        };

        let new_path = base.join(target_dir);

        if let Ok(canon) = std::fs::canonicalize(&new_path) {
            if canon.starts_with(&root_canon) {
                let mut bot_state = state.lock().await;
                let room_state = bot_state.get_room_state(&room.room_id());
                room_state.current_project_path = Some(canon.to_string_lossy().to_string());
                bot_state.save();

                let _ = room
                    .send_markdown(
                        &crate::strings::STRINGS
                            .messages
                            .directory_changed
                            .replace("{}", &canon.to_string_lossy()),
                    )
                    .await;
            } else {
                let _ = room
                    .send_markdown(&crate::strings::STRINGS.messages.access_denied_sandbox)
                    .await;
            }
        } else {
            let _ = room
                .send_markdown(&crate::strings::STRINGS.messages.directory_not_found)
                .await;
        }

        let _ = room.typing(false).await;
        return;
    }

    let output = match crate::utils::run_shell_command(command, cwd.as_deref()).await {
        Ok(o) => o,
        Err(e) => e,
    };

    let display_output = if output.trim().is_empty() {
        crate::strings::STRINGS.messages.command_no_output
    } else {
        &crate::strings::STRINGS
            .messages
            .code_block_output
            .replace("{}", &output)
    };

    let _ = room.send_markdown(&display_output).await;
    let _ = room.typing(false).await;
}

/// Cleans up the current task context (admin only).
pub async fn handle_cleanup<S: ChatService + Send + Sync>(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    room: &S,
    sender: &str,
) {
    // Check permissions
    let sender_lower = sender.to_lowercase();
    let is_admin = config
        .system
        .admin
        .iter()
        .any(|u| u.to_lowercase() == sender_lower);

    if !is_admin {
        let _ = room
            .send_markdown(
                &crate::strings::STRINGS
                    .messages
                    .admin_permission_denied
                    .replace("{}", sender),
            )
            .await;
        return;
    }

    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());

    room_state.active_task = None;
    room_state.is_task_completed = false;
    room_state.last_message_event_id = None;
    room_state.feed_event_id = None;
    room_state.feed_manager = None;
    room_state.pending_command = None;
    room_state.pending_agent_response = None;
    room_state.stop_requested = false;

    bot_state.save();

    let _ = room
        .send_markdown("🧹 **State Cleaned**: Active task code and feed reset.")
        .await;
}
