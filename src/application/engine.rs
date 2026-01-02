//! # Execution Engine
//!
//! The core loop that drives the agent's autonomous behavior.
//! It manages the cycle of thinking, acting, and observing, interfacing with the LLM and MCP.

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::path::Path;

use crate::domain::config::AppConfig;
use crate::domain::traits::LlmProvider;
use crate::application::feed::FeedManager;
use crate::infrastructure::tools::executor::SharedToolExecutor;
use crate::domain::traits::ChatProvider; // Keep ChatProvider for run_task method

use crate::application::state::BotState;

#[derive(Clone)]
pub struct ExecutionEngine {
    _config: AppConfig,
    llm: Arc<dyn LlmProvider>,
    tools: SharedToolExecutor,
    feed: Arc<Mutex<FeedManager>>,
    state: Arc<Mutex<BotState>>,
}

impl ExecutionEngine {
    pub fn new(
        config: AppConfig,
        llm: Arc<dyn LlmProvider>,
        tools: SharedToolExecutor,
        feed: Arc<Mutex<FeedManager>>,
        state: Arc<Mutex<BotState>>,
    ) -> Self {
        Self {
            _config: config,
            llm,
            tools,
            feed,
            state,
        }
    }

    /// Primary execution loop
    pub async fn run_task(&self, chat: &impl ChatProvider, task: &str, agent_name: &str, working_dir: Option<String>) -> Result<bool> {
        // Initialize Feed
        {
            let mut feed = self.feed.lock().await;
            feed.initialize(task.to_string());
            let _ = feed.update_feed(chat).await;
        }

        let max_steps = 20;
        let mut steps = 0;
        let mut history = String::new();

        loop {
            if steps >= max_steps {
                let _ = chat.send_notification("⚠️ Max steps reached.").await;
                break;
            }
            steps += 1;

            // Check for Stop Request
            {
                let mut guard = self.state.lock().await;
                let room = guard.get_room_state(&chat.room_id());
                if room.stop_requested {
                    room.stop_requested = false; // Reset flag
                    let _ = chat.send_notification("🛑 **Task Stopped by User**").await;
                     // Update Feed to Failed/Stopped
                    {
                        let mut feed = self.feed.lock().await;
                        feed.update_last_entry("Task Stopped".to_string(), false);
                         let _ = feed.update_feed(chat).await;
                    }
                    return Ok(false); // Stopped
                }
            }

            // 1. Build Context
            // TODO: Extract to ContextEngine
            let (tasks_content, roadmap_content) = if let Some(wd) = &working_dir {
                // Use MCP to read files strictly
                let client = self.tools.lock().await;
                let tasks = client.read_file(&format!("{}/tasks.md", wd)).await.unwrap_or_else(|_| "(No tasks.md)".into());
                let roadmap = client.read_file(&format!("{}/roadmap.md", wd)).await.unwrap_or_else(|_| "(No roadmap.md)".into());
                (tasks, roadmap)
            } else {
                ("(No context)".into(), "(No context)".into())
            };

            let cwd_msg = working_dir.as_deref().unwrap_or(".");
            let prompt = crate::strings::prompts::interactive_turn(cwd_msg, &roadmap_content, &tasks_content);
            
            // Allow history to grow?
            let full_prompt = format!("{}\n\nHistory:\n{}\n\nUser Question/Task: {}\n\n{}", 
                crate::strings::prompts::SYSTEM, history, task, prompt);

            // 2. LLM Completion
            let _ = chat.typing(true).await;
            
            // Pass agent_name directly to LlmProvider (which routes via Client)
            let response = match self.llm.completion(&full_prompt, agent_name).await {
                Ok(r) => r,
                Err(e) => {
                    let _ = chat.send_notification(&format!("LLM Error: {}", e)).await;
                    break;
                }
            };
            let _ = chat.typing(false).await;

            // 3. Parse Actions
            history.push_str(&format!("\n\nAgent: {}\n", response));
            let actions = crate::application::parsing::parse_actions(&response);

            if actions.is_empty() {
                // Conversational response
                let _ = chat.send_message(&response).await;
                // Wait for user reply? Or stop? 
                // For "Task" execution, we usually expect actions.
                // If it's just talking, we can consider the loop "paused" or "waiting for user".
                // But this run_task is a blocking loop. 
                // We'll break for now to release control.
                break;
            }

            // 4. Execute Actions
            for action in actions {
                match action {
                    crate::domain::types::AgentAction::Done => {
                        let mut feed = self.feed.lock().await;
                        feed.process_action(&crate::domain::types::AgentAction::Done).await;
                        let _ = feed.update_feed(chat).await;
                        return Ok(true); // Completed
                    }
                    crate::domain::types::AgentAction::ShellCommand(cmd) => {
                         // Update Feed
                        {
                            let mut feed = self.feed.lock().await;
                            feed.process_action(&crate::domain::types::AgentAction::ShellCommand(cmd.clone())).await;
                            let _ = feed.update_feed(chat).await;
                        }

                        // Execute via ToolExecutor
                        // We use a simplified direct execution for now, assuming ToolExecutor handles safety/timeouts logic
                        let client = self.tools.lock().await;
                        let output = client.execute_command(&cmd, Path::new(working_dir.as_deref().unwrap_or("."))).await;
                        
                        let (out_str, success) = match output {
                            Ok(o) => (o, true), // We need to check if output contains error codes? 
                            Err(e) => (format!("Error: {}", e), false),
                        };
                        
                        let refined_success = success && !out_str.contains("[Exit Code:") && !out_str.contains("Failed:");

                        // Update Feed Result
                        {
                            let mut feed = self.feed.lock().await;
                            feed.update_last_entry(out_str.clone(), refined_success);
                            let _ = feed.update_feed(chat).await;
                        }

                        // Append to history
                        history.push_str(&format!("\nOutput:\n{}\n", out_str));
                    }
                }
            }
        }
        
        Ok(true) // Loop finished (max steps or conversational break) - default to success? Or failure?
    }
}
