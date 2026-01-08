//! # Application Layer
//!
//! Contains the core business logic and orchestration of the bot.
//! This includes the execution engine, command routing, state management, and feed system.

pub mod engine;
pub mod feed;
pub mod feed_formatter;
pub mod logging;
pub mod parsing;
pub mod project;
pub mod router;
pub mod state;
pub mod utils;
