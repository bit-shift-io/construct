use crate::config::AppConfig;
use crate::state::BotState;
use crate::util;
use matrix_sdk::room::Room;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Handles admin terminal commands prefixed with `,`.
/// These commands are NOT sandboxed (except for `cd` restriction if desired, but user asked for full paths, so maybe less restricted?).
/// Actually, the user just said "show full path". The previous logic *did* use `Sandbox` validation for `perm` check (blocked/allowed) but maybe `bridge.rs` logic I just edited still had some of that.
/// Let's reproduce the exact logic from bridge.rs but in a clean function.
pub async fn handle_command(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    room: &Room,
    sender: &str,
    command_line: &str,
) {
    // Check permissions (case-insensitive)
    let sender_lower = sender.to_lowercase();
    let is_admin = config.system.admin.iter().any(|u| u.to_lowercase() == sender_lower);

    if !is_admin {
        let _ = room
            .send(RoomMessageEventContent::text_plain(format!(
                "{} you do not have permission to run terminal commands.",
                sender
            )))
            .await;
        return;
    }

    let command = command_line.trim();
    if command.is_empty() {
        return;
    }

    let _ = room.typing_notice(true).await;

    // Get Current Working Directory
    let cwd = {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(room.room_id().as_str());
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
            
             config.system.projects_dir.clone().unwrap_or_else(|| ".".to_string())
        } else {
            command[3..].trim().to_string()
        };

        // We use Sandbox logic manually or invoke Sandbox?
        // Let's use the logic provided in bridge.rs which used `std::fs::canonicalize` and checked `starts_with(root)`.
        
        // Current Real CWD
        let real_cwd = cwd.clone().unwrap_or_else(|| ".".to_string());
        
        let root_path = std::path::PathBuf::from(&config.system.projects_dir.clone().unwrap_or(".".to_string()));
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
                let room_state = bot_state.get_room_state(room.room_id().as_str());
                room_state.current_project_path = Some(canon.to_string_lossy().to_string());
                bot_state.save();
                
                let _ = room
                .send(RoomMessageEventContent::text_markdown(format!(
                    "📂 **Directory changed**: `{}`",
                    canon.to_string_lossy()
                )))
                .await;
            } else {
                 let _ = room
                .send(RoomMessageEventContent::text_plain(
                    "❌ Access denied: Path outside the sandbox."
                ))
                .await;
            }
        } else {
             let _ = room
            .send(RoomMessageEventContent::text_plain(format!(
                "❌ Directory not found."
            )))
            .await;
        }
        
        let _ = room.typing_notice(false).await;
        return;
    }

    // Command Permission Check
    // Admin commands should still respect blocked list? 
    // "Allowed" list is for AGENT/USER?
    // "Admin" usually implies FULL access. 
    // BUT user said "implement command permissions...".
    // "Allowed commands should execute freely... Blocked commands rejected".
    // Does this apply to ADMIN or just Agent?
    // Usually Admin overrides. But for safety, maybe check blocked?
    // The previous code in bridge.rs DID check permissions.
    // I will preserve the existing logic: Check permissions.
    
    let projects_root = config.system.projects_dir.clone().unwrap_or_else(|| ".".to_string());
    let sandbox = crate::sandbox::Sandbox::new(projects_root);
    
    match sandbox.check_command(command, &config.commands) {
        crate::sandbox::PermissionResult::Blocked(msg) => {
            let _ = room
                .send(RoomMessageEventContent::text_plain(format!("⛔ {}", msg)))
                .await;
             let _ = room.typing_notice(false).await;
            return;
        },
        crate::sandbox::PermissionResult::Ask(msg) => {
             let _ = room
                .send(RoomMessageEventContent::text_markdown(format!(
                    "⚠️ **Permission Required**: {}\n\n(Interactive approval not yet implemented. Please add to `allowed` list or use a different command.)", msg
                )))
                .await;
             let _ = room.typing_notice(false).await;
            return;
        },
        crate::sandbox::PermissionResult::Allowed => {}
    }

    let output = match util::run_shell_command(command, cwd.as_deref()).await {
        Ok(o) => o,
        Err(e) => e,
    };

    let display_output = if output.trim().is_empty() {
        "✅ (Command executed successfully, no output)".to_string()
    } else {
        format!("```\n{}\n```", output)
    };

    let _ = room
        .send(RoomMessageEventContent::text_markdown(
            display_output
        ))
        .await;
    let _ = room.typing_notice(false).await;
}
