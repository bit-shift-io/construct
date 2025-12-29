pub mod client;
pub mod manager;
pub mod tools;

// Re-export public types and functions for external use
pub use client::{McpClient, SharedMcpClient, create_shared_client};
pub use manager::McpManager;
