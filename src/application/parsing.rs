//! # Parsing Utils
//!
//! Utilities for parsing LLM responses, specifically extracting structured actions (like `ShellCommand`)
//! from the raw text output.

use crate::domain::types::AgentAction;
use regex::Regex;

pub fn parse_actions(response: &str) -> Vec<(AgentAction, usize, usize)> {
    // (Start, End, Action)
    let mut action_matches: Vec<(usize, usize, AgentAction)> = Vec::new();

    // Regex for WriteFile
    // ```write path/to/file
    // content
    // ```
    let write_regex = Regex::new(r"(?s)```write\s+([^\n]+)\n(.*?)```").unwrap();

    // Regex for ReadFile
    // Supports ```read path```, `read path`
    // Also supports fallback: **Action**: Read `path`
    let read_regex = Regex::new(r"(?i)(?:```|`)read\s+([^`]+?)\s*(?:```|`)").unwrap();
    let read_fallback = Regex::new(r"(?i)\*\*Action\*\*:\s*Read\s+`([^`]+)`").unwrap();

    let shell_regex = Regex::new(r"(?s)```bash\s+(.*?)```").unwrap();
    let sh_regex = Regex::new(r"(?s)```sh\s+(.*?)```").unwrap();
    let run_regex = Regex::new(r"(?s)```run_command\s+(.*?)```").unwrap();

    for caps in write_regex.captures_iter(response) {
        if let (Some(match_node), Some(path), Some(content)) = (caps.get(0), caps.get(1), caps.get(2)) {
            action_matches.push((
                match_node.start(),
                match_node.end(),
                AgentAction::WriteFile(
                    path.as_str().trim().to_string(),
                    content.as_str().to_string(), // Do not trim content, preserve whitespace
                ),
            ));
        }
    }

    // Read matches
    for caps in read_regex.captures_iter(response) {
        if let (Some(match_node), Some(path)) = (caps.get(0), caps.get(1)) {
            action_matches.push((
                match_node.start(),
                match_node.end(),
                AgentAction::ReadFile(path.as_str().trim().to_string()),
            ));
        }
    }
    // Fallback for Read
    for caps in read_fallback.captures_iter(response) {
        if let (Some(match_node), Some(path)) = (caps.get(0), caps.get(1)) {
            tracing::warn!("Parsed fallback action format: Read {}", path.as_str());
            action_matches.push((
                match_node.start(),
                match_node.end(),
                AgentAction::ReadFile(path.as_str().trim().to_string()),
            ));
        }
    }

    // Regex for ListDir
    // Supports ```list path``` and `list path` and multi-line blocks
    let list_regex = Regex::new(r"(?:```|`)list\s+([^`]+?)\s*(?:```|`)").unwrap();
    for caps in list_regex.captures_iter(response) {
        if let (Some(match_node), Some(path)) = (caps.get(0), caps.get(1)) {
            action_matches.push((
                match_node.start(),
                match_node.end(),
                AgentAction::ListDir(path.as_str().trim().to_string()),
            ));
        }
    }

    // Shell matches
    for caps in shell_regex.captures_iter(response) {
        if let (Some(match_node), Some(cmd)) = (caps.get(0), caps.get(1)) {
            action_matches.push((
                match_node.start(),
                match_node.end(),
                AgentAction::ShellCommand(cmd.as_str().trim().to_string()),
            ));
        }
    }
    for caps in sh_regex.captures_iter(response) {
        if let (Some(match_node), Some(cmd)) = (caps.get(0), caps.get(1)) {
            action_matches.push((
                match_node.start(),
                match_node.end(),
                AgentAction::ShellCommand(cmd.as_str().trim().to_string()),
            ));
        }
    }
    for caps in run_regex.captures_iter(response) {
        if let (Some(match_node), Some(cmd)) = (caps.get(0), caps.get(1)) {
            action_matches.push((
                match_node.start(),
                match_node.end(),
                AgentAction::ShellCommand(cmd.as_str().trim().to_string()),
            ));
        }
    }

    // Regex for SwitchMode
    // Supports ```switch_mode phase``` and `switch_mode phase` and multi-line blocks
    let switch_regex = Regex::new(r"(?:```|`)switch_mode\s+([a-zA-Z_]+)\s*(?:```|`)").unwrap();
    for caps in switch_regex.captures_iter(response) {
        if let (Some(match_node), Some(phase)) = (caps.get(0), caps.get(1)) {
            action_matches.push((
                match_node.start(),
                match_node.end(),
                AgentAction::SwitchMode(phase.as_str().trim().to_string()),
            ));
        }
    }

    // Regex for Find
    // Supports ```find path pattern``` and `find path pattern`
    let find_regex = Regex::new(r"(?:```|`)find\s+([^\s]+)\s+([^\s`]+)\s*(?:```|`)").unwrap();
    for caps in find_regex.captures_iter(response) {
        if let (Some(match_node), Some(path), Some(pattern)) = (caps.get(0), caps.get(1), caps.get(2)) {
            action_matches.push((
                match_node.start(),
                match_node.end(),
                AgentAction::Find(
                    path.as_str().trim().to_string(),
                    pattern.as_str().trim().to_string(),
                ),
            ));
        }
    }

    if response.contains("NO_MORE_STEPS") || response.contains("DONE") {
        // We append Done at the end conceptually
        if let Some(idx) = response.find("NO_MORE_STEPS") {
            action_matches.push((idx, idx + "NO_MORE_STEPS".len(), AgentAction::Done));
        } else if let Some(idx) = response.find("DONE") {
            action_matches.push((idx, idx + "DONE".len(), AgentAction::Done));
        }
    }

    // Sort matches by start index to preserve document order
    action_matches.sort_by_key(|k| k.0);

    let actions: Vec<(AgentAction, usize, usize)> = action_matches
        .into_iter()
        .map(|(start, end, action)| (action, start, end))
        .collect();

    if actions.is_empty() && response.contains("Action:") {
        tracing::warn!("Potential unparsed action in response: {}", response);
    }
    
    actions
}
