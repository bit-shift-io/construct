//! # Project Manager
//!
//! Manages project-specific context and operations, such as identifying the active project root
//! and handling project creation or listing via MCP.

use crate::infrastructure::tools::executor::SharedToolExecutor;
use anyhow::Result;
use std::path::Path;

/// Manages project-specific logic using the internal Tool System.
#[derive(Debug)]
pub struct ProjectManager {
    tools: SharedToolExecutor,
}

impl ProjectManager {
    pub fn new(tools: SharedToolExecutor) -> Self {
        Self { tools }
    }

    /// Create a new project directory and initial files
    pub async fn create_project(&self, name: &str, parent_dir: &str) -> Result<String> {
        let client = self.tools.lock().await;       
        let project_path = Path::new(parent_dir).join(name);
        let path_str = project_path.to_string_lossy().to_string();

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

    /// Validate if a given path is a valid project (currently checks for roadmap.md).
    pub async fn is_valid_project(&self, path: &str) -> bool {
        let client = self.tools.lock().await;
        // Check for roadmap.md existence using read_file as a proxy.
        client.read_file(&format!("{}/roadmap.md", path)).await.is_ok()
    }
}
