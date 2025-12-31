//! Modular error pattern system organized by language/tool.
//!
//! This module provides a flexible, extensible pattern detection system where:
//! - Patterns are organized by language/tool (Rust, Node, Go, Python, Flutter, etc.)
//! - Only relevant patterns are loaded based on project type
//! - Easy to add new patterns without modifying core logic
//! - Patterns can be loaded from external config files in the future

pub mod flutter;
pub mod generic;
pub mod go;
pub mod nodejs;
pub mod python;
pub mod rust;

use serde::{Deserialize, Serialize};

/// Represents a detected error pattern with suggested fixes
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErrorPattern {
    pub error_type: String,
    pub pattern_name: String,
    pub suggestion: String,
    pub alternative_commands: Vec<String>,
    pub confidence: f32,
}

/// Trait for pattern modules that can detect errors in their language/tool
pub trait PatternDetector {
    /// Detect error patterns in the given output
    /// Returns patterns found (empty vector if none)
    fn detect_patterns(&self, output: &str) -> Vec<ErrorPattern>;

    /// Get the language/tool name this detector handles
    fn language(&self) -> &'static str;
}

/// Registry of all available pattern detectors
pub struct PatternRegistry {
    detectors: Vec<Box<dyn PatternDetector>>,
}

impl PatternRegistry {
    /// Create a new pattern registry with all available detectors
    pub fn new() -> Self {
        Self {
            detectors: vec![
                Box::new(rust::RustPatternDetector::new()),
                Box::new(nodejs::NodeJsPatternDetector::new()),
                Box::new(go::GoPatternDetector::new()),
                Box::new(python::PythonPatternDetector::new()),
                Box::new(flutter::FlutterPatternDetector::new()),
                Box::new(generic::GenericPatternDetector::new()),
            ],
        }
    }

    /// Detect patterns in output using all registered detectors
    pub fn detect_all(&self, output: &str) -> Vec<ErrorPattern> {
        let mut all_patterns = Vec::new();

        for detector in &self.detectors {
            let patterns = detector.detect_patterns(output);
            all_patterns.extend(patterns);
        }

        // Deduplicate by error_type while keeping highest confidence
        self.match_patterns(all_patterns)
    }

    /// Detect patterns only for specific language (performance optimization)
    pub fn detect_for_language(&self, output: &str, language: &str) -> Vec<ErrorPattern> {
        let mut patterns = Vec::new();

        for detector in &self.detectors {
            if detector.language() == language {
                patterns.extend(detector.detect_patterns(output));
            }
        }

        // Also always check generic patterns
        patterns.extend(
            self.detectors
                .iter()
                .find(|d| d.language() == "generic")
                .unwrap()
                .detect_patterns(output),
        );

        self.match_patterns(patterns)
    }

    /// Deduplicate patterns, keeping highest confidence for each error_type
    pub fn match_patterns(&self, patterns: Vec<ErrorPattern>) -> Vec<ErrorPattern> {
        use std::collections::HashMap;

        let mut best_patterns: HashMap<String, ErrorPattern> = HashMap::new();

        for pattern in patterns {
            best_patterns
                .entry(pattern.error_type.clone())
                .and_modify(|existing| {
                    if pattern.confidence > existing.confidence {
                        *existing = pattern.clone();
                    }
                })
                .or_insert(pattern);
        }

        let mut result: Vec<_> = best_patterns.into_values().collect();
        result.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        result
    }
}

impl Default for PatternRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_detection() {
        let registry = PatternRegistry::new();

        // Test Rust pattern
        let rust_error = "error[E0432]: unresolved import `sysinfo::CpuExt`";
        let patterns = registry.detect_all(rust_error);
        assert!(!patterns.is_empty());
        assert!(patterns[0].error_type.contains("rust"));
    }

    #[test]
    fn test_language_specific_detection() {
        let registry = PatternRegistry::new();

        // Should only detect Rust patterns, not Node patterns
        let rust_error = "error[E0432]: unresolved import";
        let patterns = registry.detect_for_language(rust_error, "rust");
        assert!(!patterns.is_empty());
    }

    #[test]
    fn test_deduplication() {
        let registry = PatternRegistry::new();

        // Multiple detectors might match the same error
        let error = "error[E0432]: unresolved import";
        let patterns = registry.detect_all(error);

        // Check no duplicate error_types
        let error_types: Vec<_> = patterns.iter().map(|p| &p.error_type).collect();
        let unique: std::collections::HashSet<_> = error_types.into_iter().collect();
        assert_eq!(unique.len(), patterns.len());
    }
}
