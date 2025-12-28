use crate::util::run_command;
use crate::config::AppConfig;
use crate::state::BotState;
use crate::services::ChatService;
use std::sync::Arc;
use tokio::sync::Mutex;

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
