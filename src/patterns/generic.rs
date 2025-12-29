//! Generic/language-agnostic error pattern detector
//!
//! Handles detection and suggestion generation for errors that can occur
//! across multiple languages and tools, such as permission issues, disk space,
//! network errors, etc.

use super::{ErrorPattern, PatternDetector};

/// Generic error pattern detector
pub struct GenericPatternDetector;

impl GenericPatternDetector {
    pub fn new() -> Self {
        Self
    }

    /// Detect permission denied errors
    fn detect_permission_error(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("Permission denied")
            && !output.contains("EACCES")
            && !output.contains("Access denied")
        {
            return None;
        }

        Some(ErrorPattern {
            error_type: "permission_error".to_string(),
            pattern_name: "Permission Denied".to_string(),
            suggestion: "You don't have permission to access this resource or execute this command.\n\nCommon causes:\n1. File/directory has restricted permissions\n2. Trying to write to a read-only location\n3. Command requires elevated privileges\n4. File ownership issues\n\nSolutions:\n1. Check file permissions: ls -la\n2. Change permissions if you own the file: chmod +x <file>\n3. Use appropriate directory (e.g., /tmp or your home directory)\n4. Run with sudo if absolutely necessary (use with caution!)\n5. Check file/directory ownership: chown if needed\n6. Use a different directory or file path".to_string(),
            alternative_commands: vec![
                "ls -la <file>".to_string(),
                "chmod +x <file> (to make executable)".to_string(),
                "sudo <command> (use with caution!)".to_string(),
                "cp <file> /tmp/ (to work in temp directory)".to_string(),
            ],
            confidence: 0.90,
        })
    }

    /// Detect disk space errors
    fn detect_disk_space_error(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("No space left on device")
            && !output.contains("ENOSPC")
            && !output.contains("disk full")
        {
            return None;
        }

        Some(ErrorPattern {
            error_type: "disk_space_error".to_string(),
            pattern_name: "Disk Full".to_string(),
            suggestion: "Your disk is out of free space.\n\nImmediate actions:\n1. Clean build artifacts: cargo clean, npm cache clean, go clean, etc.\n2. Remove temporary files: rm -rf /tmp/*\n3. Clean package caches\n4. Remove old log files\n5. Check disk usage: df -h\n\nPrevention:\n- Regularly clean build artifacts\n- Monitor disk space\n- Use disk cleanup tools\n- Consider using external storage or cloud".to_string(),
            alternative_commands: vec![
                "df -h (check disk space)".to_string(),
                "cargo clean (for Rust projects)".to_string(),
                "npm cache clean --force (for Node projects)".to_string(),
                "go clean -cache -testcache (for Go projects)".to_string(),
                "rm -rf target/ node_modules/ (clean build artifacts)".to_string(),
                "du -sh * | sort -h (find large directories)".to_string(),
                "ncdu (interactive disk usage analyzer)".to_string(),
            ],
            confidence: 0.95,
        })
    }

    /// Detect network connectivity errors
    fn detect_network_error(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("network")
            && !output.contains("connection")
            && !output.contains("ECONNREFUSED")
            && !output.contains("timeout")
            && !output.contains("unreachable")
        {
            return None;
        }

        if output.contains("ECONNREFUSED") || output.contains("Connection refused") {
            Some(ErrorPattern {
                error_type: "network_connection_refused".to_string(),
                pattern_name: "Connection Refused".to_string(),
                suggestion: "Connection was refused by the remote host.\nCommon causes:\n1. Service is not running on the target host\n2. Wrong port or hostname\n3. Firewall blocking the connection\n4. Network configuration issues\n5. Service is overwhelmed\n\nTry:\n1. Verify the service is running\n2. Check hostname and port\n3. Test connectivity: ping <host>\n4. Check firewall rules\n5. Try telnet <host> <port>".to_string(),
                alternative_commands: vec![
                    "ping <host>".to_string(),
                    "curl -v <url>".to_string(),
                    "telnet <host> <port>".to_string(),
                    "netstat -tuln (check listening ports)".to_string(),
                ],
                confidence: 0.85,
            })
        } else if output.contains("timeout") || output.contains("timed out") {
            Some(ErrorPattern {
                error_type: "network_timeout".to_string(),
                pattern_name: "Network Timeout".to_string(),
                suggestion: "Network operation timed out.\nCommon causes:\n1. Slow or unreliable network\n2. Server is too slow to respond\n3. Packet loss\n4. Firewall blocking traffic\n5. DNS resolution issues\n\nTry:\n1. Check network connectivity\n2. Try again later\n3. Increase timeout if possible\n4. Use a faster network or mirror\n5. Check DNS settings: nslookup <host>".to_string(),
                alternative_commands: vec![
                    "ping -c 5 <host>".to_string(),
                    "traceroute <host>".to_string(),
                    "nslookup <host>".to_string(),
                    "curl --max-time 30 <url>".to_string(),
                ],
                confidence: 0.80,
            })
        } else {
            Some(ErrorPattern {
                error_type: "network_generic_error".to_string(),
                pattern_name: "Network Error".to_string(),
                suggestion: "A network error occurred.\nTry:\n1. Check internet connection\n2. Verify hostname/URL is correct\n3. Check firewall settings\n4. Try again later\n5. Check DNS resolution".to_string(),
                alternative_commands: vec![
                    "ping google.com".to_string(),
                    "curl https://www.google.com".to_string(),
                    "cat /etc/resolv.conf".to_string(),
                ],
                confidence: 0.70,
            })
        }
    }

    /// Detect file not found errors
    fn detect_file_not_found(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("No such file or directory")
            && !output.contains("cannot find")
            && !output.contains("ENOENT")
            && !output.contains("file not found")
        {
            return None;
        }

        Some(ErrorPattern {
            error_type: "file_not_found".to_string(),
            pattern_name: "File Not Found".to_string(),
            suggestion: "A required file or directory doesn't exist.\nCommon causes:\n1. File was deleted or moved\n2. Typo in the file path\n3. Working directory is different than expected\n4. File hasn't been created yet\n5. Case sensitivity (Linux/Unix is case-sensitive)\n\nTry:\n1. Check current directory: pwd\n2. List directory contents: ls -la\n3. Verify file path spelling\n4. Check if file exists in a different location\n5. Create the file if it's missing".to_string(),
            alternative_commands: vec![
                "ls -la (list all files)".to_string(),
                "pwd (check current directory)".to_string(),
                "find . -name \"<filename>\" (search for file)".to_string(),
                "cat <file> (verify file exists)".to_string(),
            ],
            confidence: 0.90,
        })
    }

    /// Detect command not found errors
    fn detect_command_not_found(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("command not found")
            && !output.contains("command not found")
            && !output.contains("is not recognized")
            && !output.contains("No such file or directory")
        {
            return None;
        }

        // Try to extract the command name
        let command_name = output
            .lines()
            .find(|line| line.contains("command not found") || line.contains("not found"))
            .and_then(|line| line.split_whitespace().next().map(|s| s.to_string()))
            .unwrap_or_else(|| "command".to_string());

        Some(ErrorPattern {
            error_type: "command_not_found".to_string(),
            pattern_name: "Command Not Found".to_string(),
            suggestion: format!(
                "The command '{}' is not found or not installed.\n\nCommon causes:\n1. Command is not installed\n2. Command is not in PATH\n3. Typo in command name\n4. Wrong package manager (e.g., using apt on Fedora)\n\nTry:\n1. Check if command is installed: which {}\n2. Install the command using your package manager\n3. Check PATH: echo $PATH\n4. Verify command spelling",
                command_name, command_name
            ),
            alternative_commands: vec![
                format!("which {}", command_name),
                format!("apt install {}", command_name),
                format!("yum install {}", command_name),
                format!("brew install {}", command_name),
                "echo $PATH".to_string(),
            ],
            confidence: 0.95,
        })
    }
}

impl PatternDetector for GenericPatternDetector {
    fn detect_patterns(&self, output: &str) -> Vec<ErrorPattern> {
        let mut patterns = Vec::new();

        // Check each pattern type (order matters - most specific first)
        if let Some(pattern) = self.detect_permission_error(output) {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_disk_space_error(output) {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_network_error(output) {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_file_not_found(output) {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_command_not_found(output) {
            patterns.push(pattern);
        }

        patterns
    }

    fn language(&self) -> &'static str {
        "generic"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_error_detection() {
        let detector = GenericPatternDetector::new();
        let error = "bash: ./script.sh: Permission denied";
        let patterns = detector.detect_patterns(error);
        assert!(!patterns.is_empty());
        assert_eq!(patterns[0].error_type, "permission_error");
    }

    #[test]
    fn test_disk_space_error_detection() {
        let detector = GenericPatternDetector::new();
        let error = "error: failed to write file: No space left on device";
        let patterns = detector.detect_patterns(error);
        assert!(!patterns.is_empty());
        assert_eq!(patterns[0].error_type, "disk_space_error");
    }

    #[test]
    fn test_network_error_detection() {
        let detector = GenericPatternDetector::new();
        let error = "curl: (7) Failed to connect to example.com port 80: Connection refused";
        let patterns = detector.detect_patterns(error);
        assert!(!patterns.is_empty());
        // Should detect a network-related error
        assert!(patterns.iter().any(|p| p.error_type.contains("network")));
    }

    #[test]
    fn test_file_not_found_detection() {
        let detector = GenericPatternDetector::new();
        let error = "Error: No such file or directory (os error 2)";
        let patterns = detector.detect_patterns(error);
        assert!(!patterns.is_empty());
        assert_eq!(patterns[0].error_type, "file_not_found");
    }

    #[test]
    fn test_command_not_found_detection() {
        let detector = GenericPatternDetector::new();
        let error = "bash: cargo: command not found";
        let patterns = detector.detect_patterns(error);
        assert!(!patterns.is_empty());
        assert_eq!(patterns[0].error_type, "command_not_found");
    }

    #[test]
    fn test_no_match() {
        let detector = GenericPatternDetector::new();
        let error = "some random error text that doesn't match any pattern";
        let patterns = detector.detect_patterns(error);
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_multiple_patterns() {
        let detector = GenericPatternDetector::new();
        let error = "Permission denied\nNo space left on device";
        let patterns = detector.detect_patterns(error);
        // Should detect both permission and disk space errors
        assert!(patterns.len() >= 2);
    }
}
