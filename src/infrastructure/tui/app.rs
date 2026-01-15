use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Wrap},
    Frame,
};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

use matrix_sdk::Client as MatrixClient;

use crate::application::project::ProjectManager;
use crate::application::router::CommandRouter;
use crate::domain::config::AppConfig;
use crate::infrastructure::llm::Client as LlmClient;
use crate::infrastructure::tools::executor::ToolExecutor;
use crate::application::state::BotState;
use crate::infrastructure::matrix::MatrixService;

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
    pub timestamp: String,
}

pub struct TuiApp {
    pub config: AppConfig,
    pub state: Arc<Mutex<BotState>>,
    pub tools: Arc<Mutex<ToolExecutor>>,
    pub llm: Arc<LlmClient>,
    pub project_manager: Arc<ProjectManager>,
    pub logs: Arc<Mutex<VecDeque<LogEntry>>>,
    pub matrix_client: MatrixClient,
    
    pub active_tab: usize,
    #[allow(dead_code)]
    pub active_room_id: Option<String>,
    pub input_buffer: String,
    #[allow(dead_code)]
    pub scroll_offset: u16,
    pub should_quit: bool,
}

impl TuiApp {
    pub fn new(
        config: AppConfig,
        state: Arc<Mutex<BotState>>,
        tools: Arc<Mutex<ToolExecutor>>,
        llm: Arc<LlmClient>,
        project_manager: Arc<ProjectManager>,
        logs: Arc<Mutex<VecDeque<LogEntry>>>,
        matrix_client: MatrixClient,
    ) -> Self {
        Self {
            config,
            state,
            tools,
            llm,
            project_manager,
            logs,
            matrix_client,
            active_tab: 0,
            active_room_id: None,
            input_buffer: String::new(),
            scroll_offset: 0,
            should_quit: false,
        }
    }

    pub async fn run(&mut self, mut terminal: ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>) -> Result<()> {
        loop {
            // Update active room list to keep tabs valid
            {
               // Just to refresh active room ID logic if needed
            }

            terminal.draw(|f| self.draw(f))?;

            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                     if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Char('c') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                                self.should_quit = true;
                            }
                            KeyCode::Tab | KeyCode::Right => {
                                self.next_tab().await;
                            }
                            KeyCode::BackTab | KeyCode::Left => {
                                self.prev_tab().await;
                            }
                            KeyCode::Char(c) => {
                                self.input_buffer.push(c);
                            }
                            KeyCode::Backspace => {
                                self.input_buffer.pop();
                            }
                            KeyCode::Enter => {
                                self.handle_submit().await;
                            }
                            _ => {}
                        }
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }
        Ok(())
    }
    
    // ... draw functions omitted if unchanged ...

    fn draw(&self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Tabs
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Input
            ])
            .split(frame.area());

        self.draw_tabs(frame, chunks[0]);
        self.draw_content(frame, chunks[1]);
        self.draw_input(frame, chunks[2]);
    }

    fn draw_tabs(&self, frame: &mut Frame, area: Rect) {
        // Collect active rooms
        // Simple logic: 0 = LOGS, 1..N = Active Rooms
        let mut titles = vec!["📝 Logs".to_string()];
        
        // This blocking lock in draw is suboptimal but okay for TUI
        if let Ok(guard) = self.state.try_lock() {
             for (id, _room) in &guard.rooms {
                 // Resolve Friendly Name from Config (Reverse Lookup)
                 let mut display_name = id.clone();
                 for (bridge_name, entries) in &self.config.bridges {
                     for entry in entries {
                         if let Some(channel) = &entry.channel {
                             if channel == id {
                                 display_name = bridge_name.clone();
                                 break;
                             }
                         }
                     }
                     if display_name != *id { break; }
                 }

                 // Check if active or has task
                 // Simplified: Just use the name
                 titles.push(display_name);
             }
        }
        
        let tabs = Tabs::new(titles)
            .block(Block::default().borders(Borders::ALL).title("Construct Mission Control"))
            .select(self.active_tab)
            .highlight_style(Style::default().fg(Color::Yellow));
        frame.render_widget(tabs, area);
    }

    fn draw_content(&self, frame: &mut Frame, area: Rect) {
        if self.active_tab == 0 {
            self.draw_logs(frame, area);
        } else {
            self.draw_room(frame, area);
        }
    }

    fn draw_logs(&self, frame: &mut Frame, area: Rect) {
         if let Ok(logs) = self.logs.try_lock() {
             // Show last 50 lines, but in chronological order (Oldest -> Newest)
             // This ensures new logs appear at the bottom.
             let skip_count = logs.len().saturating_sub(50);
             let items: Vec<ListItem> = logs.iter().skip(skip_count).map(|entry| {
                 let style = match entry.level.as_str() {
                     "ERROR" => Style::default().fg(Color::Red),
                     "WARN" => Style::default().fg(Color::Yellow),
                     "INFO" => Style::default().fg(Color::Green),
                     _ => Style::default(),
                 };
                 ListItem::new(Line::from(vec![
                     Span::styled(format!("{} [{}] ", entry.timestamp, entry.level), style),
                     Span::raw(&entry.message),
                 ]))
             }).collect();
             
             let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Logs"));
             frame.render_widget(list, area);
         }
    }

    fn draw_room(&self, frame: &mut Frame, area: Rect) {
        // Find which room corresponds to active_tab
        // We need to replicate the ordering logic used in draw_tabs
         if let Ok(guard) = self.state.try_lock() {
             let mut rooms: Vec<(&String, &crate::application::state::RoomState)> = guard.rooms.iter().collect();
             // sort to be stable
             rooms.sort_by_key(|k| k.0);
             
             let room_idx = self.active_tab - 1; // 0 is logs
             if let Some((id, room_state)) = rooms.get(room_idx) {
                 // Render Feed
                 // We need to access FeedManager. It is inside Arc<Mutex>.
                 let content = if let Some(feed_arc) = &room_state.feed_manager {
                     if let Ok(feed) = feed_arc.try_lock() {
                         feed.get_feed_content()
                     } else {
                         "Feed Locked".to_string()
                     }
                 } else {
                     "No Feed Manager".to_string()
                 };

                 // Resolve Friendly Name from Config (Reverse Lookup)
                 let mut display_name = (*id).clone();
                 for (bridge_name, entries) in &self.config.bridges {
                     for entry in entries {
                         if let Some(channel) = &entry.channel {
                             if channel == *id {
                                 display_name = bridge_name.clone();
                                 break;
                             }
                         }
                     }
                     if display_name != **id { break; }
                 }

                 let p = Paragraph::new(content)
                    .wrap(Wrap { trim: true })
                    .block(Block::default().borders(Borders::ALL).title(format!("Room: {}", display_name)));
                 frame.render_widget(p, area);
             } else {
                  let p = Paragraph::new("Room Not Found (Index Error)")
                    .block(Block::default().borders(Borders::ALL));
                 frame.render_widget(p, area);
             }
         }
    }
    
    fn draw_input(&self, frame: &mut Frame, area: Rect) {
         let p = Paragraph::new(self.input_buffer.as_str())
            .block(Block::default().borders(Borders::ALL).title("Input (> to send command)"));
        frame.render_widget(p, area);
    }

    async fn next_tab(&mut self) {
        // We need count of rooms + 1
        let count = {
            let guard = self.state.lock().await;
            guard.rooms.len() + 1
        };
        self.active_tab = (self.active_tab + 1) % count;
    }
    
    async fn prev_tab(&mut self) {
        // We need count of rooms + 1
        let count = {
             let guard = self.state.lock().await;
            guard.rooms.len() + 1
        };
        if self.active_tab > 0 {
            self.active_tab -= 1;
        } else {
            self.active_tab = count - 1;
        }
    }
    
    async fn handle_submit(&mut self) {
        if self.input_buffer.is_empty() { return; }
        
        let router = CommandRouter::new(
            self.config.clone(),
            self.tools.clone(),
            self.llm.clone(),
            self.project_manager.clone(),
            self.state.clone()
        );

        if self.active_tab == 0 {
             // Log tab input?
             // Maybe global commands?
        } else {
             // Room input
             let room_idx = self.active_tab - 1;
             
             // Fix: Clone the room_id string to extend its lifetime beyond the lock guard
             let room_id = {
                 let guard = self.state.lock().await;
                 let mut rooms: Vec<&String> = guard.rooms.keys().collect();
                 rooms.sort();
                 rooms.get(room_idx).map(|s| (*s).clone()) // Clone the String itself
             };

             if let Some(id) = room_id {
                 // Try to get actual room from Client
                 if let Some(room) = self.matrix_client.get_room(id.as_str().try_into().unwrap()) {
                     let chat = MatrixService::new(room);
                     // Dispatch
                     let _ = router.route(&chat, &self.input_buffer, "Operator").await;
                 } else {
                      // Fallback or error?
                      // If room not found in client, we can't send.
                 }
             }
        }
        
        self.input_buffer.clear();
    }
}
