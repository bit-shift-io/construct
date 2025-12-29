use anyhow::Result;
use std::sync::Arc;
use tokio::process::Command as TokioCommand;
use tokio::sync::Mutex;

/// MCP client for invoking tools on MCP server
///
/// This client currently uses direct shell execution as a placeholder.
/// It can be upgraded to use the actual MCP protocol when the rmcp API is stabilized.
pub struct McpClient {
    /// Placeholder for future MCP client connection
    #[allow(dead_code)]
    server_path: String,
}

impl McpClient {
    /// Create a new MCP client
    ///
    /// # Arguments
    /// * `server_path` - Path to MCP server binary (reserved for future use)
    /// * `allowed_dirs` - List of directories MCP server is allowed to access
    /// * `readonly` - Whether to enable read-only mode
    ///
    /// # Returns
    /// Result containing the McpClient instance or an error if initialization fails
    pub async fn new(server_path: &str, _allowed_dirs: &[String], _readonly: bool) -> Result<Self> {
        Ok(Self {
            server_path: server_path.to_string(),
        })
    }

    /// Execute a shell command with timeout support
    ///
    /// # Arguments
    /// * `command` - The shell command to execute
    /// * `timeout` - Optional timeout in seconds (defaults to 120 if None)
    /// * `working_dir` - Optional working directory for the command
    ///
    /// # Returns
    /// Result containing the command output (stdout/stderr) or an error
    pub async fn execute_command(
        &mut self,
        command: &str,
        timeout: Option<u64>,
        working_dir: Option<&str>,
    ) -> Result<String> {
        let timeout_secs = timeout.unwrap_or(120);

        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = TokioCommand::new("cmd");
            c.args(["/C", command]);
            c
        } else {
            let mut c = TokioCommand::new("sh");
            c.args(["-c", command]);
            c
        };

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        let output =
            tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), cmd.output())
                .await??;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            if !stderr.is_empty() {
                Ok(format!("{}\n[stderr]\n{}", stdout, stderr))
            } else {
                Ok(stdout)
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if stderr.is_empty() {
                Ok(format!("[Exit Code: {}]", output.status))
            } else {
                Ok(format!("[Exit Code: {}]\n{}", output.status, stderr))
            }
        }
    }

    /// Read the contents of a file
    ///
    /// # Arguments
    /// * `path` - Path to the file to read
    ///
    /// # Returns
    /// Result containing the file contents or an error
    pub async fn read_file(&mut self, path: &str) -> Result<String> {
        tokio::fs::read_to_string(path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read file: {}", e))
    }

    /// Write content to a file
    ///
    /// # Arguments
    /// * `path` - Path to the file to write
    /// * `content` - Content to write to the file
    ///
    /// # Returns
    /// Result indicating success or failure
    pub async fn write_file(&mut self, path: &str, content: &str) -> Result<()> {
        tokio::fs::write(path, content)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to write file: {}", e))
    }

    /// List the contents of a directory
    ///
    /// # Arguments
    /// * `path` - Optional directory path to list (defaults to current directory)
    ///
    /// # Returns
    /// Result containing the directory listing or an error
    pub async fn list_directory(&mut self, path: Option<&str>) -> Result<String> {
        let dir = path.unwrap_or(".");
        let mut entries = tokio::fs::read_dir(dir)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list directory: {}", e))?;

        let mut listing = String::new();
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            let file_type = if entry.file_type().await?.is_dir() {
                "DIR"
            } else {
                "FILE"
            };
            listing.push_str(&format!("{} [{}]\n", name, file_type));
        }

        Ok(listing)
    }

    /// Create a directory
    ///
    /// # Arguments
    /// * `path` - Path to the directory to create
    /// * `recursive` - Whether to create parent directories if they don't exist
    ///
    /// # Returns
    /// Result indicating success or failure
    pub async fn create_directory(&mut self, path: &str, recursive: bool) -> Result<()> {
        if recursive {
            tokio::fs::create_dir_all(path)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create directory: {}", e))
        } else {
            tokio::fs::create_dir(path)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create directory: {}", e))
        }
    }
}

/// Type alias for shared MCP client
pub type SharedMcpClient = Arc<Mutex<McpClient>>;

/// Create a shared MCP client
///
/// # Arguments
/// * `server_path` - Path to MCP server binary
/// * `allowed_dirs` - List of directories MCP server is allowed to access
/// * `readonly` - Whether to enable read-only mode
///
/// # Returns
/// Result containing the shared MCP client or an error
pub async fn create_shared_client(
    server_path: &str,
    allowed_dirs: &[String],
    readonly: bool,
) -> Result<SharedMcpClient, Box<dyn std::error::Error>> {
    let client = McpClient::new(server_path, allowed_dirs, readonly).await?;
    Ok(Arc::new(Mutex::new(client)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let mut client = McpClient::new("test-server", &["./test".to_string()], true)
            .await
            .expect("Failed to create MCP client");

        // Test a simple command
        let result = client
            .execute_command("echo 'Hello, MCP!'", Some(5), None)
            .await
            .expect("Failed to execute command");

        assert!(result.contains("Hello, MCP!"));
    }

    #[tokio::test]
    async fn test_read_write_file() -> anyhow::Result<()> {
        let mut client = McpClient::new("test-server", &["./test".to_string()], false).await?;

        // Write a test file
        client
            .write_file("/tmp/test_mcp.txt", "Test content")
            .await?;

        // Read it back
        let content = client.read_file("/tmp/test_mcp.txt").await?;
        assert_eq!(content, "Test content");

        // Cleanup
        tokio::fs::remove_file("/tmp/test_mcp.txt").await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_list_directory() -> anyhow::Result<()> {
        let mut client = McpClient::new("test-server", &["./test".to_string()], true).await?;

        let listing = client.list_directory(Some("/tmp")).await?;
        assert!(listing.contains("DIR") || listing.contains("FILE"));

        Ok(())
    }
}
