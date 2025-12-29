//! Python-specific error pattern detector
//!
//! Handles detection and suggestion generation for Python syntax errors,
//! import errors, indentation errors, and common Python development issues.

use super::{ErrorPattern, PatternDetector};

/// Python error pattern detector
pub struct PythonPatternDetector;

impl PythonPatternDetector {
    pub fn new() -> Self {
        Self
    }

    /// Detect import/module not found errors
    fn detect_import_error(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("ModuleNotFoundError") && !output.contains("ImportError") {
            return None;
        }

        // Extract module name from error
        let module_name = self.extract_module_name(output);

        if output.contains("ModuleNotFoundError") {
            Some(ErrorPattern {
                error_type: "python_missing_module".to_string(),
                pattern_name: "Python Module Not Found".to_string(),
                suggestion: format!(
                    "The Python module '{}' is not installed or not in your Python path.\nTry:\n1. Install the module: pip install {}\n2. Check if module name is correct (case-sensitive)\n3. Verify Python path: python -c \"import sys; print(sys.path)\"\n4. Check for virtual environment issues\n5. Verify requirements.txt includes the module",
                    module_name, module_name
                ),
                alternative_commands: vec![
                    format!("pip install {}", module_name),
                    format!("pip install --upgrade {}", module_name),
                    "pip list | grep -i <module>".to_string(),
                    "python -c \"import sys; print(sys.path)\"".to_string(),
                    "Check requirements.txt or pyproject.toml".to_string(),
                ],
                confidence: 0.90,
            })
        } else {
            Some(ErrorPattern {
                error_type: "python_import_error".to_string(),
                pattern_name: "Python Import Error".to_string(),
                suggestion: format!(
                    "Failed to import module '{}'.\nCommon causes:\n1. Module doesn't exist\n2. Circular import\n3. Module has syntax errors\n4. Wrong Python version\n5. Module not in Python path\n\nTry:\n1. Install the module\n2. Check for circular imports\n3. Test import in isolation: python -c \"import {}\"  \n4. Check file for syntax errors",
                    module_name, module_name
                ),
                alternative_commands: vec![
                    format!("python -c \"import {}\"", module_name),
                    "python -m py_compile <module_file>".to_string(),
                    "Check for circular imports".to_string(),
                ],
                confidence: 0.80,
            })
        }
    }

    /// Detect syntax errors
    fn detect_syntax_error(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("SyntaxError") {
            return None;
        }

        if output.contains("invalid syntax") {
            if output.contains("expected") {
                Some(ErrorPattern {
                    error_type: "python_syntax_error".to_string(),
                    pattern_name: "Python Syntax Error".to_string(),
                    suggestion: "Python syntax error - code doesn't conform to Python grammar.\nCommon causes:\n1. Missing colon after if/elif/else/for/while/def/class\n2. Mismatched parentheses/brackets/quotes\n3. Using Python 2 syntax in Python 3 (or vice versa)\n4. Assignment in expression (e.g., if x = 5: instead of if x == 5:)\n5. Missing quotes around strings\n6. Incorrect indentation\n\nTry:\n1. Check the line number in the error message\n2. Verify colons after compound statements\n3. Check matching brackets/parentheses\n4. Use a Python IDE with syntax highlighting\n5. Run with python -m py_compile to check syntax".to_string(),
                    alternative_commands: vec![
                        "python -m py_compile <file>".to_string(),
                        "Check the specific line mentioned in error".to_string(),
                        "Use pylint or flake8 for syntax checking".to_string(),
                    ],
                    confidence: 0.85,
                })
            } else {
                Some(ErrorPattern {
                    error_type: "python_syntax_error".to_string(),
                    pattern_name: "Python Syntax Error".to_string(),
                    suggestion: "Invalid Python syntax detected.\nCheck:\n1. The specific line mentioned in error\n2. For matching quotes, parentheses, brackets\n3. Proper use of operators\n4. Correct keyword spelling\n5. Python version compatibility".to_string(),
                    alternative_commands: vec![
                        "python -m py_compile <file>".to_string(),
                        "Check the line number in error message".to_string(),
                    ],
                    confidence: 0.80,
                })
            }
        } else if output.contains("IndentationError") {
            Some(ErrorPattern {
                error_type: "python_indentation_error".to_string(),
                pattern_name: "Python Indentation Error".to_string(),
                suggestion: "Python indentation error - inconsistent indentation detected.\nPython relies on consistent indentation to define code blocks.\n\nCommon causes:\n1. Mixing tabs and spaces (always use spaces!)\n2. Inconsistent indentation levels\n3. Wrong indentation for nested blocks\n4. Copy-paste code with different indentation\n\nTry:\n1. Use spaces, not tabs (PEP 8 recommends 4 spaces)\n2. Ensure consistent indentation throughout\n3. Re-indent the code block\n4. Use autopep8 or black to fix indentation automatically:\n   - autopep8 --in-place --aggressive <file>\n   - black <file>".to_string(),
                alternative_commands: vec![
                    "autopep8 --in-place --aggressive <file>".to_string(),
                    "black <file>".to_string(),
                    "Convert tabs to spaces in your editor".to_string(),
                    "Check for mixed tabs and spaces: cat -A <file>".to_string(),
                ],
                confidence: 0.95,
            })
        } else {
            None // Let other handlers catch specific syntax errors
        }
    }

    /// Detect type errors
    fn detect_type_error(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("TypeError") {
            return None;
        }

        if output.contains("unsupported operand type") || output.contains("unsupported operand") {
            Some(ErrorPattern {
                error_type: "python_type_error".to_string(),
                pattern_name: "Python Type Error - Unsupported Operand".to_string(),
                suggestion: "Operation between incompatible types.\nCommon causes:\n1. Adding string and number: 'hello' + 5\n2. Calling string method on non-string\n3. Wrong type for function argument\n4. List/string operation on wrong type\n\nTry:\n1. Convert types: str(5), int('5')\n2. Check types before operation: type(variable)\n3. Use isinstance() for type checking\n4. Ensure correct types for operations".to_string(),
                alternative_commands: vec![
                    "Use type() or isinstance() to check types".to_string(),
                    "Convert types: str(), int(), float(), list()".to_string(),
                ],
                confidence: 0.85,
            })
        } else if output.contains("is not subscriptable") {
            Some(ErrorPattern {
                error_type: "python_subscriptable_error".to_string(),
                pattern_name: "Type Not Subscriptable".to_string(),
                suggestion: "Trying to index or slice an object that doesn't support it.\nCommon causes:\n1. Trying to index a non-iterable: number[0]\n2. Using wrong variable (integer instead of list)\n3. Forgot to convert to list/set/string\n\nTry:\n1. Check variable type: type(variable)\n2. Ensure it's a list, tuple, string, or other iterable\n3. Convert if needed: list(var), str(var)".to_string(),
                alternative_commands: vec![
                    "type(variable) - check the type".to_string(),
                    "isinstance(var, (list, tuple, str))".to_string(),
                    "list(var) - convert to list".to_string(),
                ],
                confidence: 0.90,
            })
        } else if output.contains("object is not callable") {
            Some(ErrorPattern {
                error_type: "python_callable_error".to_string(),
                pattern_name: "Object Not Callable".to_string(),
                suggestion: "Trying to call something that isn't a function.\nCommon causes:\n1. Calling a variable that holds data, not a function\n2. Missing parentheses when calling a function (class vs class())\n3. Overwrote a function name with a variable\n4. Using property as method: obj.method instead of obj.method()\n\nTry:\n1. Check if it's actually a function: callable(obj)\n2. Ensure parentheses: function() instead of function\n3. Check for variable names shadowing functions\n4. Verify object type and available methods".to_string(),
                alternative_commands: vec![
                    "callable(obj) - check if callable".to_string(),
                    "type(obj) - check the type".to_string(),
                    "dir(obj) - list available attributes".to_string(),
                ],
                confidence: 0.85,
            })
        } else {
            Some(ErrorPattern {
                error_type: "python_generic_type_error".to_string(),
                pattern_name: "Python Type Error".to_string(),
                suggestion: "Type error in Python code.\nTry:\n1. Check types of variables involved: type(var)\n2. Verify operation is valid for the types\n3. Check function signature for expected types\n4. Use type hints for better error messages".to_string(),
                alternative_commands: vec![
                    "type(variable) - check the type".to_string(),
                    "Use isinstance() for type checking".to_string(),
                ],
                confidence: 0.75,
            })
        }
    }

    /// Detect attribute errors
    fn detect_attribute_error(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("AttributeError") {
            return None;
        }

        if output.contains("has no attribute") {
            Some(ErrorPattern {
                error_type: "python_attribute_error".to_string(),
                pattern_name: "Python Attribute Error".to_string(),
                suggestion: "Object doesn't have the requested attribute or method.\nCommon causes:\n1. Typo in attribute name\n2. Wrong object type (expected different class)\n3. Attribute not yet set\n4. Using wrong method name (check documentation)\n5. Import issue (module loaded but not the right one)\n\nTry:\n1. Check attribute name spelling\n2. Verify object type: type(obj)\n3. List available attributes: dir(obj)\n4. Check class definition\n5. Verify module imports".to_string(),
                alternative_commands: vec![
                    "dir(obj) - list all attributes".to_string(),
                    "type(obj) - check object type".to_string(),
                    "Check class definition and available methods".to_string(),
                ],
                confidence: 0.85,
            })
        } else if output.contains("module '") && output.contains("' has no attribute") {
            Some(ErrorPattern {
                error_type: "python_module_attribute_error".to_string(),
                pattern_name: "Module Has No Attribute".to_string(),
                suggestion: "The imported module doesn't have this attribute/function.\nCommon causes:\n1. Wrong module name\n2. Function/attribute name changed in newer version\n3. Need to import sub-module specifically\n4. Typo in attribute name\n\nTry:\n1. Check module version: pip show <module>\n2. List available attributes: dir(module)\n3. Read module documentation\n4. Import submodule: from module import submodule".to_string(),
                alternative_commands: vec![
                    "dir(module) - list available attributes".to_string(),
                    "help(module.function) - show documentation".to_string(),
                    "Check module documentation online".to_string(),
                ],
                confidence: 0.80,
            })
        } else {
            Some(ErrorPattern {
                error_type: "python_generic_attribute_error".to_string(),
                pattern_name: "Python Attribute Error".to_string(),
                suggestion: "Attribute access failed.\nTry:\n1. Check object type: type(obj)\n2. List available attributes: dir(obj)\n3. Verify attribute spelling\n4. Check object is properly initialized".to_string(),
                alternative_commands: vec![
                    "dir(obj) - show all attributes".to_string(),
                    "hasattr(obj, 'attr') - check if attribute exists".to_string(),
                ],
                confidence: 0.75,
            })
        }
    }

    /// Detect name errors
    fn detect_name_error(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("NameError") {
            return None;
        }

        if output.contains("is not defined") {
            Some(ErrorPattern {
                error_type: "python_name_not_defined".to_string(),
                pattern_name: "Python Name Not Defined".to_string(),
                suggestion: "Using a variable or function that hasn't been defined yet.\nCommon causes:\n1. Typo in variable/function name\n2. Used before definition (order matters)\n3. Defined in different scope (indentation block)\n4. Forgot to return value from function\n5. Case sensitivity (myVar vs myvar)\n\nTry:\n1. Check spelling of the name\n2. Ensure variable is defined before use\n3. Check indentation/scope\n4. Verify function returns a value".to_string(),
                alternative_commands: vec![
                    "Check spelling and case sensitivity".to_string(),
                    "Ensure variable is defined before use".to_string(),
                    "Check indentation blocks (scope)".to_string(),
                    "grep -n '<name>' <file> - find definition".to_string(),
                ],
                confidence: 0.90,
            })
        } else {
            Some(ErrorPattern {
                error_type: "python_generic_name_error".to_string(),
                pattern_name: "Python Name Error".to_string(),
                suggestion: "Name error in Python code.\nTry:\n1. Check if variable is defined\n2. Check for scope issues\n3. Verify spelling and case".to_string(),
                alternative_commands: vec![
                    "grep -n '<name>' <file>".to_string(),
                    "Check function/variable definitions".to_string(),
                ],
                confidence: 0.75,
            })
        }
    }

    /// Detect KeyError (dictionary access)
    fn detect_key_error(&self, output: &str) -> Option<ErrorPattern> {
        if !output.contains("KeyError") {
            return None;
        }

        Some(ErrorPattern {
            error_type: "python_key_error".to_string(),
            pattern_name: "Dictionary Key Not Found".to_string(),
            suggestion: "Trying to access a dictionary key that doesn't exist.\nCommon causes:\n1. Key doesn't exist in the dictionary\n2. Typo in key name\n3. Wrong dictionary (empty or different data)\n4. Case sensitivity issues\n\nTry:\n1. Use dict.get(key, default) to provide default value\n2. Check if key exists: if key in dict:\n3. List all keys: dict.keys()\n4. Use dict.get() for safe access\n5. Verify dictionary contents".to_string(),
            alternative_commands: vec![
                "dict.get(key, default) - safe access with default".to_string(),
                "if key in dict: - check before access".to_string(),
                "dict.keys() - list all keys".to_string(),
                "print(dict) - see dictionary contents".to_string(),
            ],
            confidence: 0.85,
        })
    }

    /// Helper: Extract module name from error message
    fn extract_module_name(&self, error: &str) -> String {
        // Look for patterns like "No module named 'module_name'"
        if let Some(start) = error.find("No module named") {
            let after = &error[start + 15..];
            if let Some(end) = after.find('\'') {
                return after[..end].to_string();
            }
            if let Some(end) = after.find('"') {
                return after[..end].to_string();
            }
        }

        // Also check for ModuleNotFoundError
        if let Some(start) = error.find("ModuleNotFoundError: No module named") {
            let after = &error[start + 37..];
            if let Some(end) = after.find(char::is_whitespace) {
                return after[..end]
                    .trim_matches('\'')
                    .trim_matches('"')
                    .to_string();
            }
            if let Some(end) = after.find('\n') {
                return after[..end]
                    .trim_matches('\'')
                    .trim_matches('"')
                    .to_string();
            }
        }

        // Common default modules
        if error.contains("requests") {
            return "requests".to_string();
        }
        if error.contains("numpy") {
            return "numpy".to_string();
        }
        if error.contains("pandas") {
            return "pandas".to_string();
        }
        if error.contains("flask") || error.contains("Flask") {
            return "flask".to_string();
        }

        "module_name".to_string()
    }
}

impl PatternDetector for PythonPatternDetector {
    fn detect_patterns(&self, output: &str) -> Vec<ErrorPattern> {
        let mut patterns = Vec::new();

        // Check each pattern type
        if let Some(pattern) = self.detect_import_error(output) {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_syntax_error(output) {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_type_error(output) {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_attribute_error(output) {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_name_error(output) {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_key_error(output) {
            patterns.push(pattern);
        }

        patterns
    }

    fn language(&self) -> &'static str {
        "python"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_import_error_detection() {
        let detector = PythonPatternDetector::new();
        let error = "ModuleNotFoundError: No module named 'requests'";
        let patterns = detector.detect_patterns(error);
        assert!(!patterns.is_empty());
        assert_eq!(patterns[0].error_type, "python_missing_module");
    }

    #[test]
    fn test_syntax_error_detection() {
        let detector = PythonPatternDetector::new();
        let error = "  File \"script.py\", line 10\n    if x = 5:\n          ^\nSyntaxError: invalid syntax";
        let patterns = detector.detect_patterns(error);
        assert!(!patterns.is_empty());
        assert!(
            patterns
                .iter()
                .any(|p| p.error_type == "python_syntax_error")
        );
    }

    #[test]
    fn test_indentation_error_detection() {
        let detector = PythonPatternDetector::new();
        let error = "IndentationError: unexpected indent";
        let patterns = detector.detect_patterns(error);
    }
}
