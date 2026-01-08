#![allow(dead_code)]

//! # Tool Executor
//!
//! Handles safe execution of shell commands and filesystem operations.
//! Enforces sandboxing by validating paths against valid root directories.

use anyhow::{Context as AnyhowContext, Result};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

/// Configuration for the ToolExecutor
#[derive(Debug, Clone)]
pub struct ToolConfig {
    /// List of allowed root directories.
    /// All accessed paths must be within one of these roots.
    pub allowed_directories: Vec<String>,
    pub timeout_default: u64,
    pub timeout_long: u64,
    pub long_commands: Vec<String>,
}

/// Executes tools (shell, fs) securely.
#[derive(Debug)]
pub struct ToolExecutor {
    config: ToolConfig,
}

impl ToolExecutor {
    pub fn new(
        allowed_directories: Vec<String>,
        timeout_default: u64,
        timeout_long: u64,
        long_commands: Vec<String>,
    ) -> Self {
        Self {
            config: ToolConfig {
                allowed_directories,
                timeout_default,
                timeout_long,
                long_commands,
            },
        }
    }

    /// Validates that a path is safe to access (contained within allowed roots).
    // ... validate_path (unchanged) ...
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
                    return Err(anyhow::anyhow!(
                        "Unable to validate path: Root does not exist or invalid path structure: {:?}",
                        path
                    ));
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
            Err(anyhow::anyhow!(
                "Access denied: Path '{:?}' is not in allowed directories {:?}",
                abs_path,
                self.config.allowed_directories
            ))
        }
    }

    /// Execute a shell command in a specific working directory.
    pub async fn execute_command(&self, command: &str, cwd: &Path) -> Result<String> {
        // 1. Validate CWD is allowed (double check security)
        let safe_cwd = self
            .validate_path(cwd)
            .context("Invalid CWD for command execution")?;

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
        cmd.kill_on_drop(true); // Ensure process is killed if timed out (dropped)

        // Ensure the process is grouped so we can kill its children if necessary (PGID on unix)
        // Rust std/tokio Command::spawn doesn't easily set pgid without unsafe or ext.
        // For now, we assume direct killing of the shell process is sufficient to stop most simple tasks.

        // 3. Determine Timeout
        let binary = command.split_whitespace().next().unwrap_or("");
        let timeout_sec = if self.config.long_commands.iter().any(|c| c == binary) {
            self.config.timeout_long
        } else {
            self.config.timeout_default
        };

        // 4. Configure & Spawn
        let child = cmd.spawn().context("Failed to spawn command shell")?;

        // 5. Wait for output (or timeout)
        let output_result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_sec),
            child.wait_with_output(),
        )
        .await;

        let output = match output_result {
            Ok(io_result) => io_result?,
            Err(_) => {
                // Timeout occurred!
                // Because of kill_on_drop(true), dropping the future (which owns child) kills the process.
                return Err(anyhow::anyhow!(
                    "Command '{}' timed out after {}s",
                    binary,
                    timeout_sec
                ));
            }
        };

        // 5. Format Output
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut result = String::new();
        if !stdout.is_empty() {
            result.push_str(&stdout);
        }
        if !stderr.is_empty() {
            if !result.is_empty() {
                result.push_str("\n--- STDERR ---\n");
            }
            result.push_str(&stderr);
        }

        if !output.status.success() {
            if !result.is_empty() {
                result.push_str("\n");
            }
            result.push_str(&format!("[Exit Code: {}]", output.status));
        }

        Ok(result)
    }

    pub async fn read_file(&self, path: &str) -> Result<String> {
        let path = Path::new(path);
        let safe_path = self.validate_path(path)?;

        info!("Reading file: {:?}", safe_path);

        tokio::fs::read_to_string(&safe_path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read file '{:?}': {}", safe_path, e))
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

        tokio::fs::write(safe_path, content)
            .await
            .context("Failed to write file")
    }

    pub async fn list_dir(&self, path: &str) -> Result<String> {
        let path = Path::new(path);
        let safe_path = self.validate_path(path)?;

        let mut entries = tokio::fs::read_dir(safe_path)
            .await
            .context("Failed to read dir")?;
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

    /// Find files matching a glob pattern within a directory.
    pub async fn find_files(&self, path: &str, pattern: &str) -> Result<String> {
        let path = Path::new(path);
        let safe_path = self.validate_path(path)?;

        // Ensure we are searching within a directory
        if !safe_path.is_dir() {
            return Err(anyhow::anyhow!("Find target is not a directory"));
        }

        let mut listing = String::new();
        let pattern_obj = glob::Pattern::new(pattern).context("Invalid glob pattern")?;

        // Use WalkDir for recursive traversal
        // Note: functionality is synchronous, so we might block the thread.
        // For large directories, this should ideally be in spawn_blocking.
        let walker = walkdir::WalkDir::new(&safe_path)
            .follow_links(false) // Security: don't follow symlinks to outside
            .max_depth(10); // Sanity limit

        let entries: Vec<_> = walker.into_iter().filter_map(|e| e.ok()).collect();

        for entry in entries {
            let entry_path = entry.path();
            
            // Generate relative path for matching against pattern
            // The pattern should match against the filename or relative path?
            // "find . -name *.rs" matches file name.
            // "glob" matches path.
            // Let's assume pattern is matching the FILENAME by default, 
            // OR if pattern contains separator, match relative path.
            
            // Actually, typical find usage: find <path> <pattern>
            // We want to return paths where the filename matches the pattern.
            // If pattern is "**/*.rs", glob matches against whole path.
            
            // Simple implementation: Check if filename matches pattern.
            // If pattern contains /, check if relative path matches.
            
            let file_name = entry.file_name().to_string_lossy();
            if pattern_obj.matches(&file_name) {
                 // Format: <RelativePath> [TYPE]
                 let relative = entry_path.strip_prefix(&safe_path).unwrap_or(entry_path);
                 let type_str = if entry.file_type().is_dir() { "DIR" } else { "FILE" };
                 listing.push_str(&format!("{} [{}]\n", relative.display(), type_str));
            } else if pattern.contains('/') {
                // Try matching full relative path (for **/* support)
                let relative = entry_path.strip_prefix(&safe_path).unwrap_or(entry_path);
                if pattern_obj.matches(&relative.to_string_lossy()) {
                     let type_str = if entry.file_type().is_dir() { "DIR" } else { "FILE" };
                     listing.push_str(&format!("{} [{}]\n", relative.display(), type_str));
                }
            }
        }
        
        if listing.is_empty() {
            Ok("No matching files found.".to_string())
        } else {
            Ok(listing)
        }
    }
}

pub type SharedToolExecutor = Arc<Mutex<ToolExecutor>>;
