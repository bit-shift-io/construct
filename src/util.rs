pub fn get_project_name(path: &str) -> &str {
    if path == "None" || path.is_empty() {
        "None"
    } else {
        path.split('/').last().unwrap_or(path)
    }
}

#[derive(Debug, PartialEq)]
pub enum AgentAction {
    WriteFile(String, String), // filename, content
    ChangeDir(String),         // path
    ReadFile(String),          // path
    ListDir(String),           // path
    ShellCommand(String),
    Done,
}

pub fn parse_actions(response: &str) -> Vec<AgentAction> {
    let mut actions = Vec::new();
    let mut current_pos = 0;

    while let Some(start_marker) = response[current_pos..].find("```") {
        let abs_start = current_pos + start_marker;
        if let Some(end_marker) = response[abs_start + 3..].find("```") {
            let abs_end = abs_start + 3 + end_marker;
            let block_content = &response[abs_start + 3..abs_end];

            // Extract language and content
            let mut lines = block_content.lines();
            let _lang = lines.next().unwrap_or("").trim(); // Skip language e.g. "rust" or "bash"
            let content = lines.collect::<Vec<&str>>().join("\n");
            let content = content.trim().to_string();

            if !content.is_empty() {
                // Check context before the block for WRITE_FILE
                // We look at the text between the previous block (or start) and this block
                let pre_context = &response[current_pos..abs_start];

                // We look for "WRITE_FILE:" occurring before the block.
                // It should be on the line immediately preceding the block (ignoring potential empty lines/whitespace)

                let is_write_file = if let Some(idx) = pre_context.rfind("WRITE_FILE:") {
                    // Check if there is only whitespace between "WRITE_FILE: <filename>" line and the code block
                    let after_found = &pre_context[idx..]; // "WRITE_FILE: filename \n \n"
                    // We expect "WRITE_FILE: ..." then newline then ```

                    // Find end of the WRITE_FILE line
                    if let Some(line_end) = after_found.find('\n') {
                        let between_line_and_block = &after_found[line_end..];
                        between_line_and_block.trim().is_empty()
                    } else {
                        // WRITE_FILE line goes straight to end of string? Unlikely if ``` follows
                        after_found.trim().is_empty() // Fail safe
                    }
                } else {
                    false
                };

                if is_write_file {
                    if let Some(idx) = pre_context.rfind("WRITE_FILE:") {
                        let after_key = &pre_context[idx + 11..]; // skip "WRITE_FILE:"
                        let line_end = after_key.find('\n').unwrap_or(after_key.len());
                        let filename = after_key[..line_end].trim().to_string();
                        actions.push(AgentAction::WriteFile(filename, content));
                    }
                } else if content.trim().starts_with("WRITE_FILE:") {
                    // Fallback: Check if WRITE_FILE is INSIDE the block
                    let mut inner_lines = content.lines();
                    if let Some(first_line) = inner_lines.next() {
                        let filename = first_line.replace("WRITE_FILE:", "").trim().to_string();
                        let file_content = inner_lines.collect::<Vec<&str>>().join("\n");
                        actions.push(AgentAction::WriteFile(filename, file_content));
                    }
                } else if content.trim().starts_with("CHANGE_DIR:") {
                    if let Some(first_line) = content.lines().next() {
                        let path = first_line.replace("CHANGE_DIR:", "").trim().to_string();
                        actions.push(AgentAction::ChangeDir(path));
                    }
                } else if content.trim().starts_with("READ_FILE:") {
                    if let Some(first_line) = content.lines().next() {
                        let path = first_line.replace("READ_FILE:", "").trim().to_string();
                        actions.push(AgentAction::ReadFile(path));
                    }
                } else if content.trim().starts_with("LIST_DIR") {
                    // Check for "LIST_DIR" or "LIST_DIR:" or "LIST_DIR <path>"
                    let trimmed = content.trim();
                    let path_part = if trimmed.starts_with("LIST_DIR:") {
                        trimmed.replacen("LIST_DIR:", "", 1)
                    } else if trimmed.starts_with("LIST_DIR ") {
                        trimmed.replacen("LIST_DIR", "", 1)
                    } else if trimmed == "LIST_DIR" {
                        String::new()
                    } else {
                        // Fallback? probably not LIST_DIR then
                        trimmed.to_string()
                    };

                    let path = path_part.trim().to_string();
                    let final_path = if path.is_empty() {
                        ".".to_string()
                    } else {
                        path
                    };
                    actions.push(AgentAction::ListDir(final_path));
                } else {
                    if content == "DONE" {
                        actions.push(AgentAction::Done);
                    } else {
                        actions.push(AgentAction::ShellCommand(content));
                    }
                }
            }

            // Move clean past the end of this block (+3 for closing backticks)
            current_pos = abs_end + 3;
        } else {
            break;
        }
    }

    actions
}

/// Helper to run a shell command and return stdout/stderr.
pub async fn run_command(command: &str, folder: Option<&str>) -> Result<String, String> {
    use tokio::process::Command;

    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return Err("Empty command".to_string());
    }

    let binary = parts[0];
    let args = &parts[1..];

    let output = Command::new(binary)
        .args(args)
        .current_dir(folder.unwrap_or("."))
        .output()
        .await
        .map_err(|e| {
            crate::prompts::STRINGS
                .messages
                .command_run_failed
                .replace("{}", &e.to_string())
        })?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        if !err.is_empty() {
            Err(err)
        } else {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        }
    }
}

/// Helper to run a raw shell command using `sh -c`.
/// This allows for pipes, redirects, and other shell features.
pub async fn run_shell_command(command: &str, folder: Option<&str>) -> Result<String, String> {
    use tokio::process::Command;

    // Log the command attempt
    log_interaction(
        "SHELL_EXEC",
        "system",
        &format!("Running Command: {}", command),
    );

    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(folder.unwrap_or("."))
        .output()
        .await
        .map_err(|e| {
            crate::prompts::STRINGS
                .messages
                .shell_command_failed
                .replace("{}", &e.to_string())
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let code = output.status.code().unwrap_or(-1);

    let combined = if stderr.is_empty() {
        stdout
    } else {
        format!("{}\n{}", stdout, stderr)
    };

    // Always append exit code so agent knows it finished
    let final_output = format!("{}\n[Exit Code: {}]", combined.trim(), code);

    // Log the output
    log_interaction(
        "SHELL_OUTPUT",
        "system",
        &format!("Exit: {}\nOutput:\n{}", code, combined),
    );

    if output.status.success() {
        Ok(final_output)
    } else {
        Err(final_output)
    }
}

pub fn log_interaction(kind: &str, provider: &str, content: &str) {
    use std::io::Write;
    let timestamp = chrono::Local::now().to_rfc3339();
    let log_entry = format!(
        "--- [{}] {} ({}) ---\n{}\n\n",
        timestamp, kind, provider, content
    );

    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/data/agent.log")
    {
        let _ = file.write_all(log_entry.as_bytes());
    }
}
