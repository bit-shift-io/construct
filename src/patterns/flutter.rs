//! Flutter/Dart-specific error pattern detector
//!
//! Handles detection and suggestion generation for Flutter/Dart compilation errors,
//! package dependency errors, FFI issues, and common Flutter development problems.

use super::{ErrorPattern, PatternDetector};

/// Flutter/Dart error pattern detector
pub struct FlutterPatternDetector;

impl FlutterPatternDetector {
    pub fn new() -> Self {
        Self
    }

    /// Detect missing package errors
    fn detect_missing_package(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("depends on") || !output.contains("any which doesn't exist") {
            return None;
        }

        // Extract package name from error
        let package_name = self.extract_package_name(output);

        Some(ErrorPattern {
            error_type: "flutter_missing_package".to_string(),
            pattern_name: "Flutter Package Not Found".to_string(),
            suggestion: format!(
                "The Flutter package '{}' doesn't exist on pub.dev.\nTry:\n1. Check the correct package name on https://pub.dev\n2. Common chart packages: 'fl_chart', 'charts_flutter', 'syncfusion_flutter_charts'\n3. Search with: flutter pub search <keyword>\n4. Check for typos in the package name\n5. Verify the package supports your Flutter version",
                package_name
            ),
            alternative_commands: vec![
                format!(
                    "flutter pub search {}",
                    package_name.split('_').last().unwrap_or("chart")
                ),
                "flutter pub search chart".to_string(),
                "Visit https://pub.dev to search manually".to_string(),
                "flutter --version".to_string(),
            ],
            confidence: 0.95,
        })
    }

    /// Detect web compilation errors (FFI issues)
    fn detect_web_compilation_error(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("Target dart2js failed") && !output.contains("Compilation failed") {
            return None;
        }

        // Check if it's specifically an FFI error
        if output.contains("dart:ffi") || output.contains("FFI") {
            Some(ErrorPattern {
                error_type: "flutter_web_ffi_error".to_string(),
                pattern_name: "Flutter Web FFI Not Supported".to_string(),
                suggestion: "Flutter web build failed because dart:ffi is not available for web/Wasm builds.\n\n**Background**: FFI (Foreign Function Interface) allows Dart to call native code (C, Rust, etc.), but this isn't supported when compiling to WebAssembly.\n\nSolutions:\n1. **Conditional compilation**: Use platform checks to exclude FFI on web\n   ```dart\n   import 'dart:io' if (dart:io.Platform.isAndroid) || (dart:io.Platform.isIOS);\n   ```\n2. **Platform-specific builds**: Build for desktop/mobile instead of web\n   - `flutter build macos`\n   - `flutter build windows`\n   - `flutter build apk`\n3. **Web alternative**: Use pure Dart packages instead of native code\n4. **Conditional imports**: Use export/stub files for web\n\nTry building for desktop platforms where FFI is fully supported.".to_string(),
                alternative_commands: vec![
                    "flutter build macos".to_string(),
                    "flutter build windows".to_string(),
                    "flutter build apk".to_string(),
                    "Add conditional imports: import 'dart:io' if (dart:io.Platform.isAndroid)".to_string(),
                    "Check package compatibility: flutter pub publish --dry-run".to_string(),
                ],
                confidence: 0.90,
            })
        } else {
            Some(ErrorPattern {
                error_type: "flutter_web_compilation_error".to_string(),
                pattern_name: "Flutter Web Compilation Failed".to_string(),
                suggestion: "Flutter web build failed.\nCommon causes:\n1. Platform-specific code not compatible with web\n2. Missing web-specific dependencies\n3. JavaScript/Wasm limitations\n\nTry:\n1. Check if all packages support web platform\n2. Review platform-specific code and add web alternatives\n3. Run with verbose output: flutter build web --verbose\n4. Clean build: flutter clean && flutter pub get".to_string(),
                alternative_commands: vec![
                    "flutter build macos/windows/linux".to_string(),
                    "flutter clean && flutter pub get".to_string(),
                    "flutter build web --verbose".to_string(),
                    "Check package compatibility: flutter pub deps".to_string(),
                ],
                confidence: 0.80,
            })
        }
    }

    /// Detect test failures
    fn detect_test_failure(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("Test failed") && !output.contains("Some tests failed") {
            return None;
        }

        Some(ErrorPattern {
            error_type: "flutter_test_failure".to_string(),
            pattern_name: "Flutter Tests Failed".to_string(),
            suggestion: "Flutter tests are failing.\n\nTry:\n1. Run tests with verbose output: flutter test --verbose\n2. Run individual test file: flutter test test/name_test.dart\n3. Run tests with more details: flutter test --reporter expanded\n4. Check test logs for specific failures\n5. Update test expectations if code changed legitimately\n6. Use observatory for debugging: flutter test --observatory".to_string(),
            alternative_commands: vec![
                "flutter test --verbose".to_string(),
                "flutter test --reporter expanded".to_string(),
                "flutter test test/name_test.dart".to_string(),
                "flutter test --coverage".to_string(),
            ],
            confidence: 0.85,
        })
    }

    /// Detect pub get errors
    fn detect_pub_get_error(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("pub get failed") && !output.contains("version solving failed") {
            return None;
        }

        if output.contains("connection refused") || output.contains("network") {
            Some(ErrorPattern {
                error_type: "flutter_network_error".to_string(),
                pattern_name: "Flutter Package Download Failed".to_string(),
                suggestion: "Failed to download Flutter packages due to network issues.\nTry:\n1. Check internet connection\n2. Verify pub.dev is accessible\n3. Try using a different network\n4. Check if behind a firewall/proxy\n5. Set environment variables if needed:\n   - FLUTTER_STORAGE_BASE_URL\n   - PUB_HOSTED_URL".to_string(),
                alternative_commands: vec![
                    "ping pub.dev".to_string(),
                    "flutter doctor -v".to_string(),
                    "export PUB_HOSTED_URL=https://pub.dev".to_string(),
                ],
                confidence: 0.90,
            })
        } else {
            Some(ErrorPattern {
                error_type: "flutter_dependency_error".to_string(),
                pattern_name: "Flutter Dependency Resolution Failed".to_string(),
                suggestion: "Flutter cannot resolve package dependencies.\nTry:\n1. Clean and reinstall: flutter clean && flutter pub get\n2. Check pubspec.yaml for version conflicts\n3. Update Flutter SDK: flutter upgrade\n4. Remove pubspec.lock and retry\n5. Check for incompatible version constraints".to_string(),
                alternative_commands: vec![
                    "flutter clean && flutter pub get".to_string(),
                    "rm pubspec.lock && flutter pub get".to_string(),
                    "flutter upgrade".to_string(),
                    "cat pubspec.yaml".to_string(),
                ],
                confidence: 0.80,
            })
        }
    }

    /// Detect analyzer issues
    fn detect_analyzer_error(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("error:") && !output.contains("warning:") {
            return None;
        }

        // Check for specific common issues
        if output.contains("The named parameter is not defined") {
            Some(ErrorPattern {
                error_type: "flutter_analyzer_parameter_error".to_string(),
                pattern_name: "Undefined Named Parameter".to_string(),
                suggestion: "A named parameter doesn't exist in the function/widget being called.\nCommon causes:\n1. Typo in parameter name\n2. Using old API (parameter was removed/renamed)\n3. Missing required import\n4. Wrong widget/function\n\nTry:\n1. Check parameter name spelling\n2. Review the function/widget signature\n3. Check Flutter API docs for the widget\n4. Update Flutter if using deprecated API".to_string(),
                alternative_commands: vec![
                    "Check Flutter docs for the widget".to_string(),
                    "flutter analyze".to_string(),
                    "Review function/widget signature".to_string(),
                ],
                confidence: 0.85,
            })
        } else if output.contains("The method isn't defined for the type") {
            Some(ErrorPattern {
                error_type: "flutter_analyzer_method_error".to_string(),
                pattern_name: "Method Not Defined for Type".to_string(),
                suggestion: "Calling a method that doesn't exist on this object type.\nCommon causes:\n1. Wrong object type\n2. Method name typo\n3. Using method from wrong package\n4. Missing import\n\nTry:\n1. Verify object type at runtime\n2. Check method name spelling\n3. Review available methods in the class\n4. Run flutter analyze for details".to_string(),
                alternative_commands: vec![
                    "flutter analyze".to_string(),
                    "Check available methods in the class".to_string(),
                    "Verify object types".to_string(),
                ],
                confidence: 0.80,
            })
        } else {
            Some(ErrorPattern {
                error_type: "flutter_analyzer_error".to_string(),
                pattern_name: "Flutter Analyzer Error/Warning".to_string(),
                suggestion: "Flutter analyzer found issues in your code.\nTry:\n1. Run flutter analyze for detailed info\n2. Fix specific errors/warnings shown\n3. Check for typos, missing imports, type issues\n4. Use IDE to see inline analyzer results\n5. Review Flutter style guide".to_string(),
                alternative_commands: vec![
                    "flutter analyze".to_string(),
                    "flutter analyze --no-pub".to_string(),
                ],
                confidence: 0.75,
            })
        }
    }

    /// Helper: Extract package name from error message
    fn extract_package_name(&self, error: &str) -> String {
        // Look for patterns like "depends on <package> any which doesn't exist"
        if let Some(start) = error.find("depends on") {
            let after_dep = &error[start + 10..];
            if let Some(end) = after_dep.find(" any which") {
                let package_name = after_dep[..end].trim();
                return package_name.to_string();
            }
        }

        // Common default packages
        if error.contains("chart") {
            return "fl_chart".to_string();
        }
        if error.contains("http") {
            return "http".to_string();
        }
        if error.contains("provider") {
            return "provider".to_string();
        }

        "package_name".to_string()
    }
}

impl PatternDetector for FlutterPatternDetector {
    fn detect_patterns(&self, output: &str) -> Vec<ErrorPattern> {
        let mut patterns = Vec::new();

        // Check each pattern type
        if let Some(pattern) = self.detect_missing_package(output) {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_web_compilation_error(output) {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_test_failure(output) {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_pub_get_error(output) {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_analyzer_error(output) {
            patterns.push(pattern);
        }

        patterns
    }

    fn language(&self) -> &'static str {
        "flutter"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_package_detection() {
        let detector = FlutterPatternDetector::new();
        let error = "Because a4 depends on flutter_flowing_chart any which doesn't exist (could not find package flutter_flowing_chart at https://pub.dev), version solving failed.";
        let patterns = detector.detect_patterns(error);
        assert!(!patterns.is_empty());
        assert_eq!(patterns[0].error_type, "flutter_missing_package");
    }

    #[test]
    fn test_web_ffi_error_detection() {
        let detector = FlutterPatternDetector::new();
        let error = "Target dart2js failed: ProcessException: Process exited abnormally with exit code 1: Error: Compilation failed. dart:ffi is not available on web platform";
        let patterns = detector.detect_patterns(error);
        assert!(!patterns.is_empty());
        assert!(
            patterns
                .iter()
                .any(|p| p.error_type == "flutter_web_ffi_error")
        );
    }

    #[test]
    fn test_test_failure_detection() {
        let detector = FlutterPatternDetector::new();
        let error = "00:05 +0 -1: Some tests failed.";
        let patterns = detector.detect_patterns(error);
        assert!(!patterns.is_empty());
        assert_eq!(patterns[0].error_type, "flutter_test_failure");
    }

    #[test]
    fn test_no_match() {
        let detector = FlutterPatternDetector::new();
        let error = "some random error text";
        let patterns = detector.detect_patterns(error);
        assert!(patterns.is_empty());
    }
}
