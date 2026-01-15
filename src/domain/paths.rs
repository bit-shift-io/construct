//! # Project Paths
//!
//! Centralized definitions for project structure and file paths.
//! acts as the Single Source of Truth for where files (specs, tasks, etc.) are located.

pub const SPECS_DIR: &str = "tasks/specs";
pub const ROADMAP_FILE: &str = "roadmap.md";
pub const ARCHITECTURE_FILE: &str = "architecture.md";
pub const PROGRESS_FILE: &str = "progress.md";
pub const GUIDELINES_FILE: &str = "guidelines.md";

/// Returns the relative path to the roadmap file (e.g. "tasks/specs/roadmap.md")
pub fn roadmap_rel() -> String {
    format!("{}/{}", SPECS_DIR, ROADMAP_FILE)
}

/// Returns the relative path to the architecture file
pub fn architecture_rel() -> String {
    format!("{}/{}", SPECS_DIR, ARCHITECTURE_FILE)
}

/// Returns the relative path to the progress file
pub fn progress_rel() -> String {
    format!("{}/{}", SPECS_DIR, PROGRESS_FILE)
}

/// Returns the relative path to the guidelines file
pub fn guidelines_rel() -> String {
    format!("{}/{}", SPECS_DIR, GUIDELINES_FILE)
}

/// Returns the full path to the roadmap file given a project root
pub fn roadmap_path(root: &str) -> String {
    format!("{}/{}", root, roadmap_rel())
}

/// Returns the full path to the architecture file given a project root
pub fn architecture_path(root: &str) -> String {
    format!("{}/{}", root, architecture_rel())
}

/// Returns the full path to the progress file given a project root
pub fn progress_path(root: &str) -> String {
    format!("{}/{}", root, progress_rel())
}

/// Returns the full path to the guidelines file given a project root
pub fn guidelines_path(root: &str) -> String {
    format!("{}/{}", root, guidelines_rel())
}
