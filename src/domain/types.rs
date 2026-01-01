//! # Domain Types
//!
//! Common data structures and enums used across the application logic.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentAction {
    ShellCommand(String),
    // FileOp(String), // Future
    Done,
}
