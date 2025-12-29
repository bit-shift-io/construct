//! Node.js-specific error pattern detector
//!
//! Handles detection and suggestion generation for Node.js/npm errors,
//! JavaScript/TypeScript compilation errors, and common Node.js development issues.

use super::{ErrorPattern, PatternDetector};

/// Node.js error pattern detector
pub struct NodeJsPatternDetector;

impl NodeJsPatternDetector {
    pub fn new() -> Self {
        Self
    }

    /// Detect missing module/package errors
    fn detect_missing_module(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("Cannot find module") && !output.contains("ERR_MODULE_NOT_FOUND") {
            return None;
        }

        // Extract module name from error
        let module_name = self.extract_module_name(output);

        Some(ErrorPattern {
            error_type: "node_missing_module".to_string(),
            pattern_name: "Missing Node.js Module".to_string(),
            suggestion: format!(
                "The required module '{}' is not installed or not in your dependencies.\nTry:\n1. Install the module: npm install {}\n2. Add to package.json if missing\n3. Check for typos in the import statement\n4. Verify the module exists on https://www.npmjs.com/",
                module_name, module_name
            ),
            alternative_commands: vec![
                format!("npm install {}", module_name),
                "npm install".to_string(),
                format!("npm search {} --verbose", module_name),
                "Check package.json for missing dependencies".to_string(),
            ],
            confidence: 0.90,
        })
    }

    /// Detect type errors
    fn detect_type_error(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("TypeError") || !output.contains("is not a function") {
            return None;
        }

        Some(ErrorPattern {
            error_type: "node_type_error".to_string(),
            pattern_name: "Type Error - Not a Function".to_string(),
            suggestion: "Trying to call something that isn't a function.\nCommon causes:\n1. Object/function doesn't exist in the module\n2. Spelling mistake in the function name\n3. Incorrect import syntax (default vs named)\n4. Module not properly initialized\n5. Trying to call a non-function property\n\nCheck:\n- The module exports what you expect\n- The import matches the export (require vs import)\n- The object is initialized before use".to_string(),
            alternative_commands: vec![
                "Add console.log() to debug the object".to_string(),
                "Check module.exports in the required file".to_string(),
                "Verify import/export syntax matches".to_string(),
                "Run with --trace-warnings for more details".to_string(),
            ],
            confidence: 0.70,
        })
    }

    /// Detect syntax errors
    fn detect_syntax_error(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("SyntaxError") {
            return None;
        }

        if output.contains("Unexpected token") {
            Some(ErrorPattern {
                error_type: "node_syntax_error".to_string(),
                pattern_name: "JavaScript Syntax Error".to_string(),
                suggestion: "JavaScript syntax error detected.\nCommon causes:\n1. Missing brackets, parentheses, or quotes\n2. Using ES6+ features without proper mode/babel\n3. Copy-paste errors with smart quotes\n4. JSON syntax errors in require()\n\nTry:\n1. Check for matching brackets/parentheses\n2. Verify quotes are straight quotes, not smart quotes\n3. Ensure file is in correct mode (CommonJS vs ES modules)\n4. Use a linter: eslint to catch these early".to_string(),
                alternative_commands: vec![
                    "Run eslint to find syntax errors".to_string(),
                    "Check for matching brackets/quotes".to_string(),
                    "Verify CommonJS vs ES module syntax".to_string(),
                ],
                confidence: 0.85,
            })
        } else {
            Some(ErrorPattern {
                error_type: "node_syntax_error".to_string(),
                pattern_name: "JavaScript Syntax Error".to_string(),
                suggestion: "Syntax error in JavaScript/TypeScript code.\nCheck:\n1. Line numbers in error message\n2. Missing semicolons, brackets, quotes\n3. Reserved words used as variables\n4. Template literal syntax".to_string(),
                alternative_commands: vec![
                    "Run eslint or tsc for detailed errors".to_string(),
                    "Check the specific line mentioned in error".to_string(),
                ],
                confidence: 0.80,
            })
        }
    }

    /// Detect npm dependency errors
    fn detect_dependency_error(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("npm ERR!")
            && !output.contains("ENOAUTO")
            && !output.contains("ERESOLVE")
        {
            return None;
        }

        if output.contains("ERESOLVE") {
            Some(ErrorPattern {
                error_type: "npm_dependency_conflict".to_string(),
                pattern_name: "NPM Dependency Conflict".to_string(),
                suggestion: "NPM cannot resolve dependency versions (ERESOLVE error).\nTry:\n1. Use --legacy-peer-deps flag: npm install --legacy-peer-deps\n2. Update package.json with compatible versions\n3. Clear npm cache: npm cache clean --force\n4. Delete node_modules and package-lock.json, reinstall\n5. Use npm ls to find conflicting dependencies".to_string(),
                alternative_commands: vec![
                    "npm install --legacy-peer-deps".to_string(),
                    "npm cache clean --force".to_string(),
                    "rm -rf node_modules package-lock.json && npm install".to_string(),
                    "npm ls <package> to find conflicts".to_string(),
                ],
                confidence: 0.90,
            })
        } else if output.contains("ENOENT") || output.contains("missing script") {
            Some(ErrorPattern {
                error_type: "npm_missing_script".to_string(),
                pattern_name: "NPM Script Not Found".to_string(),
                suggestion: "The specified npm script doesn't exist in package.json.\nTry:\n1. Check package.json scripts section\n2. Use npm run to see available scripts\n3. Add the script to package.json if needed\n4. Verify script name spelling".to_string(),
                alternative_commands: vec![
                    "npm run".to_string(),
                    "cat package.json | grep -A 10 scripts".to_string(),
                ],
                confidence: 0.95,
            })
        } else {
            Some(ErrorPattern {
                error_type: "npm_generic_error".to_string(),
                pattern_name: "NPM Error".to_string(),
                suggestion: "NPM encountered an error.\nCheck:\n1. Error message details\n2. package.json syntax\n3. Network connectivity\n4. npm version compatibility".to_string(),
                alternative_commands: vec![
                    "npm --version && node --version".to_string(),
                    "cat package.json".to_string(),
                ],
                confidence: 0.70,
            })
        }
    }

    /// Helper: Extract module name from error message
    fn extract_module_name(&self, error: &str) -> String {
        // Look for patterns like "Cannot find module 'module-name'"
        if let Some(start) = error.find("'") {
            let after_quote = &error[start + 1..];
            if let Some(end) = after_quote.find("'") {
                return after_quote[..end].to_string();
            }
        }

        // Also check for double quotes
        if let Some(start) = error.find("\"") {
            let after_quote = &error[start + 1..];
            if let Some(end) = after_quote.find("\"") {
                return after_quote[..end].to_string();
            }
        }

        // Common default modules
        if error.contains("express") {
            return "express".to_string();
        }
        if error.contains("react") {
            return "react".to_string();
        }
        if error.contains("lodash") {
            return "lodash".to_string();
        }

        "module-name".to_string()
    }
}

impl PatternDetector for NodeJsPatternDetector {
    fn detect_patterns(&self, output: &str) -> Vec<ErrorPattern> {
        let mut patterns = Vec::new();

        // Check each pattern type
        if let Some(pattern) = self.detect_missing_module(output) {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_type_error(output) {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_syntax_error(output) {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_dependency_error(output) {
            patterns.push(pattern);
        }

        patterns
    }

    fn language(&self) -> &'static str {
        "nodejs"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_module_detection() {
        let detector = NodeJsPatternDetector::new();
        let error = "Error: Cannot find module 'express'";
        let patterns = detector.detect_patterns(error);
        assert!(!patterns.is_empty());
        assert_eq!(patterns[0].error_type, "node_missing_module");
    }

    #[test]
    fn test_type_error_detection() {
        let detector = NodeJsPatternDetector::new();
        let error = "TypeError: object.method is not a function";
        let patterns = detector.detect_patterns(error);
        assert!(!patterns.is_empty());
        assert_eq!(patterns[0].error_type, "node_type_error");
    }

    #[test]
    fn test_syntax_error_detection() {
        let detector = NodeJsPatternDetector::new();
        let error = "SyntaxError: Unexpected token";
        let patterns = detector.detect_patterns(error);
        assert!(!patterns.is_empty());
        assert!(patterns.iter().any(|p| p.error_type == "node_syntax_error"));
    }

    #[test]
    fn test_dependency_conflict_detection() {
        let detector = NodeJsPatternDetector::new();
        let error = "npm ERR! ERESOLVE unable to resolve dependency tree";
        let patterns = detector.detect_patterns(error);
        assert!(!patterns.is_empty());
        assert_eq!(patterns[0].error_type, "npm_dependency_conflict");
    }

    #[test]
    fn test_no_match() {
        let detector = NodeJsPatternDetector::new();
        let error = "some random error text";
        let patterns = detector.detect_patterns(error);
        assert!(patterns.is_empty());
    }
}
