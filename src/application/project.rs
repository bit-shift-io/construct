//! # Project Manager
//!
//! Manages project-specific context and operations, such as identifying the active project root
//! and handling project creation or listing via MCP.

use crate::infrastructure::mcp::client::SharedMcpClient;
use anyhow::Result;
use std::path::Path;

pub struct ProjectManager {
    mcp: SharedMcpClient,
}

impl ProjectManager {
    pub fn new(mcp: SharedMcpClient) -> Self {
        Self { mcp }
    }

    /// Create a new project directory and initial files
    pub async fn create_project(&self, name: &str, parent_dir: &str) -> Result<String> {
        let mut client = self.mcp.lock().await;
        let project_path = Path::new(parent_dir).join(name);
        let path_str = project_path.to_string_lossy().to_string();

        // 1. Create Directory
        client.create_directory(&path_str, true).await?;

        // 2. Create Initial Files
        // roadmap.md
        client.write_file(
            &format!("{}/roadmap.md", path_str),
            "# Roadmap\n\n- [ ] Initial Setup\n"
        ).await?;

        // tasks.md
        client.write_file(
            &format!("{}/tasks.md", path_str),
            "# Tasks\n\n- [ ] Initialize project structure\n"
        ).await?;

        // state.md (Empty initially)
        client.write_file(
            &format!("{}/state.md", path_str),
            "# Project State\n"
        ).await?;

        Ok(path_str)
    }

    /// Validate if a path is a valid project
    pub async fn is_valid_project(&self, path: &str) -> bool {
        let mut client = self.mcp.lock().await;
        // Check for roadmap.md existence using list_directory as a proxy?
        // Or try to read it.
        // McpClient read_file returns Result.
        client.read_file(&format!("{}/roadmap.md", path)).await.is_ok()
    }
}
