use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Represents a detected error pattern with suggested fixes
#[derive(Clone, Debug)]
pub struct ErrorPattern {
    pub error_type: String,
    pub pattern_name: String,
    pub suggestion: String,
    pub alternative_commands: Vec<String>,
    pub confidence: f32,
}

/// Manages project-specific state stored in {project}/state.md
/// This includes execution history, task context, and other project-specific metadata.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ProjectStateManager {
    pub project_path: String,
}

impl ProjectStateManager {
    /// Creates a new project state manager for the given project path.
    pub fn new(project_path: String) -> Self {
        Self { project_path }
    }

    /// Gets the path to the state.md file for this project.
    fn state_file_path(&self) -> String {
        format!("{}/state.md", self.project_path)
    }

    /// Appends a new entry to the project's state.md file.
    /// Each entry includes a timestamp and the content.
    pub fn append_entry(&self, content: &str) -> Result<(), String> {
        self.append_entry_internal(content, false)
    }

    /// Internal implementation for appending entries.
    fn append_entry_internal(&self, content: &str, is_temporary: bool) -> Result<(), String> {
        let state_path = self.state_file_path();

        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let entry = if is_temporary {
            format!(
                "\n## [{}] [TEMPORARY]\n{}\n",
                timestamp,
                content.trim().replace('\n', "\n  ")
            )
        } else {
            format!(
                "\n## [{}]\n{}\n",
                timestamp,
                content.trim().replace('\n', "\n  ")
            )
        };

        let mut existing_content = if Path::new(&state_path).exists() {
            fs::read_to_string(&state_path).unwrap_or_default()
        } else {
            String::from(
                "# Project State\n\nThis file tracks the execution history and context for this project.\n",
            )
        };

        existing_content.push_str(&entry);

        fs::write(&state_path, existing_content)
            .map_err(|e| format!("Failed to write state.md: {}", e))
    }

    /// Updates the state with command execution.
    pub fn log_command(&self, command: &str, output: &str, success: bool) -> Result<(), String> {
        let status = if success { "✅" } else { "❌" };
        let entry = format!(
            "{} **Command**: `{}`\n```\n{}\n```",
            status,
            command,
            output.chars().take(1000).collect::<String>() // Truncate long output
        );
        self.append_entry(&entry)
    }

    /// Updates the state with a system note.
    pub fn log_note(&self, note: &str) -> Result<(), String> {
        let entry = format!("**Note**: {}", note);
        self.append_entry(&entry)
    }

    /// Reads the entire state.md content.
    pub fn read(&self) -> Result<String, String> {
        let state_path = self.state_file_path();
        if !Path::new(&state_path).exists() {
            return Ok(String::new());
        }
        fs::read_to_string(&state_path).map_err(|e| format!("Failed to read state.md: {}", e))
    }

    /// Checks if state.md exists.
    pub fn exists(&self) -> bool {
        Path::new(&self.state_file_path()).exists()
    }

    /// Initializes state.md if it doesn't exist.
    pub fn initialize(&self) -> Result<(), String> {
        let state_path = self.state_file_path();
        if !Path::new(&state_path).exists() {
            fs::write(
                &state_path,
                "# Project State\n\nThis file tracks the execution history and context for this project.\n",
            )
            .map_err(|e| format!("Failed to create state.md: {}", e))?;
        }
        Ok(())
    }

    /// Clears all content from state.md.
    pub fn clear(&self) -> Result<(), String> {
        let state_path = self.state_file_path();
        fs::write(
            &state_path,
            "# Project State\n\nThis file tracks the execution history and context for this project.\n",
        )
        .map_err(|e| format!("Failed to clear state.md: {}", e))
    }

    /// Gets recent execution history as a formatted summary for the agent.
    /// Returns the last N entries with their outcomes.
    pub fn get_recent_history(&self, count: usize) -> Result<String, String> {
        let content = self.read()?;
        if content.is_empty() {
            return Ok("No execution history yet.".to_string());
        }

        let mut entries = Vec::new();
        let mut current_entry = String::new();
        let mut in_code_block = false;

        for line in content.lines() {
            if line.starts_with("## [") {
                if !current_entry.is_empty() {
                    entries.push(current_entry.trim().to_string());
                }
                current_entry = line.to_string();
            } else if line.starts_with("```") {
                in_code_block = !in_code_block;
                current_entry.push_str("\n  ");
                current_entry.push_str(line);
            } else {
                if in_code_block || line.trim().is_empty() {
                    current_entry.push_str("\n  ");
                } else {
                    current_entry.push_str("\n  ");
                }
                current_entry.push_str(line);
            }
        }

        if !current_entry.is_empty() {
            entries.push(current_entry.trim().to_string());
        }

        // Take the last N entries
        let recent: Vec<_> = entries.into_iter().rev().take(count).collect();
        let recent: Vec<_> = recent.into_iter().rev().collect();

        if recent.is_empty() {
            Ok("No execution history yet.".to_string())
        } else {
            Ok(format!(
                "Recent Execution History (last {} entries):\n{}",
                count,
                recent.join("\n\n")
            ))
        }
    }

    /// Gets failed commands from state.md for error learning.
    pub fn get_failed_commands(&self) -> Result<Vec<(String, String)>, String> {
        let content = self.read()?;
        let mut failures = Vec::new();
        let mut current_command = String::new();
        let mut current_output = String::new();
        let mut is_failure = false;
        let mut in_output = false;

        for line in content.lines() {
            if line.contains("❌ **Command**") {
                is_failure = true;
                // Extract command between backticks
                if let Some(start) = line.find("`") {
                    if let Some(end) = line.rfind("`") {
                        current_command = line[start + 1..end].to_string();
                    }
                }
            } else if is_failure && line.trim() == "```" {
                in_output = !in_output;
            } else if in_output && is_failure {
                current_output.push_str(line);
                current_output.push('\n');
            } else if is_failure && line.trim().is_empty() && !in_output {
                if !current_command.is_empty() {
                    failures.push((current_command.clone(), current_output.clone()));
                }
                current_command.clear();
                current_output.clear();
                is_failure = false;
            }
        }

        // Don't forget the last entry
        if is_failure && !current_command.is_empty() {
            failures.push((current_command, current_output));
        }

        Ok(failures)
    }

    /// Detects error patterns in failed commands and suggests fixes.
    pub fn detect_error_patterns(&self) -> Result<Vec<ErrorPattern>, String> {
        let failures = self.get_failed_commands()?;
        let mut patterns = Vec::new();

        for (_cmd, output) in failures {
            // Rust-specific patterns
            if output.contains("error[E0432]") || output.contains("unresolved import") {
                if output.contains("trait") && output.contains("does not exist in the root") {
                    patterns.push(ErrorPattern {
                        error_type: "rust_trait_version_error".to_string(),
                        pattern_name: "Trait Not Found in Crate Version".to_string(),
                        suggestion: "This trait doesn't exist in the current version of the crate.
Try:
1. Removing the trait import (methods may be directly available)
2. Checking the crate's documentation for version-specific API changes
3. Updating the crate to a version that includes this trait
4. Using the trait methods directly on the type without importing"
                            .to_string(),
                        alternative_commands: vec![
                            "Check crate documentation: cargo doc --open".to_string(),
                            "Update crate: cargo add crate_name --vers latest".to_string(),
                            "Search for trait usage examples".to_string(),
                        ],
                        confidence: 0.9,
                    });
                } else if output.contains("use ") && output.contains("::") {
                    patterns.push(ErrorPattern {
                        error_type: "rust_missing_dependency".to_string(),
                        pattern_name: "Missing Crate Dependency".to_string(),
                        suggestion: "This crate is not in your dependencies.
Add it using: cargo add <crate_name>"
                            .to_string(),
                        alternative_commands: vec![
                            format!("cargo add {}", extract_crate_name(&output)),
                            "cargo search <crate_name> to find the right version".to_string(),
                        ],
                        confidence: 0.85,
                    });
                }
            }

            if output.contains("error[E0277]") && output.contains("trait bound") {
                patterns.push(ErrorPattern {
                    error_type: "rust_trait_bound_error".to_string(),
                    pattern_name: "Trait Bound Not Satisfied".to_string(),
                    suggestion: "A type doesn't implement a required trait.
Try:
1. Adding the trait derive to the type: #[derive(TraitName)]
2. Implementing the trait manually for your type
3. Using a different type that satisfies the trait bound
4. Adding the trait as a supertrait if defining your own trait"
                        .to_string(),
                    alternative_commands: vec![
                        "Check type definitions and trait implementations".to_string(),
                        "cargo doc --open to view trait requirements".to_string(),
                    ],
                    confidence: 0.8,
                });
            }

            if output.contains("error[E0308]") && output.contains("mismatched types") {
                patterns.push(ErrorPattern {
                    error_type: "rust_type_mismatch".to_string(),
                    pattern_name: "Type Mismatch".to_string(),
                    suggestion: "Types don't match. Common solutions:
1. Convert between types using .into(), .to_string(), as u32, etc.
2. Check both sides of the assignment/function call
3. Use type annotations to clarify expected types
4. Ensure generic type parameters match"
                        .to_string(),
                    alternative_commands: vec![
                        "Add type annotations to clarify expected types".to_string(),
                        "Use .into() for type conversions".to_string(),
                    ],
                    confidence: 0.75,
                });
            }

            // Node.js-specific patterns
            if output.contains("Cannot find module") || output.contains("ERR_MODULE_NOT_FOUND") {
                patterns.push(ErrorPattern {
                    error_type: "node_missing_module".to_string(),
                    pattern_name: "Missing Node.js Module".to_string(),
                    suggestion: "A required module is not installed.
Install it using: npm install <module_name>"
                        .to_string(),
                    alternative_commands: vec![
                        format!("npm install {}", extract_module_name(&output)),
                        "npm install to install all dependencies".to_string(),
                    ],
                    confidence: 0.9,
                });
            }

            if output.contains("TypeError") && output.contains("is not a function") {
                patterns.push(ErrorPattern {
                    error_type: "node_type_error".to_string(),
                    pattern_name: "Type Error - Not a Function".to_string(),
                    suggestion: "Trying to call something that isn't a function.
Check:
1. The object/function exists and is imported correctly
2. The spelling matches the export
3. You're calling it with correct syntax (object.method() vs object.method)
4. The module is properly initialized"
                        .to_string(),
                    alternative_commands: vec![
                        "Check imports and exports".to_string(),
                        "Add console.log() to debug the object".to_string(),
                    ],
                    confidence: 0.7,
                });
            }

            // Go-specific patterns
            if output.contains("cannot find package") || output.contains("module not found") {
                patterns.push(ErrorPattern {
                    error_type: "go_missing_module".to_string(),
                    pattern_name: "Missing Go Module".to_string(),
                    suggestion: "A Go module is not found.
Try:
1. go mod tidy to clean up dependencies
2. go get <module> to fetch the module
3. Check your GOPATH and module path
4. Ensure go.mod is properly configured"
                        .to_string(),
                    alternative_commands: vec![
                        "go mod tidy".to_string(),
                        "go get <missing_module>".to_string(),
                    ],
                    confidence: 0.85,
                });
            }

            if output.contains("undefined:") && output.contains("declared and not used") {
                patterns.push(ErrorPattern {
                    error_type: "go_unused_variable".to_string(),
                    pattern_name: "Unused Variable".to_string(),
                    suggestion: "A variable is declared but never used.
Fix:
1. Use the variable in your code
2. Remove the unused declaration
3. Use _ to explicitly ignore the value: _ = variable"
                        .to_string(),
                    alternative_commands: vec!["Remove unused variables or use _".to_string()],
                    confidence: 0.95,
                });
            }

            // Generic patterns
            if output.contains("Permission denied") || output.contains("EACCES") {
                patterns.push(ErrorPattern {
                    error_type: "permission_error".to_string(),
                    pattern_name: "Permission Denied".to_string(),
                    suggestion: "You don't have permission to access this resource.
Try:
1. Running with appropriate permissions (carefully!)
2. Checking file/directory ownership
3. Adjusting file permissions with chmod
4. Using a different directory or file path"
                        .to_string(),
                    alternative_commands: vec![
                        "sudo <command> (use with caution!)".to_string(),
                        "chmod +x <file> to make executable".to_string(),
                    ],
                    confidence: 0.9,
                });
            }

            if output.contains("No space left on device") || output.contains("ENOSPC") {
                patterns.push(ErrorPattern {
                    error_type: "disk_space_error".to_string(),
                    pattern_name: "Disk Full".to_string(),
                    suggestion: "Your disk is out of space.
Try:
1. Cleaning build artifacts: cargo clean, npm cache clean, etc.
2. Removing temporary files
3. Cleaning package caches
4. Freeing up disk space"
                        .to_string(),
                    alternative_commands: vec![
                        "cargo clean (for Rust projects)".to_string(),
                        "npm cache clean --force (for Node projects)".to_string(),
                        "df -h to check disk space".to_string(),
                    ],
                    confidence: 0.95,
                });
            }
        }

        // Deduplicate patterns by error_type
        let mut unique_patterns = Vec::new();
        let mut seen_types = std::collections::HashSet::new();
        for pattern in patterns {
            if seen_types.insert(pattern.error_type.clone()) {
                unique_patterns.push(pattern);
            }
        }

        Ok(unique_patterns)
    }

    /// Formats error patterns for display to the agent.
    pub fn format_error_patterns(&self, patterns: &[ErrorPattern]) -> String {
        if patterns.is_empty() {
            return String::new();
        }

        let mut output = String::from("## 🔍 Detected Error Patterns\n\n");

        for (i, pattern) in patterns.iter().enumerate() {
            output.push_str(&format!(
                "### Pattern {}: {} (confidence: {:.0}%)\n\
                **Type**: `{}`\n\
                **Suggestion**: {}\n\
                **Alternative Approaches**:\n",
                i + 1,
                pattern.pattern_name,
                pattern.confidence * 100.0,
                pattern.error_type,
                pattern.suggestion
            ));

            for (j, cmd) in pattern.alternative_commands.iter().enumerate() {
                output.push_str(&format!("{}. `{}`\n", j + 1, cmd));
            }
            output.push('\n');
        }

        output.push_str("💡 **Recommendation**: Learn from these patterns and try the suggested alternatives!\n");
        output
    }
}

/// Helper: Extract crate name from Rust error message
fn extract_crate_name(error: &str) -> String {
    // Look for patterns like "use crate::name" or "extern crate name"
    if let Some(start) = error.find("use ") {
        let after_use = &error[start + 4..];
        if let Some(end) = after_use.find(';') {
            let import_path = after_use[..end].trim();
            return import_path
                .split("::")
                .next()
                .unwrap_or("crate_name")
                .to_string();
        }
    }
    "crate_name".to_string()
}

/// Helper: Extract module name from Node.js error message
fn extract_module_name(error: &str) -> String {
    // Look for patterns like "Cannot find module 'module-name'"
    if let Some(start) = error.find("'") {
        let after_quote = &error[start + 1..];
        if let Some(end) = after_quote.find("'") {
            return after_quote[..end].to_string();
        }
    }
    "module_name".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_project_state_manager() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path().to_str().unwrap().to_string();

        let manager = ProjectStateManager::new(project_path.clone());

        // Test initialization
        manager.initialize().unwrap();
        assert!(manager.exists());

        // Test appending entries
        manager.log_note("Test note").unwrap();
        manager
            .log_command("ls", "file1.txt\nfile2.txt", true)
            .unwrap();

        let content = manager.read().unwrap();
        assert!(content.contains("Test note"));
        assert!(content.contains("ls"));
        assert!(content.contains("file1.txt"));

        // Test clearing
        manager.clear().unwrap();
        let cleared_content = manager.read().unwrap();
        assert!(!cleared_content.contains("Test note"));
    }
}
