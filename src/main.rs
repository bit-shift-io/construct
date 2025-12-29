#![recursion_limit = "256"]

mod agent;
mod bridge;
mod commands;
mod config;

mod features;
mod patterns;
mod services;
mod state;
mod utils;

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

use crate::bridge::BridgeManager;
use crate::config::AppConfig;
use crate::services::matrix::MatrixService;
use crate::state::BotState;
use std::time::SystemTime;

/// Static configuration and state managers.
/// Using OnceLock for safe global access.
static CONFIG: OnceLock<AppConfig> = OnceLock::new();
static BRIDGE_MANAGER: OnceLock<Arc<BridgeManager>> = OnceLock::new();

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
        fs::read_to_string("data/config.yaml").context("Failed to read data/config.yaml")?;

    let config: AppConfig =
        serde_yaml::from_str(&config_content).context("Failed to parse YAML")?;

    // Clear agent.log on startup for fresh debugging session
    let _ = fs::write(
        "data/agent.log",
        format!(
            "--- [{}] Agent session started ---\n\n",
            chrono::Local::now().to_rfc3339()
        ),
    );

    // 2. Initialize global state and manager
    let state = Arc::new(Mutex::new(BotState::load()));
    let bridge_manager = Arc::new(BridgeManager::new(config.clone(), state.clone()));

    CONFIG.set(config.clone()).ok();
    BRIDGE_MANAGER.set(bridge_manager).ok();

    tracing::info!(
        "{}",
        crate::strings::STRINGS
            .logs
            .config_loaded
            .replace("{}", &config.services.matrix.username)
    );

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

    tracing::info!("{}", crate::strings::STRINGS.logs.login_success);

    // 5. Update Display Name if configured
    if let Some(display_name) = &config.services.matrix.display_name {
        tracing::info!(
            "{}",
            crate::strings::STRINGS
                .logs
                .setting_display_name
                .replace("{}", display_name)
        );
        if let Err(e) = client.account().set_display_name(Some(display_name)).await {
            tracing::error!(
                "{}",
                crate::strings::STRINGS
                    .logs
                    .set_display_name_fail
                    .replace("{}", &e.to_string())
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
        tracing::info!("{}", crate::strings::STRINGS.logs.sync_loop_start);
        if let Err(e) = sync_client.sync(SyncSettings::default()).await {
            tracing::error!(
                "{}",
                crate::strings::STRINGS
                    .logs
                    .sync_loop_fail
                    .replace("{}", &e.to_string())
            );
        }
    });

    // 7. Initialize Room states from bridges
    setup_bridges(&client, &config, state.clone()).await;

    // 8. Graceful Shutdown
    match tokio::signal::ctrl_c().await {
        Ok(()) => tracing::info!("{}", crate::strings::STRINGS.logs.shutdown),
        Err(err) => tracing::error!(
            "{}",
            crate::strings::STRINGS
                .logs
                .shutdown_fail
                .replace("{}", &err.to_string())
        ),
    }

    sync_handle.abort();
    Ok(())
}

/// Iterates through configured bridges and joins necessary Matrix rooms.
async fn setup_bridges(client: &Client, config: &AppConfig, state: Arc<Mutex<BotState>>) {
    for (bridge_name, entries) in &config.bridges {
        for entry in entries {
            if entry.service.as_deref() == Some("matrix") {
                if let Some(room_id_str) = &entry.channel {
                    tracing::info!(
                        "{}",
                        crate::strings::STRINGS
                            .logs
                            .bridge_joining
                            .replace("{}", bridge_name)
                            .replace("{}", room_id_str)
                    );

                    if let Ok(room_id) = RoomId::parse(room_id_str) {
                        if let Err(e) = client.join_room_by_id(&room_id).await {
                            tracing::error!(
                                "{}",
                                crate::strings::STRINGS
                                    .logs
                                    .bridge_join_fail
                                    .replace("{}", room_id_str)
                                    .replace("{}", &e.to_string())
                            );
                        } else if let Some(room) = client.get_room(&room_id) {
                            tracing::info!(
                                "{}",
                                crate::strings::STRINGS
                                    .logs
                                    .bridge_join_success
                                    .replace("{}", room_id_str)
                            );

                            // Send status message instead of welcome message
                            let service = MatrixService::new(room);
                            crate::commands::handle_status(&config, state.clone(), &service).await;
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
            crate::strings::STRINGS
                .logs
                .invite_received
                .replace("{}", &format!("{:?}", room.room_id()))
        );
        if let Err(e) = room.join().await {
            tracing::error!(
                "{}",
                crate::strings::STRINGS
                    .logs
                    .join_invite_fail
                    .replace("{}", &e.to_string())
            );
        } else {
            tracing::info!("{}", crate::strings::STRINGS.logs.join_invite_success);
        }
    }
}
