pub fn get_project_name(path: &str) -> &str {
    if path == "None" || path.is_empty() {
        "None"
    } else {
        path.split('/').last().unwrap_or(path)
    }
}

#[derive(Debug, PartialEq)]
pub enum AgentAction {
    ShellCommand(String), // All sandboxed system commands
    Done,                 // Completion signal
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
                if content == "DONE" || content.contains("echo DONE") {
                    actions.push(AgentAction::Done);
                } else if !content.contains("**System Command Output:**")
                    && !content.contains("System Command Output:")
                {
                    actions.push(AgentAction::ShellCommand(content));
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
            crate::strings::STRINGS
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

/// Helper to run a raw shell command using `sh -c` with timeout support.
/// This allows for pipes, redirects, and other shell features.
pub async fn run_shell_command(command: &str, folder: Option<&str>) -> Result<String, String> {
    run_shell_command_with_timeout(command, folder, None).await
}

/// Helper to run a raw shell command using `sh -c` with configurable timeout.
/// This allows for pipes, redirects, and other shell features.
pub async fn run_shell_command_with_timeout(
    command: &str,
    folder: Option<&str>,
    timeout: Option<std::time::Duration>,
) -> Result<String, String> {
    use tokio::process::Command;
    use tokio::time::timeout as tokio_timeout;

    // Determine which timeout to use (default to medium)
    let timeout_duration = timeout.unwrap_or_else(|| std::time::Duration::from_secs(120));

    // Log the command attempt
    log_interaction(
        "SHELL_EXEC",
        "system",
        &format!(
            "Running Command: {} (timeout: {:?})",
            command, timeout_duration
        ),
    );

    // Execute command with timeout
    let result = tokio_timeout(
        timeout_duration,
        Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(folder.unwrap_or("."))
            .output(),
    )
    .await;

    let output = match result {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => {
            return Err(crate::strings::STRINGS
                .messages
                .shell_command_failed
                .replace("{}", &e.to_string()));
        }
        Err(_) => {
            return Err(format!(
                "Command timed out after {:?}. Consider breaking this into smaller steps or running in the background.",
                timeout_duration
            ));
        }
    };

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
        .open("data/agent.log")
    {
        let _ = file.write_all(log_entry.as_bytes());
    }
}
