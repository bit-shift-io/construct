use serde::Deserialize;
use std::collections::HashMap;

/// Main application configuration structure.
/// Matches the layout of `data/config.yaml`.
#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub services: ServicesConfig,
    #[serde(default)]
    pub agents: AgentsConfig,
    #[serde(default)]
    pub bridges: HashMap<String, Vec<BridgeEntry>>,
    #[allow(dead_code)]
    pub commands: CommandsConfig,
    #[serde(default)]
    pub system: SystemConfig,
    pub mcp: McpConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CommandsConfig {
    #[serde(default = "default_command_mode")]
    pub default: String,
    #[serde(default)]
    pub ask: Vec<String>,
    #[serde(default)]
    pub allowed: Vec<String>,
    #[serde(default)]
    pub blocked: Vec<String>,
    #[serde(default)]
    pub timeouts: TimeoutConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TimeoutConfig {
    #[serde(default = "default_short_timeout")]
    pub short: u64,
    #[serde(default = "default_medium_timeout")]
    pub medium: u64,
    #[serde(default = "default_long_timeout")]
    pub long: u64,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            short: default_short_timeout(),
            medium: default_medium_timeout(),
            long: default_long_timeout(),
        }
    }
}

fn default_short_timeout() -> u64 {
    30
}
fn default_medium_timeout() -> u64 {
    120
}
fn default_long_timeout() -> u64 {
    600
}

fn default_command_mode() -> String {
    "ask".to_string()
}

/// System-level settings for the bot.
#[derive(Debug, Default, Deserialize, Clone)]
pub struct SystemConfig {
    #[serde(default)]
    pub projects_dir: Option<String>,
    #[serde(default)]
    pub action_delay: Option<u64>,
    #[serde(default)]
    pub admin: Vec<String>,
}

/// Represents a specific bridge entry connecting a service to a channel.
#[derive(Debug, Deserialize, Clone)]
pub struct BridgeEntry {
    pub service: Option<String>,
    pub channel: Option<String>,
    pub agents: Option<Vec<String>>,
}

/// Configuration for various connected services.
#[derive(Debug, Deserialize, Clone)]
pub struct ServicesConfig {
    pub matrix: MatrixConfig,
}

pub type AgentsConfig = HashMap<String, AgentConfig>;

#[derive(Debug, Deserialize, Clone)]
pub struct AgentConfig {
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub command: Option<String>, // Legacy CLI command
    #[serde(default)]
    pub model: String, // Required for Rig
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>, // e.g. "GEMINI_API_KEY"
    #[serde(default)]
    pub model_order: Option<Vec<String>>, // Regex patterns for ordering discovered models
    #[serde(default)]
    pub model_fallbacks: Option<Vec<String>>, // Explicit fallback models if discovery fails
    #[serde(default)]
    pub fallback_agent: Option<String>, // Agent to switch to if all models fail
    #[serde(default)]
    pub requests_per_minute: Option<u64>,
    /// Additional provider-specific parameters (e.g., caching, debug, temperature)
    #[serde(default)]
    pub extra_params: std::collections::HashMap<String, serde_json::Value>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            provider: String::new(),
            command: None,
            model: String::new(),
            endpoint: None,
            api_key: None,
            api_key_env: None,
            model_order: None,
            model_fallbacks: None,
            fallback_agent: None,
            requests_per_minute: None,
            extra_params: std::collections::HashMap::new(),
        }
    }
}

/// Specific configuration for the Matrix service.
#[derive(Debug, Deserialize, Clone)]
pub struct MatrixConfig {
    #[serde(default)]
    #[allow(dead_code)]
    pub protocol: String,
    pub username: String,
    pub password: String,
    pub homeserver: String,
    #[serde(default)]
    pub display_name: Option<String>,
}

/// Configuration for MCP (Model Context Protocol) server
#[derive(Debug, Deserialize, Clone)]
pub struct McpConfig {
    /// Path to the MCP server binary
    pub server_path: String,
    /// List of directories the MCP server is allowed to access
    pub allowed_directories: Vec<String>,
    /// Enable read-only mode for additional safety
    #[serde(default = "default_false")]
    pub readonly: bool,
    /// Default timeout in seconds for commands
    #[serde(default = "default_mcp_timeout")]
    pub default_timeout: u64,
}

fn default_false() -> bool {
    false
}

fn default_mcp_timeout() -> u64 {
    120
}
