use crate::util::run_command;
use crate::state::BotState;
use crate::services::ChatService;
use std::sync::Arc;
use tokio::sync::Mutex;

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
