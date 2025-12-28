use std::fs;
use std::path::Path;

use serde::{Serialize, Deserialize};

/// Manages project-specific state stored in {project}/state.md
/// This includes execution history, task context, and other project-specific metadata.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ProjectStateManager {
    pub project_path: String,
}

impl ProjectStateManager {
    /// Creates a new project state manager for the given project path.
    pub fn new(project_path: String) -> Self {
        Self { project_path }
    }

    /// Gets the path to the state.md file for this project.
    fn state_file_path(&self) -> String {
        format!("{}/state.md", self.project_path)
    }

    /// Appends a new entry to the project's state.md file.
    /// Each entry includes a timestamp and the content.
    pub fn append_entry(&self, content: &str) -> Result<(), String> {
        self.append_entry_internal(content, false)
    }

    /// Internal implementation for appending entries.
    fn append_entry_internal(&self, content: &str, is_temporary: bool) -> Result<(), String> {
        let state_path = self.state_file_path();

        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let entry = if is_temporary {
            format!(
                "\n## [{}] [TEMPORARY]\n{}\n",
                timestamp,
                content.trim().replace('\n', "\n  ")
            )
        } else {
            format!(
                "\n## [{}]\n{}\n",
                timestamp,
                content.trim().replace('\n', "\n  ")
            )
        };

        let mut existing_content = if Path::new(&state_path).exists() {
            fs::read_to_string(&state_path).unwrap_or_default()
        } else {
            String::from(
                "# Project State\n\nThis file tracks the execution history and context for this project.\n",
            )
        };

        existing_content.push_str(&entry);

        fs::write(&state_path, existing_content)
            .map_err(|e| format!("Failed to write state.md: {}", e))
    }

    /// Updates the state with command execution.
    pub fn log_command(&self, command: &str, output: &str, success: bool) -> Result<(), String> {
        let status = if success { "✅" } else { "❌" };
        let entry = format!(
            "{} **Command**: `{}`\n```\n{}\n```",
            status,
            command,
            output.chars().take(1000).collect::<String>() // Truncate long output
        );
        self.append_entry(&entry)
    }

    /// Updates the state with a system note.
    pub fn log_note(&self, note: &str) -> Result<(), String> {
        let entry = format!("**Note**: {}", note);
        self.append_entry(&entry)
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_project_state_manager() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path().to_str().unwrap().to_string();

        let manager = ProjectStateManager::new(project_path.clone());

        // Test initialization
        manager.initialize().unwrap();
        assert!(manager.exists());

        // Test appending entries
        manager.log_note("Test note").unwrap();
        manager
            .log_command("ls", "file1.txt\nfile2.txt", true)
            .unwrap();

        let content = manager.read().unwrap();
        assert!(content.contains("Test note"));
        assert!(content.contains("ls"));
        assert!(content.contains("file1.txt"));

        // Test clearing
        manager.clear().unwrap();
        let cleared_content = manager.read().unwrap();
        assert!(!cleared_content.contains("Test note"));
    }
}
