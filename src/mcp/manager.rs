use std::sync::Arc;

use crate::mcp::client::{SharedMcpClient, create_shared_client};

/// Manages MCP client for tool invocation
///
/// The McpManager provides a singleton-like interface to access the MCP client
/// throughout the application. It handles client creation and provides shared
/// access via Arc<Mutex<McpClient>> for thread-safe concurrent usage.
///
/// # Example
/// ```no_run
/// let mcp_manager = McpManager::new(
///     "mcp-filesystem-server",
///     &vec!["./projects".to_string(), "./data".to_string()],
///     false
/// ).await?;
///
/// let client = mcp_manager.client();
/// // Use client to invoke tools
/// ```
pub struct McpManager {
    /// The shared client for communicating with the MCP server
    client: SharedMcpClient,
}

impl McpManager {
    /// Create a new MCP manager by initializing the client
    ///
    /// This creates an MCP client that spawns and connects to the MCP server process.
    /// The client manages its own connection to the server.
    ///
    /// # Arguments
    /// * `server_path` - Path to MCP server binary
    /// * `allowed_dirs` - List of directories MCP server is allowed to access
    /// * `readonly` - Whether to enable read-only mode for additional safety
    ///
    /// # Returns
    /// Result containing the McpManager instance or an error if initialization fails
    ///
    /// # Errors
    /// Returns an error if:
    /// - The server binary cannot be found or executed
    /// - The client cannot connect to the server
    /// - The server fails to start
    pub async fn new(
        server_path: &str,
        allowed_dirs: &[String],
        readonly: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let client = create_shared_client(server_path, allowed_dirs, readonly).await?;

        Ok(Self { client })
    }

    /// Get the MCP client for tool invocation
    ///
    /// Returns a clone of the Arc wrapping the client, allowing multiple
    /// parts of the application to share the same client instance safely.
    ///
    /// # Returns
    /// Arc wrapping the MCP client
    pub fn client(&self) -> SharedMcpClient {
        Arc::clone(&self.client)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires actual MCP server binary
    async fn test_mcp_manager_creation() {
        let manager = McpManager::new("mcp-filesystem-server", &vec!["./test".to_string()], true)
            .await
            .expect("Failed to create MCP manager");

        // Test client access
        let client = manager.client();
        assert!(client.try_lock().is_ok());
    }
}
