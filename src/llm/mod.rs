//! Simple LLM API wrapper for multiple providers
//!
//! This module provides a unified interface for interacting with multiple LLM providers
//! (OpenAI, Anthropic, Gemini, Groq, xAI, DeepAI, Zai, etc.) with support for
//! native context caching (Anthropic prompt caching, Gemini context caching).
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use construct::llm::{Client, Context, CacheConfig};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a client from config
//!     let client = Client::new(app_config);
//!
//!     // Simple prompt
//!     let response = client.prompt("openai", "Hello, world!").await?;
//!     println!("Response: {}", response.content);
//!
//!     // With native caching
//!     let context = Context::new()
//!         .add_system_message("You are a helpful assistant.")
//!         .add_user_message("What is 2+2?")
//!         .with_cache(CacheConfig {
//!             max_age_seconds: Some(3600),
//!         });
//!
//!     let response = client.chat("anthropic", context).await?;
//!     println!("Response: {} (cached: {})", response.content, response.cached);
//!
//!     Ok(())
//! }
//! ```

mod client;
pub mod providers;
mod types;

pub use client::Client;

pub use types::{
    Context, Error, Message, MessageRole, Provider, Response, TokenUsage,
};
