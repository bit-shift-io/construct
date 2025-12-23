use std::path::PathBuf;
use crate::config::CommandsConfig;

#[derive(Debug)]
pub enum PermissionResult {
    Allowed,
    Blocked(String),
    Ask(String),
}

pub struct Sandbox {
    root_dir: PathBuf,
}

impl Sandbox {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let mut root_dir = root.into();
        // Resolve to absolute path to ensure safety check works correctly
        if let Ok(canon) = std::fs::canonicalize(&root_dir) {
            root_dir = canon;
        }
        Self { root_dir }
    }

    /// Resolves a virtual path (where "/" is root_dir) to a real system path.
    /// Returns None if the path escapes the root directory.
    pub fn resolve_path(&self, current_virtual_cwd: &str, target_path: &str) -> Option<PathBuf> {
        // Handle "cd /" or absolute paths as being relative to root
        let base = if target_path.starts_with('/') {
            self.root_dir.clone()
        } else {
            // Join root + current_virtual + target
            // We need to be careful. current_virtual_cwd should be effectively trusted relative path.
            // But for safety, let's treat virtual cwd as just relative.
            let rel_cwd = current_virtual_cwd.trim_start_matches('/');
            self.root_dir.join(rel_cwd)
        };

        let target = base.join(target_path);
        
        // Canonicalize to resolve .. and symlinks
        match std::fs::canonicalize(&target) {
            Ok(canon) => {
                if canon.starts_with(&self.root_dir) {
                    Some(canon)
                } else {
                    None // Escaped jail
                }
            }
            Err(_) => {
                // Path might not exist yet, or we simply block non-existent paths for 'cd' context
                // For 'cd', we usually want it to exist.
                // Simple security: deny if we can't verify it stays inside.
                None
            }
        }
    }

    /// Checks if a command binary is allowed/blocked.
    pub fn check_command(&self, command_line: &str, config: &CommandsConfig) -> PermissionResult {
        let parts: Vec<&str> = command_line.split_whitespace().collect();
        if parts.is_empty() {
            return PermissionResult::Allowed; // Empty command does nothing
        }
        
        let binary = parts[0];

        // 1. Check Blocked
        if config.blocked.iter().any(|b| b == binary) {
            return PermissionResult::Blocked(format!("Command '{}' is explicitly blocked.", binary));
        }

        // 2. Check Allowed
        if config.allowed.iter().any(|a| a == binary) {
            return PermissionResult::Allowed;
        }

        // 3. Check Ask
        if config.ask.iter().any(|a| a == binary) {
            return PermissionResult::Ask(format!("Command '{}' requires confirmation.", binary));
        }

        // 4. Default Mode
        match config.default.as_str() {
            "allow" => PermissionResult::Allowed,
            "block" => PermissionResult::Blocked(format!("Command '{}' is not in allowlist.", binary)),
            _ => PermissionResult::Ask(format!("Unknown command '{}' (default policy is ask).", binary)),
        }
    }

    /// Rewrites the output to hide the real root path.
    /// Replaces `/home/user/projects` with `/` in the output string.
    pub fn virtualize_output(&self, output: &str) -> String {
        let root_str = self.root_dir.to_string_lossy();
        // Improve this: Naive replace might break some things, but good for MVP.
        // We replace the root path with empty string or "/" ?
        // Usually we want to present it as if root is `/`.
        // So `/home/user/projects/foo` -> `/foo`
        // `/home/user/projects` -> `/`
        
        // Handle exact match
        if output.trim() == root_str {
             return output.replace(&*root_str, "/");
        }
        
        output.replace(&*root_str, "")
    }
}
