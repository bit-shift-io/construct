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
    // Regex for WriteFile
    // ```write path/to/file
    // content
    // ```
    let write_regex = Regex::new(r"(?s)```write\s+([^\n]+)\n(.*?)```").unwrap();
    
    // Regex for ReadFile
    // ```read path/to/file```
    let read_regex = Regex::new(r"```read\s+([^`]+)```").unwrap();

    let shell_regex = Regex::new(r"(?s)```bash\n(.*?)```").unwrap();
    let sh_regex = Regex::new(r"(?s)```sh\n(.*?)```").unwrap();

    for caps in write_regex.captures_iter(response) {
        if let (Some(path), Some(content)) = (caps.get(1), caps.get(2)) {
            actions.push(AgentAction::WriteFile(
                path.as_str().trim().to_string(),
                content.as_str().to_string() // Do not trim content, preserve whitespace
            ));
        }
    }

    for caps in read_regex.captures_iter(response) {
        if let Some(path) = caps.get(1) {
            actions.push(AgentAction::ReadFile(path.as_str().trim().to_string()));
        }
    }

    // Regex for ListDir
    // Supports ```list path``` and `list path` and multi-line blocks
    let list_regex = Regex::new(r"(?:```|`)list\s+([^`]+?)\s*(?:```|`)").unwrap();
    for caps in list_regex.captures_iter(response) {
        if let Some(path) = caps.get(1) {
            actions.push(AgentAction::ListDir(path.as_str().trim().to_string()));
        }
    }

    // Only fallback to shell if no specific tool used? 
    // Or allow mixing? Allowing mixing is fine.
    for caps in shell_regex.captures_iter(response) {
        if let Some(cmd) = caps.get(1) {
            actions.push(AgentAction::ShellCommand(cmd.as_str().trim().to_string()));
        }
    }
    for caps in sh_regex.captures_iter(response) {
        if let Some(cmd) = caps.get(1) {
            actions.push(AgentAction::ShellCommand(cmd.as_str().trim().to_string()));
        }
    }

    // Regex for SwitchMode
    // Supports ```switch_mode phase``` and `switch_mode phase` and multi-line blocks
    let switch_regex = Regex::new(r"(?:```|`)switch_mode\s+([a-zA-Z_]+)\s*(?:```|`)").unwrap();
    for caps in switch_regex.captures_iter(response) {
        if let Some(phase) = caps.get(1) {
            actions.push(AgentAction::SwitchMode(phase.as_str().trim().to_string()));
        }
    }
        
    if response.contains("NO_MORE_STEPS") || response.contains("DONE") { // Support both for safety
         actions.push(AgentAction::Done);
    }
    
    actions
}
