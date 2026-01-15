use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{Event, Subscriber};
use tracing_subscriber::Layer;
use chrono::Local;

use crate::infrastructure::tui::LogEntry;

pub struct TuiLogLayer {
    pub logs: Arc<Mutex<VecDeque<LogEntry>>>,
}

impl<S> Layer<S> for TuiLogLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let timestamp = Local::now().format("%H:%M:%S").to_string();
        let level = event.metadata().level().to_string();
        
        let mut visitor = MessageVisitor::new();
        event.record(&mut visitor);
        
        // Simple blocking lock is risky in tracing, but for TUI prototype it's okay?
        // Better to use try_lock or just risk it (it's in a separate thread usually?)
        // Tracing can happen anywhere.
        // For now, we'll try to block_on if possible, or just lock().
        // Since we are in an async runtime, blocking the thread in a tracing call might be bad if the tracing call is in the same thread holding the lock.
        // But `logs` is only held by TuiApp for drawing.
        
        if let Ok(mut guard) = self.logs.try_lock() {
            guard.push_back(LogEntry {
                timestamp,
                level,
                message: visitor.message,
            });
            // Cap capacity
            if guard.len() > 1000 {
                guard.pop_front();
            }
        }
    }
}

struct MessageVisitor {
    message: String,
}

impl MessageVisitor {
    fn new() -> Self {
        Self {
            message: String::new(),
        }
    }
}

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
        }
    }
    
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        }
    }
}
