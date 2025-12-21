#![recursion_limit = "256"]

mod config;
mod state;
mod agent;
mod bridge;
mod commands;

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

use crate::config::AppConfig;
use crate::state::BotState;
use crate::bridge::BridgeManager;

/// Static configuration and state managers.
/// Using OnceLock for safe global access.
static CONFIG: OnceLock<AppConfig> = OnceLock::new();
static BRIDGE_MANAGER: OnceLock<Arc<BridgeManager>> = OnceLock::new();

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Initial configuration loading
    let config_content =
        fs::read_to_string("data/config.yaml").context("Failed to read data/config.yaml")?;

    let config: AppConfig =
        serde_yaml::from_str(&config_content).context("Failed to parse YAML")?;

    // 2. Initialize global state and manager
    let state = Arc::new(Mutex::new(BotState::load()));
    let bridge_manager = Arc::new(BridgeManager::new(config.clone(), state.clone()));
    
    CONFIG.set(config.clone()).ok();
    BRIDGE_MANAGER.set(bridge_manager).ok();

    println!("Loaded configuration for user: {}", config.services.matrix.username);

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

    println!("Logged in successfully!");

    // 5. Update Display Name if configured
    if let Some(display_name) = &config.services.matrix.display_name {
        println!("Setting display name to: {}", display_name);
        if let Err(e) = client.account().set_display_name(Some(display_name)).await {
            eprintln!("Failed to set display name: {}", e);
        }
    }

    // 6. Register Event Handlers
    // Invitations: handled locally in main or moved to bridge if needed
    client.add_event_handler(|ev: StrippedRoomMemberEvent, room: Room| async move { handle_invites(ev, room).await });
    
    // Messages: delegated to BridgeManager
    client.add_event_handler(|ev: SyncRoomMessageEvent, room: Room| async move {
        if let Some(manager) = BRIDGE_MANAGER.get() {
            manager.handle_message(ev, room).await;
        }
    });

    // 6. Start Sync Loop
    let sync_client = client.clone();
    let sync_handle = tokio::spawn(async move {
        println!("Starting sync loop...");
        if let Err(e) = sync_client.sync(SyncSettings::default()).await {
            eprintln!("Sync loop failed: {}", e);
        }
    });

    // 7. Initialize Room states from bridges
    setup_bridges(&client, &config, state.clone()).await;

    // 8. Graceful Shutdown
    match tokio::signal::ctrl_c().await {
        Ok(()) => println!("Shutting down..."),
        Err(err) => eprintln!("Unable to listen for shutdown signal: {}", err),
    }

    sync_handle.abort();
    Ok(())
}

/// Iterates through configured bridges and joins necessary Matrix rooms.
async fn setup_bridges(client: &Client, config: &AppConfig, state: Arc<Mutex<BotState>>) {
    for (bridge_name, entries) in &config.bridges {
        for entry in entries {
            if entry.service == "matrix" {
                if let Some(room_id_str) = &entry.channel {
                    println!("Bridge [{}]: Joining room {}...", bridge_name, room_id_str);
                    
                    if let Ok(room_id) = RoomId::parse(room_id_str) {
                        if let Err(e) = client.join_room_by_id(&room_id).await {
                            eprintln!("   Failed to join room {}: {}", room_id_str, e);
                        } else if let Some(room) = client.get_room(&room_id) {
                            println!("   Successfully joined room {}.", room_id_str);
                            
                            let mut bot_state = state.lock().await;
                            let room_state = bot_state.get_room_state(room_id.as_str());
                            
                            let current_project_full = room_state.current_project_path.clone().unwrap_or_else(|| "None".to_string());
                            let project_name = if current_project_full == "None" {
                                "None"
                            } else {
                                current_project_full.split('/').last().unwrap_or(&current_project_full)
                            };

                            let active_agent_opt = room_state.active_agent.as_ref();
                            let resolved_agent = crate::commands::resolve_agent_name(active_agent_opt, config);
                            
                            let active_model = match room_state.active_model.as_deref() {
                                Some(m) if m == "default" => "auto",
                                Some(m) => m,
                                None => "auto",
                            };
                            
                            let bot_name = config.services.matrix.display_name.as_deref()
                                .or(client.user_id().map(|u| u.localpart()))
                                .unwrap_or("Bot");

                            let help_text = format!("{} online, use .help to list commands.\nProject: {}\nAgent: {} | {}", 
                                bot_name, project_name, resolved_agent, active_model);
                            
                            let _ = room.send(matrix_sdk::ruma::events::room::message::RoomMessageEventContent::text_plain(help_text)).await;
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
        println!("💌 Received invite for room {:?}", room.room_id());
        if let Err(e) = room.join().await {
            eprintln!("Failed to join room after invite: {}", e);
        } else {
            println!("✅ Successfully joined room!");
        }
    }
}
