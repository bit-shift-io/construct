pub fn config_loaded(user: &str) -> String {
    format!("Loaded configuration for user: {user}")
}

pub const LOGIN_SUCCESS: &str = "Logged in successfully!";

pub fn setting_display_name(name: &str) -> String {
    format!("Setting display name to: {name}")
}

pub fn set_display_name_fail(err: &str) -> String {
    format!("Failed to set display name: {err}")
}

pub const SYNC_LOOP_START: &str = "Starting sync loop...";

pub fn sync_loop_fail(err: &str) -> String {
    format!("Sync loop failed: {err}")
}

pub const SHUTDOWN: &str = "Shutting down...";

pub fn shutdown_fail(err: &str) -> String {
    format!("Unable to listen for shutdown signal: {err}")
}

pub fn bridge_joining(name: &str, room: &str) -> String {
    format!("Bridge [{name}]: Joining room {room}...")
}

pub fn bridge_join_fail(room: &str, err: &str) -> String {
    format!("   Failed to join room {room}: {err}")
}

pub fn bridge_join_success(room: &str) -> String {
    format!("   Successfully joined room {room}.")
}

pub fn invite_received(room_id: &str) -> String {
    format!("💌 Received invite for room {room_id:?}")
}

pub fn join_invite_fail(err: &str) -> String {
    format!("Failed to join room after invite: {err}")
}

pub const JOIN_INVITE_SUCCESS: &str = "✅ Successfully joined room!";
pub const CONFIG_READ_ERROR: &str = "Failed to read data/config.yaml";
pub const CONFIG_PARSE_ERROR: &str = "Failed to parse YAML";
pub const MCP_START_FAIL_WARN: &str = "Continuing without MCP - admin commands will still work";

pub fn agent_session_start(timestamp: &str) -> String {
    format!("--- [{}] Agent session started ---\n\n", timestamp)
}


pub fn mcp_started(dirs: &[String]) -> String {
    format!("MCP sidecar started successfully with allowed directories: {:?}", dirs)
}

pub const MCP_FAILED: &str = "Failed to start MCP sidecar: {}";

pub fn mcp_failed(error: &str) -> String {
    MCP_FAILED.replace("{}", error)
}
