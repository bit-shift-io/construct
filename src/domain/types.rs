//! # Domain Types
//!
//! Common data structures and enums used across the application logic.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentAction {
    ShellCommand(String),
    WriteFile(String, String), // path, content
    ReadFile(String),          // path
    ListDir(String),           // path
    Find(String, String),      // path, pattern
    SwitchMode(String),        // phase (planning, execution)
    Done,
}
