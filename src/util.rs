pub fn get_project_name(path: &str) -> &str {
    if path == "None" || path.is_empty() {
        "None"
    } else {
        path.split('/').last().unwrap_or(path)
    }
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
        .map_err(|e| crate::prompts::STRINGS.messages.command_run_failed.replace("{}", &e.to_string()))?;

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

    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(folder.unwrap_or("."))
        .output()
        .await
        .map_err(|e| crate::prompts::STRINGS.messages.shell_command_failed.replace("{}", &e.to_string()))?;

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
