use crate::services::ChatService;
use crate::state::BotState;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Shows current git changes in the active project.
/// Shows uncommitted changes using `git diff`.
pub async fn handle_changes(
    state: Arc<Mutex<BotState>>,
    mcp_manager: Option<Arc<crate::mcp::McpManager>>,
    room: &impl ChatService,
) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    let working_dir = room_state.current_project_path.as_deref();
    let response = if let Some(mcp) = &mcp_manager {
        // Use MCP client
        let client = mcp.client();
        let mut locked_client = client.lock().await;
        match locked_client
            .execute_command("git diff", Some(30), working_dir)
            .await
        {
            Ok(o) => o,
            Err(e) => e.to_string(),
        }
    } else {
        // Fallback to direct execution
        match crate::utils::run_command("git diff", working_dir).await {
            Ok(o) => o,
            Err(e) => e,
        }
    };
    let _ = room
        .send_markdown(
            &crate::strings::messages::current_changes_header(&response),
        )
        .await;
}

/// Commits changes in the active project.
/// Commits changes with a message.
pub async fn handle_commit(
    state: Arc<Mutex<BotState>>,
    mcp_manager: Option<Arc<crate::mcp::McpManager>>,
    argument: &str,
    room: &impl ChatService,
) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    let working_dir = room_state.current_project_path.as_deref();
    if argument.is_empty() {
        let _ = room
            .send_markdown(crate::strings::messages::PLEASE_COMMIT_MSG)
            .await;
    } else {
        // Note: Git command construction is internal info, kept as format!
        let cmd = format!("git add . && git commit -m \"{}\"", argument);
        let resp = if let Some(mcp) = &mcp_manager {
            // Use MCP client
            let client = mcp.client();
            let mut locked_client = client.lock().await;
            match locked_client
                .execute_command(&cmd, Some(30), working_dir)
                .await
            {
                Ok(o) => o,
                Err(e) => e.to_string(),
            }
        } else {
            // Fallback to direct execution
            match crate::utils::run_command(&cmd, working_dir).await {
                Ok(o) => o,
                Err(e) => e,
            }
        };
        let _ = room
            .send_markdown(
                &crate::strings::messages::committed_msg(&resp),
            )
            .await;
    }
}

/// Discards uncommitted changes in the active project.
/// Discards uncommitted changes.
pub async fn handle_discard(
    state: Arc<Mutex<BotState>>,
    mcp_manager: Option<Arc<crate::mcp::McpManager>>,
    room: &impl ChatService,
) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    let working_dir = room_state.current_project_path.as_deref();
    if let Some(mcp) = &mcp_manager {
        // Use MCP client
        let client = mcp.client();
        let mut locked_client = client.lock().await;
        let _ = locked_client
            .execute_command("git checkout .", Some(30), working_dir)
            .await;
    } else {
        // Fallback to direct execution
        let _ = crate::utils::run_command("git checkout .", working_dir).await;
    }
    let _ = room
        .send_markdown(crate::strings::messages::CHANGES_DISCARDED)
        .await;
}
