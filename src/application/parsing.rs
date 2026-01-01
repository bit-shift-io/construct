//! # Parsing Utils
//!
//! Utilities for parsing LLM responses, specifically extracting structured actions (like `ShellCommand`)
//! from the raw text output.

use crate::domain::types::AgentAction;
use regex::Regex;

pub fn parse_actions(response: &str) -> Vec<AgentAction> {
    let mut actions = Vec::new();
    
    // Simple regex for shell commands
    // Format: [COMMAND] cmd [/COMMAND]
    // Or just look for markdown code blocks?
    // The previous implementation used "Action: ShellCommand(...)" logic or similar.
    // Let's check what the user's previous implementation did.
    // Assuming standard "Tool Use" pattern.
    
    // For now, let's implement a robust parser based on common patterns
    // or port specific logic if seen in core/utils.rs
    
    // Re-implementing logic akin to core/utils.rs
    let shell_regex = Regex::new(r"(?s)```bash\n(.*?)```").unwrap();
    // Also support sh
    let sh_regex = Regex::new(r"(?s)```sh\n(.*?)```").unwrap();

    for caps in shell_regex.captures_iter(response) {
        if let Some(cmd) = caps.get(1) {
            actions.push(AgentAction::ShellCommand(cmd.as_str().trim().to_string()));
        }
    }
    for caps in sh_regex.captures_iter(response) {
        if let Some(cmd) = caps.get(1) {
             // Avoid duplicates if both match? simplified logic
            actions.push(AgentAction::ShellCommand(cmd.as_str().trim().to_string()));
        }
    }
    
    if response.contains("TASK_FINISHED") || response.contains("NO_MORE_STEPS") {
         actions.push(AgentAction::Done);
    }
    
    actions
}
