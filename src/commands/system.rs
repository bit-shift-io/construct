use crate::core::config::AppConfig;
use std::collections::HashSet;
use std::time::Duration;

// Helper functions are available for future use
#[allow(dead_code)]

/// Intelligent command timeout selector
/// Determines appropriate timeout based on command type and patterns
pub fn get_command_timeout(command: &str, config: &AppConfig) -> Duration {
    let cmd_lower = command.to_lowercase();
    let first_word = command
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_lowercase();

    // Short timeout commands (30s) - quick file operations and searches
    let short_commands: HashSet<&str> = vec![
        "ls", "cat", "head", "tail", "pwd", "echo", "grep", "find", "wc", "date", "whoami", "id",
        "uptime", "df", "du", "basename", "dirname", "realpath", "readlink", "file", "stat",
        "touch", "mkdir", "ln", "rm", "rmdir",
    ]
    .into_iter()
    .collect();

    // Long timeout commands (600s) - build systems and package managers
    let long_commands: HashSet<&str> = vec![
        "cargo", "npm", "yarn", "pnpm", "pip", "pip3", "python3", "python", "node", "bun", "deno",
        "go", "rustc", "gcc", "g++", "clang", "cmake", "make", "ninja", "bazel", "buck", "gradle",
        "maven", "docker", "podman", "git", "svn", "hg",
    ]
    .into_iter()
    .collect();

    // Check for long-running patterns first
    if cmd_lower.contains("cargo build") || cmd_lower.contains("cargo test") {
        return Duration::from_secs(config.commands.timeouts.long);
    }

    if cmd_lower.contains("npm install") || cmd_lower.contains("npm ci") {
        return Duration::from_secs(config.commands.timeouts.long);
    }

    if cmd_lower.contains("pip install") || cmd_lower.contains("pip3 install") {
        return Duration::from_secs(config.commands.timeouts.long);
    }

    if cmd_lower.contains("git clone") || cmd_lower.contains("git pull") {
        return Duration::from_secs(config.commands.timeouts.long);
    }

    // Check for long commands by binary name
    if long_commands.contains(first_word.as_str()) {
        return Duration::from_secs(config.commands.timeouts.long);
    }

    // Check for short commands by binary name
    if short_commands.contains(first_word.as_str()) {
        return Duration::from_secs(config.commands.timeouts.short);
    }

    // Default to medium timeout
    Duration::from_secs(config.commands.timeouts.medium)
}

/// Validate file operations for sandbox safety
pub fn validate_file_operation(command: &str, sandbox_root: &str) -> Result<(), String> {
    let parts: Vec<&str> = command.split_whitespace().collect();

    // Extract file paths from common operations
    for (i, part) in parts.iter().enumerate() {
        match *part {
            "cat" | "head" | "tail" | "less" | "more" => {
                if i + 1 < parts.len() {
                    validate_path(parts[i + 1], sandbox_root)?;
                }
            }
            "ls" => {
                if i + 1 < parts.len() && !parts[i + 1].starts_with('-') {
                    validate_path(parts[i + 1], sandbox_root)?;
                }
            }
            ">" | ">>" | "<" => {
                if i + 1 < parts.len() {
                    validate_path(parts[i + 1], sandbox_root)?;
                }
            }
            "cd" => {
                if i + 1 < parts.len() {
                    validate_path(parts[i + 1], sandbox_root)?;
                }
            }
            "rm" | "mv" | "cp" => {
                // Validate all arguments after these commands
                for arg in parts.iter().skip(i + 1) {
                    if !arg.starts_with('-') {
                        validate_path(arg, sandbox_root)?;
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}

/// Validate that a path doesn't escape the sandbox
fn validate_path(path: &str, sandbox_root: &str) -> Result<(), String> {
    use crate::strings::messages;
    use std::path::Path;

    // Skip validation for flags and options
    if path.starts_with('-') {
        return Ok(());
    }

    // Resolve the full path
    let full_path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("{}/{}", sandbox_root, path)
    };

    // Try to canonicalize to resolve .. and symlinks
    match std::fs::canonicalize(&full_path) {
        Ok(canon) => {
            if !canon.starts_with(sandbox_root) {
                return Err(messages::sandbox_escape_error(path));
            }
        }
        Err(_) => {
            // Path doesn't exist yet - check parent directory
            if let Some(parent) = Path::new(&full_path).parent() {
                if let Ok(canon_parent) = std::fs::canonicalize(parent) {
                    if !canon_parent.starts_with(sandbox_root) {
                        return Err(messages::sandbox_escape_parent_error(path));
                    }
                }
            }
        }
    }

    Ok(())
}

/// Helper to generate safe file write commands
pub fn write_file_command(filename: &str, content: &str) -> String {
    if content.contains('\n') || content.len() > 100 {
        format!("cat << 'EOF' > {}\n{}\nEOF", filename, content)
    } else {
        format!("echo '{}' > {}", content, filename)
    }
}

/// Helper to generate file read commands
pub fn read_file_command(path: &str) -> String {
    format!("cat {}", path)
}

/// Helper to generate directory listing commands
pub fn list_dir_command(path: Option<&str>) -> String {
    match path {
        Some(p) => format!("ls -la {}", p),
        None => "ls -la".to_string(),
    }
}

/// Helper to generate directory change commands
pub fn change_dir_command(path: &str) -> String {
    format!("cd {}", path)
}
