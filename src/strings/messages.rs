//! # Messages
//!
//! Contains constant strings and format functions for user-facing messages.
//! Includes error messages, status updates, and notification templates.

// New Strings
pub const AUTH_DENIED: &str = "🚫 **Authorization Denied**.";
pub const UNKNOWN_COMMAND: &str = "❓ Unknown command.";

pub fn task_failed(err: &str) -> String {
    format!("❌ **Task Failed**: {err}")
}

pub fn project_created_notification(name: &str, path: &str) -> String {
    format!("Project '{name}' created at `{path}`.")
}

pub fn project_creation_failed(err: &str) -> String {
    format!("Failed to create project: {err}")
}

pub fn directory_changed_msg(path: &str) -> String {
    format!("Changed directory to `{path}`")
}

pub fn invalid_directory(err: &str) -> String {
    format!("Invalid directory: {err}")
}

pub fn command_output_format(workdir: &str, command: &str, output: &str) -> String {
    format!("**[{workdir}]** $ `{command}`\n```\n{output}\n```")
}

pub fn command_failed(err: &str) -> String {
    format!("Command Failed: {err}")
}

pub const READ_USAGE: &str = "Usage: `.read <file_path>`";
pub const ASK_USAGE: &str = "Usage: `.ask <message>`";
pub const PROJECT_USAGE: &str = "Usage: `.project <path>`";

pub fn active_project_set(path: &str) -> String {
    format!("Active project set to: `{path}`")
}

pub fn invalid_project_path(path: &str) -> String {
    format!("Path `{path}` does not appear to be a valid project (missing roadmap.md).")
}

pub fn project_listing_not_implemented(path: &str) -> String {
    format!("Project listing for `{path}` not yet implemented in PM.")
}

pub const NOT_IN_PROJECT: &str =
    "⚠️ You are not inside a project. Use `.new` or `.open` (cd) first.";

pub fn file_read_success(path: &str, content: &str) -> String {
    format!("**File: {path}**\n\n```\n{content}\n```")
}

pub fn file_read_failed(err: &str) -> String {
    format!("Failed to read file: {err}")
}

pub fn room_status_msg(project: &str, cwd: &str, model: &str, agent: &str) -> String {
    format!(
        "**Project**: {project}\n**CWD**: {cwd}\n**Model**: {model}\n**Agent**: {agent}"
    )
}

pub const WIZARD_CANCELLED: &str = "❌ Wizard cancelled.";
// Note: We might want a dynamic one for wizard success to show path, but let's stick to what we see in the code or make it dynamic.
