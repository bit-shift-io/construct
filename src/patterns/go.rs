//! Go-specific error pattern detector
//!
//! Handles detection and suggestion generation for Go compilation errors,
//! go module errors, and common Go development issues.

use super::{ErrorPattern, PatternDetector};

/// Go error pattern detector
pub struct GoPatternDetector;

impl GoPatternDetector {
    pub fn new() -> Self {
        Self
    }

    /// Detect missing module/package errors
    fn detect_missing_module(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("cannot find package") && !output.contains("module not found") {
            return None;
        }

        // Extract package name from error
        let package_name = self.extract_package_name(output);

        Some(ErrorPattern {
            error_type: "go_missing_module".to_string(),
            pattern_name: "Missing Go Module".to_string(),
            suggestion: format!(
                "A required Go module/package is not found.\nTry:\n1. Run go mod tidy to clean up dependencies\n2. Fetch the module: go get {}\n3. Check if the module path is correct\n4. Verify your GOPATH and module configuration\n5. Ensure go.mod is properly configured",
                package_name
            ),
            alternative_commands: vec![
                "go mod tidy".to_string(),
                format!("go get {}", package_name),
                "cat go.mod".to_string(),
                "go env GOPATH".to_string(),
            ],
            confidence: 0.85,
        })
    }

    /// Detect unused variable errors
    fn detect_unused_variable(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("declared and not used") {
            return None;
        }

        Some(ErrorPattern {
            error_type: "go_unused_variable".to_string(),
            pattern_name: "Unused Variable".to_string(),
            suggestion: "A variable is declared but never used in your Go code.\nGo compiler strictly enforces this to prevent dead code.\n\nFix options:\n1. Use the variable in your code\n2. Remove the unused declaration\n3. Use the blank identifier _ to explicitly ignore the value\n4. For loop variables, use _ if you don't need the value\n\nExamples:\n- _ = variable  // Explicitly ignore\n- for _, item := range items { ... }  // Ignore index".to_string(),
            alternative_commands: vec![
                "Remove unused variables or use _".to_string(),
                "Use golint to find unused variables".to_string(),
                "Run go vet to catch unused variables".to_string(),
            ],
            confidence: 0.95,
        })
    }

    /// Detect type mismatch errors
    fn detect_type_mismatch(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("cannot use")
            && !output.contains("type")
            && !output.contains("in assignment")
        {
            return None;
        }

        Some(ErrorPattern {
            error_type: "go_type_mismatch".to_string(),
            pattern_name: "Go Type Mismatch".to_string(),
            suggestion: "Type mismatch error in Go - you're trying to use a value of one type where another type is expected.\nCommon causes:\n1. Assigning incompatible types\n2. Wrong function return type\n3. Incorrect type conversion\n4. Interface vs concrete type mismatch\n\nTry:\n1. Use type conversion: value.Type() or Type(value)\n2. Check function signatures\n3. Verify both sides of assignment\n4. Ensure interface is implemented correctly".to_string(),
            alternative_commands: vec![
                "Check type definitions and conversions".to_string(),
                "Run go vet for type checking".to_string(),
                "Verify interface implementations".to_string(),
            ],
            confidence: 0.80,
        })
    }

    /// Detect import cycle errors
    fn detect_import_cycle(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("import cycle") && !output.contains("cycle not allowed") {
            return None;
        }

        Some(ErrorPattern {
            error_type: "go_import_cycle".to_string(),
            pattern_name: "Import Cycle Detected".to_string(),
            suggestion: "Go detected a circular import dependency in your code.\nPackage A imports B, B imports C, and C imports A (directly or indirectly).\n\nSolutions:\n1. Refactor to break the cycle\n2. Move shared code to a separate package\n3. Use interfaces to decouple dependencies\n4. Extract common functionality to a new package\n5. Consider if you actually need all these imports".to_string(),
            alternative_commands: vec![
                "Analyze import graph: go list -f '{{.ImportPath}} {{join .Deps \" \"}}' all".to_string(),
                "Check package dependencies".to_string(),
                "Refactor to eliminate circular dependencies".to_string(),
            ],
            confidence: 0.90,
        })
    }

    /// Detect undefined type/function errors
    fn detect_undefined(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("undefined:") && !output.contains("not declared by package") {
            return None;
        }

        // Try to extract what's undefined
        let undefined_item = if let Some(pos) = output.find("undefined:") {
            let after = &output[pos + 10..];
            after
                .lines()
                .next()
                .unwrap_or("type or function")
                .trim()
                .to_string()
        } else {
            "type or function".to_string()
        };

        Some(ErrorPattern {
            error_type: "go_undefined".to_string(),
            pattern_name: "Undefined Type or Function".to_string(),
            suggestion: format!(
                "The identifier '{}' is not defined in the current scope.\nCommon causes:\n1. Typo in the name\n2. Not exported from the package (lowercase name)\n3. Not imported yet\n4. Defined in a different package and not referenced correctly\n5. Case sensitivity issue (Go is case-sensitive)",
                undefined_item
            ),
            alternative_commands: vec![
                "Check spelling and case sensitivity".to_string(),
                "Ensure the identifier is exported (starts with capital letter)".to_string(),
                "Verify imports are correct".to_string(),
                "Run go vet to catch undeclared variables".to_string(),
            ],
            confidence: 0.85,
        })
    }

    /// Helper: Extract package name from error message
    fn extract_package_name(&self, error: &str) -> String {
        // Look for patterns like "cannot find package \"github.com/user/repo\""
        if let Some(start) = error.find("\"") {
            let after_quote = &error[start + 1..];
            if let Some(end) = after_quote.find("\"") {
                return after_quote[..end].to_string();
            }
        }

        // Also check for patterns without quotes
        if let Some(start) = error.find("cannot find package ") {
            let after = &error[start + 20..];
            if let Some(end) = after.find('\n') {
                return after[..end].trim().to_string();
            }
        }

        // Common default packages
        if error.contains("github.com") {
            if let Some(start) = error.find("github.com/") {
                let after = &error[start..];
                if let Some(end) = after.find(char::is_whitespace) {
                    return after[..end].to_string();
                }
                return after
                    .lines()
                    .next()
                    .unwrap_or("github.com/user/repo")
                    .to_string();
            }
        }

        "module/path".to_string()
    }
}

impl PatternDetector for GoPatternDetector {
    fn detect_patterns(&self, output: &str) -> Vec<ErrorPattern> {
        let mut patterns = Vec::new();

        // Check each pattern type
        if let Some(pattern) = self.detect_missing_module(output) {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_unused_variable(output) {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_type_mismatch(output) {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_import_cycle(output) {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_undefined(output) {
            patterns.push(pattern);
        }

        patterns
    }

    fn language(&self) -> &'static str {
        "go"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_module_detection() {
        let detector = GoPatternDetector::new();
        let error =
            "build github.com/user/project: cannot find package \"github.com/gin-gonic/gin\"";
        let patterns = detector.detect_patterns(error);
        assert!(!patterns.is_empty());
        assert_eq!(patterns[0].error_type, "go_missing_module");
    }

    #[test]
    fn test_unused_variable_detection() {
        let detector = GoPatternDetector::new();
        let error = "main.go:10:2: x declared and not used";
        let patterns = detector.detect_patterns(error);
        assert!(!patterns.is_empty());
        assert_eq!(patterns[0].error_type, "go_unused_variable");
    }

    #[test]
    fn test_import_cycle_detection() {
        let detector = GoPatternDetector::new();
        let error = "import cycle not allowed\npackage github.com/user/a imports github.com/user/b imports github.com/user/a";
        let patterns = detector.detect_patterns(error);
        assert!(!patterns.is_empty());
        assert_eq!(patterns[0].error_type, "go_import_cycle");
    }

    #[test]
    fn test_no_match() {
        let detector = GoPatternDetector::new();
        let error = "some random error text";
        let patterns = detector.detect_patterns(error);
        assert!(patterns.is_empty());
    }
}
