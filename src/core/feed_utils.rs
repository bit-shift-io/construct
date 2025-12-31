use crate::core::feed::FeedManager;
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




/// Format wizard step content for feed display.
pub fn format_wizard_step(
    step: &crate::core::state::WizardStep,
    _mode: &crate::core::state::WizardMode,
    buffer: &str,
    data: &std::collections::HashMap<String, String>,
) -> String {
    match step {
        crate::core::state::WizardStep::ProjectName => {
            "🚀 **New Project Wizard**\n\n\
             Please enter a Project Folder Name (no spaces, e.g., my-cool-app):".to_string()
        }
        crate::core::state::WizardStep::ProjectType => {
            "📝 **Project Description**\n\n\
             Describe your project. Please include the programming language and tech stack you want to use.\n\n\
             Type .ok to finish and create the project.".to_string()
        }
        crate::core::state::WizardStep::Description => {
            format!(
                "📝 **Project Description**\n\n\
                 Current description:\n\
                 > {}\n\n\
                 Type .ok to finish or continue adding details.",
                buffer
            )
        }
        crate::core::state::WizardStep::Confirmation => {
            let name = data.get("name").unwrap_or(&"?".to_string()).clone();
            format!(
                "✅ **Confirm Project Creation**\n\n\
                 **Name:** {}\n\
                 **Description:** {}\n\n\
                 Type .ok to create or .cancel to abort.",
                name, buffer
            )
        }
        crate::core::state::WizardStep::TaskDescription => {
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
        crate::core::state::WizardStep::Stack => {
            "🔧 **Tech Stack**\n\n\
             Please describe the tech stack you want to use (frameworks, libraries, etc.).".to_string()
        }
    }
}






