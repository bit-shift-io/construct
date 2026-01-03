/// Sanitizes a path by replacing the configured project root with a simplified representation.
/// e.g. `/home/user/Projects/foo` -> `/foo` (if root is `/home/user/Projects`)
/// e.g. `/home/user/Projects` -> `/`
pub fn sanitize_path(path: &str, projects_dir: Option<&str>) -> String {
    if let Some(root) = projects_dir {
        // Normalize root by stripping trailing slash for consistent comparison
        let root = if root.ends_with('/') { &root[..root.len()-1] } else { root };
        
        if path.starts_with(root) {
            let relative = &path[root.len()..];
            if relative.is_empty() {
                return "/".to_string();
            }
            // Ensure we are matching a directory boundary or exact match
            // e.g. root=/foo, path=/foobar should NOT match (unless we want it to?)
            // Usually we want /foo/bar -> /bar.
            if relative.starts_with('/') {
                 return relative.to_string();
            }
        }
    }
    path.to_string()
}

/// Checks if a command contains any absolute paths that are NOT within the allowed root.
/// Returns `true` if safe (no suspicious args), `false` if unsafe.
pub fn check_command_safety(command: &str, projects_root: Option<&str>) -> bool {
    // Simple tokenizer: split by space.
    let tokens: Vec<&str> = command.split_whitespace().collect();
    
    for token in tokens {
        // Strip potential quoting
        let clean_token = token.trim_matches(|c| c == '\'' || c == '"');
        
        // 1. Check for Path Traversal
        if clean_token.contains("..") {
            return false; // suspicious relative path
        }

        // 2. Check Absolute Paths
        if clean_token.starts_with('/') {
            if let Some(root) = projects_root {
                 let normalized_root = if root.ends_with('/') { &root[..root.len()-1] } else { root };
                 // Must start with root to be safe
                 if !clean_token.starts_with(normalized_root) {
                     return false; // Accessing /etc, /var, etc.
                 }
            } else {
                // If no root configured, block all absolute paths for safety
                return false; 
            }
        }
        
        // 3. Optional: Block dangerous shell operators if needed (like > /etc/passwd)
        // Check if token contains > and an absolute path immediately after?
        // Basic split checks individual tokens. `>file` might be one token if no space.
        if clean_token.contains(">/") {
             // quick check for redirection to absolute path without space
             return false;
        }
    }
    true
}
