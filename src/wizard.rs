use crate::config::AppConfig;
use crate::state::{BotState, WizardMode, WizardState, WizardStep};
use crate::services::ChatService;
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn start_new_project_wizard<S: ChatService + Clone + Send + 'static>(
    state: Arc<Mutex<BotState>>,
    room: &S,
) {
    {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());

        room_state.wizard = WizardState {
            active: true,
            mode: WizardMode::Project,
            step: Some(WizardStep::ProjectName),
            data: std::collections::HashMap::new(),
            buffer: String::new(),
        };

        bot_state.save();
    }
    
    // Initial Render
    render_step(state.clone(), room).await;
}

pub async fn start_task_wizard<S: ChatService + Clone + Send + 'static>(
    state: Arc<Mutex<BotState>>,
    room: &S,
) {
    {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());

        room_state.wizard = WizardState {
            active: true,
            mode: WizardMode::Task,
            step: Some(WizardStep::TaskDescription),
            data: std::collections::HashMap::new(),
            buffer: String::new(),
        };
        
        bot_state.save();
    }
    
    render_step(state.clone(), room).await;
}

pub async fn handle_input<S: ChatService + Clone + Send + 'static>(
    config: &AppConfig,
    state: Arc<Mutex<BotState>>,
    room: &S,
    input: &str,
) {
    // 1. Check control commands
    if input == ".cancel" {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());
        room_state.wizard = WizardState::default(); // Reset
        bot_state.save();
        let _ = room.send_markdown(&crate::prompts::STRINGS.wizard.cancelled).await;
        return;
    }

    // Check for explicit restart
    if input == ".new" {
         {
            let mut bot_state = state.lock().await;
            let room_state = bot_state.get_room_state(&room.room_id());
            room_state.wizard = WizardState {
                active: true,
                mode: WizardMode::Project,
                step: Some(WizardStep::ProjectName),
                data: std::collections::HashMap::new(),
                buffer: String::new(),
            };
            bot_state.save();
         }
         render_step(state.clone(), room).await;
         return;
    }
    
    // Check for task restart
    if input == ".task" {
         {
            let mut bot_state = state.lock().await;
            let room_state = bot_state.get_room_state(&room.room_id());
            room_state.wizard = WizardState {
                active: true,
                mode: WizardMode::Task,
                step: Some(WizardStep::TaskDescription),
                data: std::collections::HashMap::new(),
                buffer: String::new(),
            };
            bot_state.save();
         }
         render_step(state.clone(), room).await;
         return;
    }

    let (step, _mode) = {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());
        (room_state.wizard.step.clone(), room_state.wizard.mode.clone())
    };

    match step {
        Some(WizardStep::ProjectName) => {
            let name = input.trim().to_string();
            if name.is_empty() {
                 let _ = room.send_markdown("Please enter a valid project name or .cancel").await;
                 return;
            }
            update_data(state.clone(), room, "name", &name).await;
            advance_step(state.clone(), room, WizardStep::ProjectType).await;
        }
        Some(WizardStep::ProjectType) => {
             // 1. App, 2. Lib, 3. CLI, 4. Web
             let val = match input.trim() {
                 "1" | "app" => Some("Application"),
                 "2" | "lib" => Some("Library"),
                 "3" | "cli" => Some("CLI Tool"),
                 "4" | "web" => Some("Web Site/App"),
                 _ => None,
             };
             
             if let Some(v) = val {
                 update_data(state.clone(), room, "type", v).await;
                 advance_step(state.clone(), room, WizardStep::Stack).await;
             } else {
                  let _ = room.send_markdown("❌ Invalid selection. Please select 1-4.").await;
                  render_step(state, room).await;
             }
        }
        Some(WizardStep::Stack) => {
             // 1. Rust, 2. Python, 3. TypeScript, 4. Go, 5. Custom
             let val = match input.trim() {
                 "1" | "rust" => "Rust",
                 "2" | "python" => "Python",
                 "3" | "ts" | "typescript" => "TypeScript",
                 "4" | "go" => "Go",
                 val => val, // Treat as custom
             };
             update_data(state.clone(), room, "stack", val).await;
             advance_step(state.clone(), room, WizardStep::Description).await;
        }
        Some(WizardStep::Description) => {
             if input.trim() == ".ok" {
                  // Finalize description
                  advance_step(state.clone(), room, WizardStep::Confirmation).await;
             } else {
                  // Append to buffer
                  append_buffer(state.clone(), room, input).await;
             }
        }
        Some(WizardStep::TaskDescription) => {
             if input.trim() == ".ok" {
                 // Finalize
                 advance_step(state.clone(), room, WizardStep::Confirmation).await;
             } else {
                 append_buffer(state.clone(), room, input).await;
             }
        }
        Some(WizardStep::Confirmation) => {
             if input.trim() == ".ok" {
                  // Trigger generation
                  finish_wizard(config, state.clone(), room).await;
             } else {
                   let _ = room.send_markdown("Type .ok to generate or .cancel to abort.").await;
             }
        }
        None => {}
    }
}

async fn update_data<S: ChatService>(state: Arc<Mutex<BotState>>, room: &S, key: &str, val: &str) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    room_state.wizard.data.insert(key.to_string(), val.to_string());
    bot_state.save();
}

async fn append_buffer<S: ChatService>(state: Arc<Mutex<BotState>>, room: &S, input: &str) {
    let mut bot_state = state.lock().await;
    let room_state = bot_state.get_room_state(&room.room_id());
    if !room_state.wizard.buffer.is_empty() {
        room_state.wizard.buffer.push('\n');
    }
    room_state.wizard.buffer.push_str(input);
    bot_state.save();
}

async fn advance_step<S: ChatService + Clone + Send + 'static>(state: Arc<Mutex<BotState>>, room: &S, next_step: WizardStep) {
    {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());
        room_state.wizard.step = Some(next_step);
        bot_state.save();
    }
    render_step(state, room).await;
}

async fn render_step<S: ChatService>(state: Arc<Mutex<BotState>>, room: &S) {
     let (step, buffer_len, mode) = {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());
        (room_state.wizard.step.clone(), room_state.wizard.buffer.len(), room_state.wizard.mode.clone())
    };

    let msg = match step {
        Some(WizardStep::ProjectName) => {
            crate::prompts::STRINGS.wizard.project_name.clone()
        }
        Some(WizardStep::ProjectType) => {
            crate::prompts::STRINGS.wizard.project_type.clone()
        }
        Some(WizardStep::Stack) => {
             crate::prompts::STRINGS.wizard.stack.clone()
        }
        Some(WizardStep::Description) => {
            if buffer_len == 0 {
                crate::prompts::STRINGS.wizard.description.clone()
            } else {
                // Continuation prompt, maybe minimal
                // Actually we don't spam on every message in description phase. 
                // Only if entering the phase.
                // We'll rely on "advance_step" calling this.
                // If we are already in description phase and just appended, we don't call render_step.
                return; 
            }
        }
        Some(WizardStep::TaskDescription) => {
            if buffer_len == 0 {
                 crate::prompts::STRINGS.wizard.task_description.clone()
            } else {
                 return;
            }
        }
        Some(WizardStep::Confirmation) => {
             let mut bot_state = state.lock().await;
             let room_state = bot_state.get_room_state(&room.room_id());
             let d = &room_state.wizard.data;
             let desc = &room_state.wizard.buffer;
             
             if mode == WizardMode::Task {
                 format!("**✅ Confirm Task**\n\n**Requirements**:\n{}\n\nType `.ok` to start task.", desc)
             } else {
                 format!("**✅ Review Project**\n\n**Name**: {}\n**Type**: {}\n**Stack**: {}\n**Description**: {} chars\n\nType `.ok` to generate plan.", 
                    d.get("name").unwrap_or(&"?".to_string()),
                    d.get("type").unwrap_or(&"?".to_string()),
                    d.get("stack").unwrap_or(&"?".to_string()),
                    desc.len()
                 )
             }
        }
        None => return,
    };

    let _ = room.send_markdown(&msg).await;
}

async fn finish_wizard<S: ChatService + Clone + Send + 'static>(config: &AppConfig, state: Arc<Mutex<BotState>>, room: &S) {
    let mode = {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());
        room_state.wizard.mode.clone()
    };
    
    if mode == WizardMode::Task {
        let desc = {
            let mut bot_state = state.lock().await;
            let room_state = bot_state.get_room_state(&room.room_id());
            let desc = room_state.wizard.buffer.clone();
            room_state.wizard = WizardState::default();
            bot_state.save();
            desc
        };
        
        let prompt = crate::prompts::STRINGS.prompts.task_requirements_prompt.replace("{}", &desc);
        crate::commands::handle_task(config, state.clone(), &prompt, room).await;
        return;
    }

    let (name, ptype, stack, desc) = {
        let mut bot_state = state.lock().await;
        let room_state = bot_state.get_room_state(&room.room_id());
        let d = &room_state.wizard.data;
        let desc = room_state.wizard.buffer.clone();
        
        let n = d.get("name").unwrap_or(&"unnamed".to_string()).clone();
        let t = d.get("type").unwrap_or(&"app".to_string()).clone();
        let s = d.get("stack").unwrap_or(&"rust".to_string()).clone();
        
        // Reset wizard
        room_state.wizard = WizardState::default();
        bot_state.save();
        
        (n, t, s, desc)
    };
    
    // Construct the task arguments for the agent
    let prompt = crate::prompts::STRINGS.prompts.new_project_prompt
        .replace("{}", &ptype)
        .replace("{}", &name)
        .replace("{}", &stack)
        .replace("{}", &desc);
    
    // Trigger .new / .task logic
    // We reuse commands::handle_new (which creates dir) AND commands::handle_task (which generates plan).
    // Or we just call handle_new then handle_task.
    
    // 1. Create Project Dir
    // Replaced create_new_project with handle_new
    crate::commands::handle_new(config, state.clone(), &name, room).await;
    
    // 2. Start Task
    crate::commands::handle_task(config, state.clone(), &prompt, room).await;
}
