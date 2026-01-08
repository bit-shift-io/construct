#![allow(dead_code)]

//! # Logging Service
//!
//! A centralized logging service that can dispatch log messages to multiple sinks (Console, File, Chat).
//! Used primarily for internal auditing and debugging.

use crate::domain::traits::ChatProvider;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use tracing::{Level, debug, error, info, warn};

/// Different destinations for logs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogTarget {
    Console,
    File,
    Chat,
}

/// Centralized Logging Service
/// supports dispatching logs to enabled sinks
pub struct LoggingService<CP: ChatProvider + ?Sized> {
    file_path: Option<String>,
    chat_provider: Option<Arc<CP>>,
    console_enabled: bool,
    min_level: Level,
}

impl<CP: ChatProvider + ?Sized> LoggingService<CP> {
    pub fn new() -> Self {
        Self {
            file_path: None,
            chat_provider: None,
            console_enabled: true,
            min_level: Level::INFO,
        }
    }

    pub fn with_file(mut self, path: String) -> Self {
        self.file_path = Some(path);
        self
    }

    pub fn with_chat(mut self, provider: Arc<CP>) -> Self {
        self.chat_provider = Some(provider);
        self
    }

    pub fn with_console(mut self, enabled: bool) -> Self {
        self.console_enabled = enabled;
        self
    }

    pub fn level(mut self, level: Level) -> Self {
        self.min_level = level;
        self
    }

    pub async fn log(&self, level: Level, message: &str) {
        if level > self.min_level {
            return;
        }

        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let formatted = format!("[{}] [{}] {}", timestamp, level, message);

        // 1. Console
        if self.console_enabled {
            match level {
                Level::ERROR => error!("{}", message),
                Level::WARN => warn!("{}", message),
                Level::INFO => info!("{}", message),
                Level::DEBUG => debug!("{}", message),
                Level::TRACE => tracing::trace!("{}", message),
            }
        }

        // 2. File
        if let Some(path) = &self.file_path
            && let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path)
        {
            let _ = writeln!(file, "{}", formatted);
        }

        // 3. Chat (Async, fire-and-forget style for now, ideally queued)
        // Note: For critical errors or debug mode only to avoid spam
        if let Some(chat) = &self.chat_provider {
            // Only log ERRORs to chat by default, or if explicitly requested?
            // User asked for "options to write to ... chat".
            // Let's assume we log everything if the sink is enabled, but practically
            // we might want a filter. For now, we log everything if sink is present.
            // But we should format it as a code block for readability.
            let _ = chat
                .send_notification(&format!("`[LOG]` {}", message))
                .await;
        }
    }
}
