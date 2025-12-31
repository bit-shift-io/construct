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












