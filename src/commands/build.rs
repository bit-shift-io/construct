use crate::core::config::AppConfig;
use crate::services::ChatService;
use crate::core::state::BotState;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Triggers a build of the project.
pub async fn handle_build(
    _config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    mcp_manager: Option<Arc<crate::mcp::McpManager>>,
    room: &impl ChatService,
) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    let cmd = "cargo build";
    let working_dir = room_state.current_project_path.as_deref();

    let _ = room
        .send_markdown(crate::strings::messages::BUILDING_MSG)
        .await;

    let response = if let Some(mcp) = &mcp_manager {
        // Use MCP client
        let client = mcp.client();
        let mut locked_client = client.lock().await;
        // Long timeout for cargo build (600s)
        match locked_client
            .execute_command(cmd, Some(600), working_dir)
            .await
        {
            Ok(o) => o,
            Err(e) => e.to_string(),
        }
    } else {
        // Fallback to direct execution (if MCP unavailable)
        match crate::core::utils::run_command(cmd, working_dir).await {
            Ok(o) => o,
            Err(e) => e,
        }
    };

    let _ = room
        .send_markdown(
                &crate::strings::messages::build_result(&response),
        )
        .await;
}

/// Triggers a deployment of the project.
pub async fn handle_deploy(
    _config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    mcp_manager: Option<Arc<crate::mcp::McpManager>>,
    room: &impl ChatService,
) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    // Use standard docker deploy if allowed, or hardcoded default
    let cmd = "docker compose up -d --build";
    let working_dir = room_state.current_project_path.as_deref();

    let _ = room
        .send_markdown(crate::strings::messages::DEPLOYING_MSG)
        .await;

    let response = if let Some(mcp) = &mcp_manager {
        // Use MCP client
        let client = mcp.client();
        let mut locked_client = client.lock().await;
        // Medium timeout for docker deploy (120s)
        match locked_client
            .execute_command(cmd, Some(120), working_dir)
            .await
        {
            Ok(o) => o,
            Err(e) => e.to_string(),
        }
    } else {
        // Fallback to direct execution
        match crate::core::utils::run_command(cmd, working_dir).await {
            Ok(o) => o,
            Err(e) => e,
        }
    };

    let _ = room
        .send_markdown(
                &crate::strings::messages::deploy_result(&response),
        )
        .await;
}

/// Triggers a check of the project (e.g., cargo check).
pub async fn handle_check(
    _config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    mcp_manager: Option<Arc<crate::mcp::McpManager>>,
    room: &impl ChatService,
) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    let cmd = "cargo check";
    let working_dir = room_state.current_project_path.as_deref();

    let _ = room
        .send_markdown(crate::strings::messages::CHECKING_MSG)
        .await;

    let response = if let Some(mcp) = &mcp_manager {
        // Use MCP client
        let client = mcp.client();
        let mut locked_client = client.lock().await;
        // Medium timeout for cargo check (120s)
        match locked_client
            .execute_command(cmd, Some(120), working_dir)
            .await
        {
            Ok(o) => o,
            Err(e) => e.to_string(),
        }
    } else {
        // Fallback to direct execution
        match crate::core::utils::run_command(cmd, working_dir).await {
            Ok(o) => o,
            Err(e) => e,
        }
    };

    let _ = room
        .send_markdown(
                &crate::strings::messages::check_result(&response),
        )
        .await;
}
