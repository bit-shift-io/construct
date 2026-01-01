
//! # Wizard Strings
//!
//! Strings and templates used by the interactive wizard system.
//! Includes prompts for different wizard steps and modes.

use crate::application::state::{WizardMode, WizardStep};
use std::collections::HashMap;

pub const CANCELLED: &str = "❌ Wizard cancelled.";

pub fn format_wizard_step(
    step: &WizardStep,
    _mode: &WizardMode,
    buffer: &str,
    _data: &HashMap<String, String>,
) -> String {
    let mut output = String::new();
    // Logic to select title/description based on step
    match step {
        WizardStep::ProjectName => {
             output.push_str("### 🧙 New Project Wizard\n\n**Step 1: Project Name**\n\nPlease enter a name for your new project (e.g., `my-awesome-app`).");
        }
        WizardStep::Description => {
             output.push_str("### 📝 Project Description\n\n**Step 2: Description**\n\nDescribe what you want to build.");
        }
        WizardStep::TaskDescription => {
             output.push_str("### 📋 New Task\n\n**Describe the task**\n\nWhat would you like the agent to do?");
        }
        WizardStep::Confirmation => {
             output.push_str("### ✅ Confirmation\n\nReady to proceed? Type `.ok` to start or `.cancel` to abort.");
        }
        _ => {
             output.push_str("### Wizard\n\nUnknown step.");
        }
    }
    
    if !buffer.is_empty() {
        output.push_str("\n\n**Current Input:**\n```\n");
        output.push_str(buffer);
        output.push_str("\n```\n\nType `.ok` to finish this step.");
    }

    output
}
