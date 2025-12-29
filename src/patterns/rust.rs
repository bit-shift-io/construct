//! Rust-specific error pattern detector
//!
//! Handles detection and suggestion generation for Rust compilation errors,
//! cargo errors, and common Rust development issues.

use super::{ErrorPattern, PatternDetector};

/// Rust error pattern detector
pub struct RustPatternDetector;

impl RustPatternDetector {
    pub fn new() -> Self {
        Self
    }

    /// Detect trait version incompatibility errors
    fn detect_trait_version_error(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("error[E0432]") || !output.contains("unresolved import") {
            return None;
        }

        // Check if it's specifically a trait error
        // Match various error formats: "does not exist", "no `X` in the root", "not found in root"
        // Note: "trait" might be mentioned elsewhere, so check for the specific error patterns
        let has_trait_syntax = output.contains("`")
            && (output.contains(" in the root") || output.contains("does not exist"));
        let has_explicit_trait = output.contains("trait") && output.contains("does not exist");

        if !has_trait_syntax && !has_explicit_trait {
            return None;
        }

        // Extract crate name if possible
        let crate_name = self.extract_crate_name(output);

        Some(ErrorPattern {
            error_type: "rust_trait_version_error".to_string(),
            pattern_name: "Trait Not Found in Crate Version".to_string(),
            suggestion: format!(
                "The trait '{}' doesn't exist in the current version of the crate.
Try:
1. Removing the trait import - methods may be directly available
2. Checking the crate's documentation for version-specific API changes: `cargo doc --open`
3. Updating the crate to a version that includes this trait: `cargo add {} --vers latest`
4. Using the trait methods directly on the type without importing",
                crate_name.as_ref().unwrap_or(&"Unknown".to_string()),
                crate_name.as_ref().unwrap_or(&"crate_name".to_string())
            ),
            alternative_commands: vec![
                "cargo doc --open".to_string(),
                format!(
                    "cargo add {} --vers latest",
                    crate_name.as_ref().unwrap_or(&"crate_name".to_string())
                ),
                "Search for trait usage examples in the crate's documentation".to_string(),
            ],
            confidence: 0.90,
        })
    }

    /// Detect missing dependency errors
    fn detect_missing_dependency(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("error[E0432]") || !output.contains("unresolved import") {
            return None;
        }

        // Skip if it's a trait error (handled separately)
        // Trait errors have E0432 + unresolved import + trait keyword
        if output.contains("trait")
            || (output.contains("unresolved import")
                && (output.contains("no ") && output.contains(" in the root")
                    || output.contains("not found in the root")))
        {
            return None;
        }

        let crate_name = self.extract_crate_name(output)?;

        Some(ErrorPattern {
            error_type: "rust_missing_dependency".to_string(),
            pattern_name: "Missing Crate Dependency".to_string(),
            suggestion: format!(
                "The crate '{}' is not in your Cargo.toml dependencies.
Add it using: `cargo add {}`",
                crate_name, crate_name
            ),
            alternative_commands: vec![
                format!("cargo add {}", crate_name),
                format!("cargo search {} --limit 5", crate_name),
                "Check crates.io for the correct package name".to_string(),
            ],
            confidence: 0.85,
        })
    }

    /// Detect trait bound errors
    fn detect_trait_bound_error(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("error[E0277]") || !output.contains("trait bound") {
            return None;
        }

        Some(ErrorPattern {
            error_type: "rust_trait_bound_error".to_string(),
            pattern_name: "Trait Bound Not Satisfied".to_string(),
            suggestion: "A type doesn't implement a required trait.
Try:
1. Adding the trait derive to the type: `#[derive(TraitName)]`
2. Implementing the trait manually for your type: `impl TraitName for MyType`
3. Using a different type that already satisfies the trait bound
4. Adding the trait as a supertrait if defining your own trait: `trait MyTrait: OtherTrait`
5. Using generic type constraints properly"
                .to_string(),
            alternative_commands: vec![
                "Check type definitions and implement required traits".to_string(),
                "Run `cargo doc --open` to view trait requirements".to_string(),
                "Check if you need to add trait bounds to generic types".to_string(),
            ],
            confidence: 0.80,
        })
    }

    /// Detect type mismatch errors
    fn detect_type_mismatch(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("error[E0308]") || !output.contains("mismatched types") {
            return None;
        }

        Some(ErrorPattern {
            error_type: "rust_type_mismatch".to_string(),
            pattern_name: "Type Mismatch".to_string(),
            suggestion: "Types don't match in an assignment, function call, or expression.
Common solutions:
1. Convert between types using `.into()`, `.to_string()`, `as u32`, etc.
2. Check both sides of the assignment or function call
3. Add type annotations to clarify expected types: `let x: Type = ...`
4. Ensure generic type parameters match
5. Check if you need to dereference: `*variable` or `&variable`"
                .to_string(),
            alternative_commands: vec![
                "Add explicit type annotations to clarify expected types".to_string(),
                "Use `.into()` for type conversions".to_string(),
                "Run `cargo fix --edition-idioms` to apply common fixes".to_string(),
            ],
            confidence: 0.75,
        })
    }

    /// Helper: Extract crate name from error message
    fn extract_crate_name(&self, error: &str) -> Option<String> {
        // Look for patterns like "use crate::name;" or "extern crate name"
        if let Some(start) = error.find("use ") {
            let after_use = &error[start + 4..];
            if let Some(end) = after_use.find(';') {
                let import_path = after_use[..end].trim();
                let crate_name = import_path.split("::").next()?;
                return Some(crate_name.to_string());
            }
        }

        // Also check for "extern crate" syntax
        if let Some(start) = error.find("extern crate ") {
            let after_extern = &error[start + 12..];
            if let Some(end) = after_extern.find(';') {
                return Some(after_extern[..end].trim().to_string());
            }
        }

        Some("unknown".to_string())
    }
}

impl PatternDetector for RustPatternDetector {
    fn detect_patterns(&self, output: &str) -> Vec<ErrorPattern> {
        let mut patterns = Vec::new();

        // Check each pattern type (order matters - more specific first)
        // Note: Multiple patterns may match; deduplication happens at registry level
        if let Some(pattern) = self.detect_trait_version_error(output) {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_missing_dependency(output) {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_trait_bound_error(output) {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_type_mismatch(output) {
            patterns.push(pattern);
        }

        patterns
    }

    fn language(&self) -> &'static str {
        "rust"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trait_version_detection() {
        let detector = RustPatternDetector::new();
        let error = "error[E0432]: unresolved import `sysinfo::CpuExt`
  --> src/main.rs:3:23
   |
3  | use sysinfo::{System, CpuExt};
   |                       ^^^^^^ no `CpuExt` in the root";

        let patterns = detector.detect_patterns(error);
        assert!(!patterns.is_empty());
        // Should detect trait version error (higher priority than missing dependency)
        assert!(
            patterns
                .iter()
                .any(|p| p.error_type == "rust_trait_version_error")
        );
    }

    #[test]
    fn test_missing_dependency_detection() {
        let detector = RustPatternDetector::new();
        let error = "error[E0432]: unresolved import `serde`
   --> src/main.rs:1:5
    |
1  | use serde;
   |     ^^^^ no `serde` in the root";

        let patterns = detector.detect_patterns(error);
        assert!(!patterns.is_empty());
        // Should detect missing dependency (trait patterns don't match this error format)
        assert!(
            patterns
                .iter()
                .any(|p| p.error_type == "rust_missing_dependency")
        );
    }

    #[test]
    fn test_type_mismatch_detection() {
        let detector = RustPatternDetector::new();
        let error = "error[E0308]: mismatched types
  --> src/main.rs:5:18
   |
5  |     let x: i32 = \"hello\";";

        let patterns = detector.detect_patterns(error);
        assert!(!patterns.is_empty());
        assert_eq!(patterns[0].error_type, "rust_type_mismatch");
    }

    #[test]
    fn test_no_match() {
        let detector = RustPatternDetector::new();
        let error = "some random error text";

        let patterns = detector.detect_patterns(error);
        assert!(patterns.is_empty());
        assert_eq!(patterns.len(), 0);
    }
}
