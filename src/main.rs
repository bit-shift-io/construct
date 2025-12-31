#![recursion_limit = "256"]
mod commands;
mod core;
mod llm;
mod mcp;
mod patterns;
mod services;
mod strings;

use anyhow::{Context, Result};
use matrix_sdk::{
    Client,
    config::SyncSettings,
    room::Room,
    ruma::{
        RoomId,
        events::room::{
            member::{MembershipState, StrippedRoomMemberEvent},
            message::SyncRoomMessageEvent,
        },
    },
};
use std::fs;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;
use tracing;
use tracing_subscriber;


use crate::core::bridge::BridgeManager;
use crate::core::config::AppConfig;
use crate::core::state::BotState;
use crate::mcp::McpManager;
use crate::services::matrix::MatrixService;
use crate::strings::logs;
use std::time::SystemTime;

/// Static configuration and state managers.
/// Using OnceLock for safe global access.
static CONFIG: OnceLock<AppConfig> = OnceLock::new();
static BRIDGE_MANAGER: OnceLock<Arc<BridgeManager>> = OnceLock::new();
static MCP_MANAGER: OnceLock<Option<Arc<McpManager>>> = OnceLock::new();

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing subscriber for logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("info")
                    // Filter out verbose Matrix SDK logs
                    .add_directive("matrix_sdk=warn".parse().unwrap())
                    .add_directive("hyper=warn".parse().unwrap())
                    // Filter out sync callback spam
                    .add_directive("sync_with_callback=off".parse().unwrap())
                    // Filter out backup warnings
                    .add_directive("backup=off".parse().unwrap())
                    // Keep important Matrix connection logs
                    .add_directive("matrix_sdk::client=info".parse().unwrap())
            }),
        )
        .with_target(false)
        .with_level(true)
        .init();

    // 1. Initial configuration loading
    let config_content =
        fs::read_to_string("data/config.yaml").context(logs::CONFIG_READ_ERROR)?;

    let config: AppConfig =
        serde_yaml::from_str(&config_content).context(logs::CONFIG_PARSE_ERROR)?;

    // Clear agent.log on startup for fresh debugging session
    let _ = fs::write(
        "data/agent.log",
        logs::agent_session_start(&chrono::Local::now().to_rfc3339()),
    );

    // 2. Initialize global state and manager
    let state = Arc::new(Mutex::new(BotState::load()));

    // Initialize MCP Manager (optional - will continue without it if it fails)
    let mcp_manager = match McpManager::new(
        &config.mcp.server_path,
        &config.mcp.allowed_directories,
        config.mcp.readonly,
    )
    .await
    {
        Ok(manager) => {
            tracing::info!(
                "{}",
                logs::mcp_started(&config.mcp.allowed_directories)
            );
            Some(Arc::new(manager))
        }
        Err(e) => {
            tracing::error!("{}", logs::mcp_failed(&e.to_string()));
            tracing::warn!("{}", logs::MCP_START_FAIL_WARN);
            None
        }
    };

    MCP_MANAGER.set(mcp_manager.clone()).ok();

    let bridge_manager = Arc::new(BridgeManager::new(
        config.clone(),
        state.clone(),
        mcp_manager.clone(),
    ));

    CONFIG.set(config.clone()).ok();
    BRIDGE_MANAGER.set(bridge_manager).ok();

    tracing::info!(
        "{}",
        logs::config_loaded(&config.services.matrix.username)
    );

    // 3. Setup Matrix Client

    // 3. Setup Matrix Client
    let client = Client::builder()
        .homeserver_url(&config.services.matrix.homeserver)
        .build()
        .await?;

    // 4. Authenticate
    client
        .matrix_auth()
        .login_username(
            &config.services.matrix.username,
            &config.services.matrix.password,
        )
        .send()
        .await?;

    tracing::info!("{}", logs::LOGIN_SUCCESS);

    // 5. Update Display Name if configured
    if let Some(display_name) = &config.services.matrix.display_name {
        tracing::info!(
            "{}",
            logs::setting_display_name(display_name)
        );
        if let Err(e) = client.account().set_display_name(Some(display_name)).await {
            tracing::error!(
                "{}",
                logs::set_display_name_fail(&e.to_string())
            );
        }
    }

    // 6. Register Event Handlers
    // Invitations: handled locally in main or moved to bridge if needed
    client.add_event_handler(|ev: StrippedRoomMemberEvent, room: Room| async move {
        handle_invites(ev, room).await
    });

    // Messages: delegated to BridgeManager
    let start_time = SystemTime::now();
    client.add_event_handler(move |ev: SyncRoomMessageEvent, room: Room| async move {
        if let Some(manager) = BRIDGE_MANAGER.get() {
            // Ensure we only process messages from rooms we have joined
            if room.state() != matrix_sdk::RoomState::Joined {
                return;
            }
            let SyncRoomMessageEvent::Original(event) = ev else {
                return;
            };

            // Ignore messages from self
            if event.sender == room.own_user_id() {
                return;
            }

            // Ignore messages sent before the bot started
            let ts = SystemTime::UNIX_EPOCH
                + std::time::Duration::from_millis(event.origin_server_ts.0.into());
            if ts < start_time {
                return;
            }

            let msg_body = event.content.body();
            let sender = event.sender.as_str();

            // Create generic service wrapper
            let service = MatrixService::new(room);

            // Dispatch
            manager.dispatch(&service, sender, msg_body).await;
        }
    });

    // 6. Start Sync Loop
    let sync_client = client.clone();
    let sync_handle = tokio::spawn(async move {
        tracing::info!("{}", logs::SYNC_LOOP_START);
        if let Err(e) = sync_client.sync(SyncSettings::default()).await {
            tracing::error!("{}", logs::sync_loop_fail(&e.to_string()));
        }
    });

    // 7. Initialize Room states from bridges
    setup_bridges(&client, &config, state.clone(), mcp_manager.clone()).await;

    // 8. Graceful Shutdown
    match tokio::signal::ctrl_c().await {
        Ok(()) => tracing::info!("{}", logs::SHUTDOWN),
        Err(err) => tracing::error!("{}", logs::shutdown_fail(&err.to_string())),
    }

    sync_handle.abort();
    Ok(())
}

/// Iterates through configured bridges and joins necessary Matrix rooms.
async fn setup_bridges(
    client: &Client,
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    mcp_manager: Option<Arc<crate::mcp::McpManager>>,
) {
    for (bridge_name, entries) in &config.bridges {
        for entry in entries {
            if entry.service.as_deref() == Some("matrix") {
                if let Some(room_id_str) = &entry.channel {
                    tracing::info!(
                        "{}",
                        logs::bridge_joining(bridge_name, room_id_str)
                    );

                    if let Ok(room_id) = RoomId::parse(room_id_str) {
                        if let Err(e) = client.join_room_by_id(&room_id).await {
                            tracing::error!(
                                "{}",
                                logs::bridge_join_fail(room_id_str, &e.to_string())
                            );
                        } else if let Some(room) = client.get_room(&room_id) {
                            tracing::info!(
                                "{}",
                                logs::bridge_join_success(room_id_str)
                            );

                            // Send status message instead of welcome message
                            let service = MatrixService::new(room);
                            commands::handle_status(
                                &config,
                                state.clone(),
                                mcp_manager.clone(),
                                &service,
                            )
                            .await;
                        }
                    }
                }
            }
        }
    }
}

/// Handles incoming room invitations.
async fn handle_invites(event: StrippedRoomMemberEvent, room: Room) {
    if event.content.membership == MembershipState::Invite {
        tracing::info!(
            "{}",
            logs::invite_received(&format!("{:?}", room.room_id()))
        );
        if let Err(e) = room.join().await {
            tracing::error!("{}", logs::join_invite_fail(&e.to_string()));
        } else {
            tracing::info!("{}", logs::JOIN_INVITE_SUCCESS);
        }
    }
}
