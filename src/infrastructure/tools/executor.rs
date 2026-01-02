#![allow(dead_code)]
#![allow(dead_code)]
//! # Tool Executor
//!
//! Handles safe execution of shell commands and filesystem operations.
//! Enforces sandboxing by validating paths against valid root directories.

use anyhow::{Result, Context as AnyhowContext};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::sync::Mutex;
use std::sync::Arc;

/// Configuration for the ToolExecutor
#[derive(Debug, Clone)]
pub struct ToolConfig {
    /// List of allowed root directories.
    /// All accessed paths must be within one of these roots.
    pub allowed_directories: Vec<String>,
}

/// Executes tools (shell, fs) securely.
#[derive(Debug)]
pub struct ToolExecutor {
    config: ToolConfig,
}

impl ToolExecutor {
    pub fn new(allowed_directories: Vec<String>) -> Self {
        Self {
            config: ToolConfig { allowed_directories },
        }
    }

    /// Validates that a path is safe to access (contained within allowed roots).
    /// Returns the canonical absolute path if safe.
    pub fn validate_path(&self, path: &Path) -> Result<PathBuf> {
        // 1. Resolve to absolute path (if it exists)
        let abs_path = if path.exists() {
            path.canonicalize()?
        } else {
            // For non-existent files:
            // 1. Try parent
            // 2. If parent doesn't exist, try parent's parent, etc.
            // 3. Keep track of the "relative part" we need to append back
            
            let mut current = path.to_path_buf();
            let mut relative_parts = std::vec::Vec::new();
            
            loop {
                 if current.exists() {
                     let canonical = current.canonicalize()?;
                     // Re-append parts
                     let mut final_path = canonical;
                     for part in relative_parts.iter().rev() {
                         final_path.push(part);
                     }
                     break final_path;
                 }
                 
                 if let Some(parent) = current.parent() {
                     if let Some(name) = current.file_name() {
                        relative_parts.push(name.to_owned());
                     }
                     current = parent.to_path_buf();
                 } else {
                     // Hit root and it doesn't exist? (Shouldn't happen on linux for /)
                     return Err(anyhow::anyhow!("Unable to validate path: Root does not exist or invalid path structure: {:?}", path));
                 }
            }
        };

        // 2. Check against allowed roots
        let safe = self.config.allowed_directories.iter().any(|root| {
            // We assume config roots are already absolute/canonical or we canonicalize them here.
            //Ideally, convert root to Path and check strictly.
            let root_path = Path::new(root);
             // Simple prefix check for MVP. 
             // In production code, we'd canonicalize roots in `new()`.
             abs_path.starts_with(root_path)
        });

        if safe {
            Ok(abs_path)
        } else {
            Err(anyhow::anyhow!("Access denied: Path '{:?}' is not in allowed directories {:?}", abs_path, self.config.allowed_directories))
        }
    }

    /// Execute a shell command in a specific working directory.
    pub async fn execute_command(&self, command: &str, cwd: &Path) -> Result<String> {
        // 1. Validate CWD is allowed (double check security)
        let safe_cwd = self.validate_path(cwd).context("Invalid CWD for command execution")?;

        // 2. Construct Command
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = tokio::process::Command::new("cmd");
            c.args(["/C", command]);
            c
        } else {
            let mut c = tokio::process::Command::new("sh");
            c.args(["-c", command]);
            c
        };

        cmd.current_dir(safe_cwd);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // 3. Execute with Timeout
        let child = cmd.spawn().context("Failed to spawn command shell")?;
        
        // Wait for output (or timeout)
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(120),
            child.wait_with_output()
        ).await.context("Command timed out")??;

        // 4. Format Output
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        let mut result = String::new();
        if !stdout.is_empty() {
            result.push_str(&stdout);
        }
        if !stderr.is_empty() {
            if !result.is_empty() { result.push_str("\n--- STDERR ---\n"); }
            result.push_str(&stderr);
        }
        
        if !output.status.success() {
             if !result.is_empty() { result.push_str("\n"); }
             result.push_str(&format!("[Exit Code: {}]", output.status));
        }

        Ok(result)
    }

    pub async fn read_file(&self, path: &str) -> Result<String> {
        let path = Path::new(path);
        let safe_path = self.validate_path(path)?;
        
        tokio::fs::read_to_string(safe_path).await.context("Failed to read file")
    }
    
    pub async fn write_file(&self, path: &str, content: &str) -> Result<()> {
         let path = Path::new(path);
         // Note: For write, validation logic in `validate_path` handles parent existence check
         let safe_path = self.validate_path(path)?;
         
         // Helper: ensure parent dir exists if safe_path was resolved via parent
         if let Some(parent) = safe_path.parent() {
             if !parent.exists() {
                 tokio::fs::create_dir_all(parent).await?;
             }
         }

         tokio::fs::write(safe_path, content).await.context("Failed to write file")
    }
    
    pub async fn list_dir(&self, path: &str) -> Result<String> {
        let path = Path::new(path);
        let safe_path = self.validate_path(path)?;
        
        let mut entries = tokio::fs::read_dir(safe_path).await.context("Failed to read dir")?;
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
}

pub type SharedToolExecutor = Arc<Mutex<ToolExecutor>>;
