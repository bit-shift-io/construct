//! # New Project Command
//!
//! Handles the `.new` command.
//! Scaffolds a new project directory (roadmap.md, tasks.md) via MCP.

use crate::application::feed::{FeedManager, FeedMode};
use crate::application::project::ProjectManager;
use crate::application::state::{BotState, WizardMode, WizardStep};
use crate::domain::config::AppConfig;
use crate::domain::traits::ChatProvider;
use crate::infrastructure::tools::executor::SharedToolExecutor;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn handle_new(
    config: &AppConfig,
    project_manager: &ProjectManager,
    state: &Arc<Mutex<BotState>>,
    tools: SharedToolExecutor,
    chat: &impl ChatProvider,
    args: &str,
) -> Result<()> {
    // Parse args: .new <project_name> <requirements...>
    let parts: Vec<&str> = args.splitn(2, ' ').collect();

    // If no args (or empty string), Start Wizard
    if args.trim().is_empty() {
        // Initialize Feed
        let feed = Arc::new(Mutex::new(FeedManager::new(
            None,
            config.system.projects_dir.clone(),
            tools.clone(),
            None,
        )));
        
        {
            let mut f = feed.lock().await;
            f.set_title("🧙 New Project Wizard".to_string());
        }
        {
            let mut guard = state.lock().await;
            let room_state = guard.get_room_state(&chat.room_id());
            room_state.wizard.active = true;
            room_state.wizard.mode = WizardMode::Project;
            room_state.wizard.step = Some(WizardStep::ProjectName);
            room_state.wizard.data.clear();

            // Set Feed
            room_state.feed_manager = Some(feed.clone());
        }

        // Initial Wizard Entry
        // Since FeedMode::Wizard renders "Current Step" for 'running' status entries,
        // we just add the first question.
        {
            let mut f = feed.lock().await;
            f.mode = FeedMode::Wizard;
            f.add_prompt("Please enter a **Project Name**.".to_string());
            f.update_feed(chat).await?;
        }

        return Ok(());
    }

    let name = parts[0];
    let _requirements = if parts.len() > 1 { parts[1] } else { "" };

    // Create Project
    let parent_dir = config
        .system
        .projects_dir
        .clone()
        .unwrap_or(".".to_string());
    match project_manager.create_project(name, &parent_dir).await {
        Ok(path) => {
            let _ = chat
                .send_notification(&crate::strings::messages::project_created_notification(
                    name, &path,
                ))
                .await;

            // Switch context
            let mut guard = state.lock().await;
            {
                let room_state = guard.get_room_state(&chat.room_id());
                room_state.current_working_dir = Some(path.clone());
                room_state.current_project_path = Some(path.clone());

                // Create specs directory
                let specs_dir = std::path::Path::new(&path).join("tasks").join("specs");
                let _ = std::fs::create_dir_all(&specs_dir);

                // Write request.md to specs/request.md (Global Request History)
                let current_date = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
                let req_template = crate::strings::templates::REQUEST_TEMPLATE
                        .replace("{{CURRENT_DATE}}", &current_date);
                
                if !_requirements.is_empty() {
                    let req_content = req_template.replace("{{OBJECTIVE}}", _requirements);
                    let _ = std::fs::write(specs_dir.join("request.md"), req_content);
                } else {
                    let req_content = req_template.replace("{{OBJECTIVE}}", "(No initial requirements provided)");
                    let _ = std::fs::write(specs_dir.join("request.md"), req_content);
                }

                // Set phase to NewProject so engine displays roadmap/architecture
                // We CLEAR active_task to ensure we don't carry over old state.
                room_state.active_task = None;
                room_state.task_phase = crate::application::state::TaskPhase::NewProject;
            } // Borrow ends
            guard.save();

            // Optional: Confirm switch? The notification above is minimal.
            // But we already said "created at path". The user expectation is cd.
            // Maybe add a small note?
            // chat.send_notification("Switched to project directory.").await;
            // Stick to the minimal notification for now, or append to it?
            // The notification string is: "Project 'name' created at `path`."
            // Implicitly that's where we are now.
        }
        Err(e) => {
            let _ = chat
                .send_notification(&crate::strings::messages::project_creation_failed(
                    &e.to_string(),
                ))
                .await;
        }
    }

    Ok(())
}
