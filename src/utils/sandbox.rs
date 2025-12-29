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
        // Log when check_command is called
        crate::utils::log_interaction(
            "SANDBOX_START",
            "system",
            &format!("check_command called for: {:?}", command_line),
        );

        // 1. Detect potentially dangerous sub-shell execution that we can't easily parse
        // But ignore backticks inside heredocs (they're just text content)
        let has_heredoc = command_line.contains("<< 'EOF")
            || command_line.contains("<< \"EOF")
            || command_line.contains("<< EOF");
        let has_command_substitution = command_line.contains("$(");
        let has_backticks = if has_heredoc {
            // For heredocs, only check for backticks before the heredoc starts
            if let Some(heredoc_start) = command_line.find("<<") {
                let before_heredoc = &command_line[..heredoc_start];
                before_heredoc.contains('`')
            } else {
                command_line.contains('`')
            }
        } else {
            command_line.contains('`')
        };

        if has_command_substitution || has_backticks {
            return PermissionResult::Ask(format!(
                "Complex subshell execution detected in: {}",
                command_line
            ));
        }

        // 2. Split into individual commands respecting quotes
        let commands = self.split_shell_commands(command_line);

        crate::utils::log_interaction(
            "SANDBOX_AFTER_SPLIT",
            "system",
            &format!("Commands split: {:?}", commands),
        );

        if commands.is_empty() {
            crate::utils::log_interaction(
                "SANDBOX_EMPTY",
                "system",
                "No commands, returning Allowed",
            );
            return PermissionResult::Allowed;
        }

        let mut final_result = PermissionResult::Allowed;

        // 3. Check EACH command in the chain
        for cmd in commands {
            crate::utils::log_interaction(
                "SANDBOX_LOOP",
                "system",
                &format!("Checking command: {:?}", cmd),
            );

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

            crate::utils::log_interaction(
                "SANDBOX_BINARY",
                "system",
                &format!("Binary extracted: {:?}", binary),
            );

            // Check status for this specific binary
            let result = self.check_single_binary(binary, config);

            match result {
                PermissionResult::Blocked(msg) => {
                    // If ANY part is blocked, the whole chain is blocked immediately
                    crate::utils::log_interaction(
                        "SANDBOX_RESULT",
                        "system",
                        &format!("Blocked: {:?}", msg),
                    );
                    return PermissionResult::Blocked(msg);
                }
                PermissionResult::Ask(msg) => {
                    // If ANY part needs asking, we upgrade the final result to Ask
                    // (unless we later hit a Blocked, which we check first)
                    crate::utils::log_interaction(
                        "SANDBOX_RESULT",
                        "system",
                        &format!("Ask: {:?}", msg),
                    );
                    final_result = PermissionResult::Ask(msg);
                }
                PermissionResult::Allowed => {
                    // If Allowed, continue checking others
                    crate::utils::log_interaction(
                        "SANDBOX_RESULT",
                        "system",
                        "Allowed for this binary",
                    );
                }
            }
        }

        crate::utils::log_interaction(
            "SANDBOX_FINAL",
            "system",
            &format!("Final result: {:?}", final_result),
        );
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

        crate::utils::log_interaction(
            "SANDBOX_CMD_PERMISSION",
            "system",
            &format!("Command permission result: {:?}", cmd_permission),
        );

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

        crate::utils::log_interaction(
            "SANDBOX_FILE_OP_RESULT",
            "system",
            &format!("Final file operation result: {:?}", cmd_permission),
        );

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
                // Path doesn't exist yet - check if it would be created within sandbox
                // We allow creation of new files as long as the parent directory exists within sandbox
                if let Some(parent) = target.parent() {
                    match std::fs::canonicalize(parent) {
                        Ok(canon_parent) => canon_parent.starts_with(&self.root_dir),
                        Err(_) => {
                            // Parent doesn't exist either - check if relative path stays within root
                            // For relative paths like "tasks.md", allow if it doesn't try to escape
                            if path.starts_with("../") || path.contains("/../") {
                                false
                            } else if path.starts_with('/') {
                                // Absolute path that doesn't exist
                                false
                            } else {
                                // Relative path like "tasks.md" - allow creation in current directory
                                true
                            }
                        }
                    }
                } else {
                    false
                }
            }
        }
    }

    fn check_single_binary(&self, binary: &str, config: &CommandsConfig) -> PermissionResult {
        // Build a single comprehensive log entry
        let mut log_details = Vec::new();
        log_details.push(format!("Binary: '{}'", binary));
        log_details.push(format!("Allowed: {:?}", config.allowed));
        log_details.push(format!("Ask: {:?}", config.ask));
        log_details.push(format!("Blocked: {:?}", config.blocked));
        log_details.push(format!("Default: {}", config.default));

        // Log immediately before checking
        crate::utils::log_interaction("SANDBOX_CHECK_START", "system", &log_details.join("\n"));

        // 1. Check Blocked
        if config.blocked.iter().any(|b| b == binary) {
            log_details.push(format!("Result: BLOCKED - in blocked list"));
            crate::utils::log_interaction("SANDBOX_CHECK", "system", &log_details.join("\n"));
            return PermissionResult::Blocked(
                crate::strings::STRINGS
                    .messages
                    .command_blocked
                    .replace("{}", binary),
            );
        }

        // 2. Check Allowed
        if config.allowed.iter().any(|a| a == binary) {
            log_details.push(format!("Result: ALLOWED - in allowed list"));
            crate::utils::log_interaction("SANDBOX_CHECK", "system", &log_details.join("\n"));
            return PermissionResult::Allowed;
        }

        // 3. Check Ask
        if config.ask.iter().any(|a| a == binary) {
            log_details.push(format!("Result: ASK - in ask list"));
            crate::utils::log_interaction("SANDBOX_CHECK", "system", &log_details.join("\n"));
            return PermissionResult::Ask(
                crate::strings::STRINGS
                    .messages
                    .command_ask
                    .replace("{}", binary),
            );
        }

        // 4. Default Mode
        let result = match config.default.as_str() {
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
        };
        log_details.push(format!("Result: {:?} - using default", result));
        crate::utils::log_interaction("SANDBOX_CHECK", "system", &log_details.join("\n"));
        result
    }

    /// Helper to split shell chains like "cmd1 && cmd2 | cmd3" respecting quotes.
    /// Now also handles heredocs to avoid splitting content within heredocs.
    fn split_shell_commands(&self, input: &str) -> Vec<String> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut in_single = false;
        let mut in_double = false;
        let mut in_heredoc = false;
        let mut heredoc_delimiter = String::new();
        let mut chars = input.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                '\'' if !in_double && !in_heredoc => in_single = !in_single,
                '"' if !in_single && !in_heredoc => in_double = !in_double,
                '\n' => {
                    // Check if we're starting a heredoc (only if not already in one)
                    if !in_single && !in_double && !in_heredoc {
                        let current_str = current.trim();
                        // Look for heredoc pattern at end of line: << 'DELIM' or << "DELIM" or << DELIM
                        // The pattern should be near the end, possibly followed by redirection
                        if current_str.contains("<<") {
                            // Extract the part after <<
                            if let Some(heredoc_start) = current_str.split("<<").nth(1) {
                                let delimiter = heredoc_start
                                    .trim()
                                    .trim_start_matches('\'')
                                    .trim_start_matches('"')
                                    .trim_end_matches('\'')
                                    .trim_end_matches('"')
                                    .split_whitespace()
                                    .next()
                                    .unwrap_or("");

                                if !delimiter.is_empty() {
                                    in_heredoc = true;
                                    heredoc_delimiter = delimiter.to_string();
                                }
                            }
                        }
                    }
                    current.push(c);

                    // Check if this line ends the heredoc
                    if in_heredoc {
                        let line_content = current.trim();
                        if line_content == heredoc_delimiter {
                            in_heredoc = false;
                            heredoc_delimiter.clear();
                        }
                    }
                }
                _ => {
                    if !in_single && !in_double && !in_heredoc {
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
