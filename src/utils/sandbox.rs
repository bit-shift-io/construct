use crate::config::CommandsConfig;
use std::path::PathBuf;

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
    #[allow(dead_code)] // Will be used in commands.rs
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
    #[allow(dead_code)] // Keeping potential future utility
    pub fn resolve_path(&self, current_virtual_cwd: &str, target_path: &str) -> Option<PathBuf> {
        // Handle "cd /" or absolute paths as being relative to root
        let base = if target_path.starts_with('/') {
            self.root_dir.clone()
        } else {
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
                None
            }
        }
    }

    /// Checks if a command binary is allowed/blocked.
    /// Handles chained commands (&&, ||, ;, |) and detects subshells.
    pub fn check_command(&self, command_line: &str, config: &CommandsConfig) -> PermissionResult {
        // 1. Detect potentially dangerous sub-shell execution that we can't easily parse
        if command_line.contains("$(") || command_line.contains('`') {
            return PermissionResult::Ask(format!(
                "Complex subshell execution detected in: {}",
                command_line
            ));
        }

        // 2. Split into individual commands respecting quotes
        let commands = self.split_shell_commands(command_line);
        if commands.is_empty() {
            return PermissionResult::Allowed;
        }

        let mut final_result = PermissionResult::Allowed;

        // 3. Check EACH command in the chain
        for cmd in commands {
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            // Handle sudo prefix - check the actual command
            let binary = if parts[0] == "sudo" && parts.len() > 1 {
                parts[1]
            } else {
                parts[0]
            };

            // Check status for this specific binary
            let result = self.check_single_binary(binary, config);

            match result {
                PermissionResult::Blocked(msg) => {
                    // If ANY part is blocked, the whole chain is blocked immediately
                    return PermissionResult::Blocked(msg);
                }
                PermissionResult::Ask(msg) => {
                    // If ANY part needs asking, we upgrade the final result to Ask
                    // (unless we later hit a Blocked, which we check first)
                    final_result = PermissionResult::Ask(msg);
                }
                PermissionResult::Allowed => {
                    // If Allowed, continue checking others
                }
            }
        }

        final_result
    }

    /// Enhanced check for file operations - validates both command AND paths
    /// This extends check_command to also validate file paths stay within sandbox
    pub fn check_file_operation(
        &self,
        command_line: &str,
        config: &CommandsConfig,
    ) -> PermissionResult {
        // First check command permissions using existing logic
        let cmd_permission = self.check_command(command_line, config);

        if matches!(cmd_permission, PermissionResult::Blocked(_)) {
            return cmd_permission;
        }

        // Additional path validation for file operations
        if let Some(paths) = self.extract_file_paths(command_line) {
            for path in paths {
                if !self.is_path_safe(&path) {
                    return PermissionResult::Blocked(format!(
                        "Path '{}' escapes sandbox boundary",
                        path
                    ));
                }
            }
        }

        cmd_permission
    }

    /// Extract file paths from shell commands for validation
    fn extract_file_paths(&self, command: &str) -> Option<Vec<String>> {
        let mut paths = Vec::new();
        let parts: Vec<&str> = command.split_whitespace().collect();

        // Common file operation patterns
        for (i, part) in parts.iter().enumerate() {
            match *part {
                "cat" | "head" | "tail" => {
                    if i + 1 < parts.len() {
                        paths.push(parts[i + 1].to_string());
                    }
                }
                "ls" => {
                    if i + 1 < parts.len() && !parts[i + 1].starts_with('-') {
                        paths.push(parts[i + 1].to_string());
                    }
                }
                ">" | ">>" | "<" => {
                    if i + 1 < parts.len() {
                        paths.push(parts[i + 1].to_string());
                    }
                }
                "cd" => {
                    if i + 1 < parts.len() {
                        paths.push(parts[i + 1].to_string());
                    }
                }
                "rm" | "mv" | "cp" => {
                    // Validate all arguments after these commands (skip flags)
                    for arg in parts.iter().skip(i + 1) {
                        if !arg.starts_with('-') {
                            paths.push(arg.to_string());
                        }
                    }
                }
                _ => {}
            }
        }

        if paths.is_empty() { None } else { Some(paths) }
    }

    /// Check if path stays within sandbox boundaries
    fn is_path_safe(&self, path: &str) -> bool {
        use std::path::Path;

        // Skip validation for flags and options
        if path.starts_with('-') {
            return true;
        }

        let target = if path.starts_with('/') {
            self.root_dir.join(path.trim_start_matches('/'))
        } else {
            self.root_dir.join(path)
        };

        // Try to canonicalize to resolve .. and symlinks
        match std::fs::canonicalize(&target) {
            Ok(canon) => canon.starts_with(&self.root_dir),
            Err(_) => {
                // Path doesn't exist yet - check parent directory
                if let Some(parent) = target.parent() {
                    std::fs::canonicalize(parent)
                        .map(|p| p.starts_with(&self.root_dir))
                        .unwrap_or(false)
                } else {
                    false
                }
            }
        }
    }

    fn check_single_binary(&self, binary: &str, config: &CommandsConfig) -> PermissionResult {
        // 1. Check Blocked
        if config.blocked.iter().any(|b| b == binary) {
            return PermissionResult::Blocked(
                crate::strings::STRINGS
                    .messages
                    .command_blocked
                    .replace("{}", binary),
            );
        }

        // 2. Check Allowed
        if config.allowed.iter().any(|a| a == binary) {
            return PermissionResult::Allowed;
        }

        // 3. Check Ask
        if config.ask.iter().any(|a| a == binary) {
            return PermissionResult::Ask(
                crate::strings::STRINGS
                    .messages
                    .command_ask
                    .replace("{}", binary),
            );
        }

        // 4. Default Mode
        match config.default.as_str() {
            "allow" => PermissionResult::Allowed,
            "block" => PermissionResult::Blocked(
                crate::strings::STRINGS
                    .messages
                    .command_not_allowed
                    .replace("{}", binary),
            ),
            _ => PermissionResult::Ask(
                crate::strings::STRINGS
                    .messages
                    .command_unknown
                    .replace("{}", binary),
            ),
        }
    }

    /// Helper to split shell chains like "cmd1 && cmd2 | cmd3" respecting quotes.
    fn split_shell_commands(&self, input: &str) -> Vec<String> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut in_single = false;
        let mut in_double = false;
        let mut chars = input.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                '\'' if !in_double => in_single = !in_single,
                '"' if !in_single => in_double = !in_double,
                _ => {
                    if !in_single && !in_double {
                        // Separators: ; | &
                        // We treat '&&', '||', '|', '&', ';' as split points
                        if c == ';' {
                            if !current.trim().is_empty() {
                                parts.push(current.trim().to_string());
                            }
                            current.clear();
                            continue;
                        }
                        if c == '|' {
                            if chars.peek() == Some(&'|') {
                                chars.next();
                            } // consume ||
                            if !current.trim().is_empty() {
                                parts.push(current.trim().to_string());
                            }
                            current.clear();
                            continue;
                        }
                        if c == '&' {
                            if chars.peek() == Some(&'&') {
                                chars.next();
                            } // consume &&
                            if !current.trim().is_empty() {
                                parts.push(current.trim().to_string());
                            }
                            current.clear();
                            continue;
                        }
                    }
                }
            }
            current.push(c);
        }
        if !current.trim().is_empty() {
            parts.push(current.trim().to_string());
        }
        parts
    }

    /// Rewrites the output to hide the real root path.
    #[allow(dead_code)]
    pub fn virtualize_output(&self, output: &str) -> String {
        let root_str = self.root_dir.to_string_lossy();
        if output.trim() == root_str {
            return output.replace(&*root_str, "/");
        }
        output.replace(&*root_str, "")
    }
}
