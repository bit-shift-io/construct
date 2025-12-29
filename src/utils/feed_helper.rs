use crate::features::feed::FeedManager;
use crate::services::ChatService;

/// Send or edit a message in the feed.
/// If the feed manager has an event ID, edit the existing message.
/// Otherwise, send a new message and store the event ID.
pub async fn update_feed_message<S: ChatService>(
    room: &S,
    feed_manager: &mut FeedManager,
    content: &str,
) -> Result<(), String> {
    if let Some(event_id) = feed_manager.get_event_id() {
        // Edit existing message
        room.edit_markdown(event_id, content)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        // Send new message and store event ID
        let new_event_id = room
            .send_markdown(content)
            .await
            .map_err(|e| e.to_string())?;
        feed_manager.set_event_id(new_event_id);
    }

    Ok(())
}

/// Start a new feed with initial content.
/// Sends a new message and stores the event ID in the feed manager.
pub async fn start_feed<S: ChatService>(
    room: &S,
    feed_manager: &mut FeedManager,
    initial_content: &str,
) -> Result<(), String> {
    let event_id = room
        .send_markdown(initial_content)
        .await
        .map_err(|e| e.to_string())?;
    feed_manager.set_event_id(event_id);
    feed_manager.save_to_disk();
    Ok(())
}

/// Get or create a FeedManager for the current room state.
/// Returns a clone of the existing feed manager, or creates a new one.
pub fn get_or_create_feed_manager(
    current_project_path: Option<String>,
    existing_feed: Option<&FeedManager>,
) -> FeedManager {
    match existing_feed {
        Some(feed) => feed.clone(),
        None => FeedManager::new(current_project_path),
    }
}

/// Format wizard step content for feed display.
pub fn format_wizard_step(
    step: &crate::state::WizardStep,
    _mode: &crate::state::WizardMode,
    buffer: &str,
    data: &std::collections::HashMap<String, String>,
) -> String {
    match step {
        crate::state::WizardStep::ProjectName => {
            "🚀 **New Project Wizard**\n\n\
             Please enter a Project Folder Name (no spaces, e.g., my-cool-app):".to_string()
        }
        crate::state::WizardStep::ProjectType => {
            "📝 **Project Description**\n\n\
             Describe your project. Please include the programming language and tech stack you want to use.\n\n\
             Type .ok to finish and create the project.".to_string()
        }
        crate::state::WizardStep::Description => {
            format!(
                "📝 **Project Description**\n\n\
                 Current description:\n\
                 > {}\n\n\
                 Type .ok to finish or continue adding details.",
                buffer
            )
        }
        crate::state::WizardStep::Confirmation => {
            let name = data.get("name").unwrap_or(&"?".to_string()).clone();
            format!(
                "✅ **Confirm Project Creation**\n\n\
                 **Name:** {}\n\
                 **Description:** {}\n\n\
                 Type .ok to create or .cancel to abort.",
                name, buffer
            )
        }
        crate::state::WizardStep::TaskDescription => {
            if buffer.is_empty() {
                "📝 **Task Description**\n\n\
                 Please describe the task you want to accomplish.\n\n\
                 Type .ok to finish.".to_string()
            } else {
                format!(
                    "📝 **Task Description**\n\n\
                     Current description:\n\
                     > {}\n\n\
                     Type .ok to finish or continue adding details.",
                    buffer
                )
            }
        }
        crate::state::WizardStep::Stack => {
            "🔧 **Tech Stack**\n\n\
             Please describe the tech stack you want to use (frameworks, libraries, etc.).".to_string()
        }
    }
}

/// Build user-friendly feed content from generated plan files.
pub fn format_plan_content(
    plan_content: Option<&str>,
    tasks_content: Option<&str>,
    roadmap_content: Option<&str>,
) -> String {
    let mut feed_content = String::new();

    feed_content.push_str("## 📋 Project Setup Complete\n\n");

    if let Some(plan) = plan_content {
        feed_content.push_str("### Plan\n\n");
        feed_content.push_str(plan);
        feed_content.push_str("\n\n");
    }

    if let Some(tasks) = tasks_content {
        feed_content.push_str("### Tasks\n\n");
        feed_content.push_str(tasks);
        feed_content.push_str("\n\n");
    }

    if let Some(roadmap) = roadmap_content {
        feed_content.push_str("### Roadmap\n\n");
        feed_content.push_str(roadmap);
    }

    feed_content
}

/// Parse a file section from agent output.
/// Extracts content between ```markdown and ``` markers for a specific filename.
pub fn parse_file(filename: &str, output: &str) -> Option<String> {
    let file_marker = format!("{}#", filename);

    // Find the file marker
    let marker_pos = output.find(&file_marker)?;
    let search_from = &output[marker_pos..];
    let start_relative = search_from.find("```markdown")?;

    // Find the closing ``` after the opening ```markdown
    let content_section = &search_from[start_relative + 13..];
    let end_relative = content_section.find("```")?;

    let content = &content_section[..end_relative];
    Some(content.trim().to_string())
}
