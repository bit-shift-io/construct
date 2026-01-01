#![recursion_limit = "256"]
//! # Main Entry Point (V2)
//!
//! Initializes the application using the V2 architecture:
//! - Domain: Configuration and Types
//! - Infrastructure: Matrix, MCP, LLM
//! - Application: Router, Engine, Logging, Feed
//! - Interface: Command Handlers
//!

mod domain;
mod infrastructure;
mod application;
mod interface;
mod strings; 

use anyhow::{Context, Result};
use matrix_sdk::{
    Client,
    config::SyncSettings,
    room::Room,
    ruma::events::room::{
        member::{MembershipState, StrippedRoomMemberEvent},
        message::SyncRoomMessageEvent,
    },
};
use std::fs;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;
use tracing;

use crate::domain::config::AppConfig;
use crate::application::router::CommandRouter;
use crate::application::project::ProjectManager;
use crate::infrastructure::mcp::client::McpClient;
use crate::infrastructure::matrix::MatrixService;
use crate::infrastructure::llm::Client as LlmClient;

static CONFIG: OnceLock<AppConfig> = OnceLock::new();
static ROUTER: OnceLock<Arc<CommandRouter>> = OnceLock::new();

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Load Configuration
    let config_content = fs::read_to_string("data/config.yaml").context("Failed to read config.yaml")?;
    let config: AppConfig = serde_yaml::from_str(&config_content).context("Failed to parse config.yaml")?;
    CONFIG.set(config.clone()).ok();

    CONFIG.set(config.clone()).ok();
    
    // 2. Logging Setup
    // Ensure data directory exists
    if !std::path::Path::new("data").exists() {
        fs::create_dir("data").context("Failed to create data directory")?;
    }

    let file_appender = tracing_appender::rolling::never("data", "session.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new("info")
    });
    
    // Layer for file
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false);

    // Layer for console
    let console_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout);

    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    tracing_subscriber::registry()
        .with(env_filter)
        .with(console_layer)
        .with(file_layer)
        .init();

    tracing::info!("Starting Construct V2...");

    // 3. Initialize Infrastructure
    // MCP
    let mcp = Arc::new(Mutex::new(McpClient::new(
        &config.mcp.server_path,
        &config.mcp.allowed_directories,
        config.mcp.readonly,
    ).await?));
    
    // LLM
    let llm = Arc::new(LlmClient::new(config.clone()));

    // 4. Initialize Application Components
    let project_manager = Arc::new(ProjectManager::new(mcp.clone()));
    let state = Arc::new(Mutex::new(crate::application::state::BotState::load()));

    // 5. Matrix Setup
    let client = Client::builder()
        .homeserver_url(&config.services.matrix.homeserver)
        .build()
        .await?;

    client.matrix_auth()
        .login_username(&config.services.matrix.username, &config.services.matrix.password)
        .send()
        .await?;
        
    tracing::info!("Logged in as {}", config.services.matrix.username);

    // 6. Event Loop
    let start_time = std::time::SystemTime::now();
    let startup_client = client.clone();
    let startup_project_manager = project_manager.clone();
    let startup_state = state.clone();
    let startup_config = config.clone();

    // Startup Announcement Task
    tokio::spawn(async move {
        // Wait for initial sync to likely populate state
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        
        for room in startup_client.joined_rooms() {
            let chat = MatrixService::new(room.clone());
            // We can reuse the handle_status logic
            // Note: Router logic for .status uses handle_status.
            // We can just call it directly.
            if let Err(e) = crate::interface::commands::misc::handle_status(&startup_state, &chat).await {
                tracing::error!("Failed to send startup status to room {}: {}", room.room_id(), e);
            }
        }
    });

    client.add_event_handler(move |ev: SyncRoomMessageEvent, room: Room| {
        let config = config.clone();
        let mcp = mcp.clone();
        let llm = llm.clone();
        let state = state.clone();
        let project_manager = project_manager.clone();
        
        async move {
            let make_chat = |room: Room| MatrixService::new(room);
            let make_router = |config, mcp, llm, pm, state| CommandRouter::new(config, mcp, llm, pm, state);

            if let Some(original_msg) = ev.as_original() {
                // Ignore events older than start_time
                let ts = ev.origin_server_ts();
                // Ruma MilliSecondsSinceUnixEpoch
                let event_time = std::time::UNIX_EPOCH + std::time::Duration::from_millis(ts.get().into());
                if event_time < start_time {
                    return;
                }

                if let matrix_sdk::ruma::events::room::message::MessageType::Text(text_content) = &original_msg.content.msgtype {
                     let body = &text_content.body;
                     if original_msg.sender == room.own_user_id() { return; }
                     
                     let chat = make_chat(room);
                     let router = make_router(config, mcp, llm, project_manager, state);
                     
                     // Dispatch
                     let _ = router.route(&chat, &body).await;
                }
            }
        }
    });

    // Handle Invites
    client.add_event_handler(|ev: StrippedRoomMemberEvent, room: Room| async move {
         if ev.content.membership == MembershipState::Invite {
             let _ = room.join().await;
         }
    });
    
    client.sync(SyncSettings::default()).await?;

    Ok(())
}
