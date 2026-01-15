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
        let project_path = Path::new(parent_dir).join(name);
        let path_str = project_path.to_string_lossy().to_string();

        // Create project directory
        std::fs::create_dir_all(&project_path)
            .map_err(|e| anyhow::anyhow!("Failed to create project dir: {}", e))?;

        // Create tasks directory
        std::fs::create_dir_all(project_path.join("tasks"))
            .map_err(|e| anyhow::anyhow!("Failed to create tasks dir: {}", e))?;

        // Create specs directory inside tasks
        std::fs::create_dir_all(project_path.join(crate::domain::paths::SPECS_DIR))
            .map_err(|e| anyhow::anyhow!("Failed to create tasks/specs dir: {}", e))?;


        // We leave the file creation to the Agent in the New Project phase.

        Ok(path_str)
    }

    /// Validate if a given path is a valid project (currently checks for roadmap.md).
    pub async fn is_valid_project(&self, path: &str) -> bool {
        let client = self.tools.lock().await;
        // Check for roadmap.md existence using read_file as a proxy.
        client
            .read_file(&crate::domain::paths::roadmap_path(path))
            .await
            .is_ok()
    }
}
