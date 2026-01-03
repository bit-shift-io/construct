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
    // Ideally we should handle quotes, but simple split caches most shell usage.
    let tokens: Vec<&str> = command.split_whitespace().collect();
    
    for token in tokens {
        // Strip potential quoting
        let clean_token = token.trim_matches(|c| c == '\'' || c == '"');
        
        if clean_token.starts_with('/') {
            // It's an absolute path.
            if let Some(root) = projects_root {
                 let normalized_root = if root.ends_with('/') { &root[..root.len()-1] } else { root };
                 // Must start with root to be safe
                 if !clean_token.starts_with(normalized_root) {
                     return false; // Accessing /etc, /var, etc.
                 }
            } else {
                // No root defined? Then absolute paths are technically "outside sandbox" logic if we enforce strictness.
                // But sandbox logic usually implies "restrict to X". If X is None, maybe Allow All?
                // The user said "sandbox to projects_dir".
                // If projects_dir is None (not configured), we probably shouldn't block everything?
                // Assuming defaults safe.
                return false; // Fail safe: if we are checking safety, absolute path with no config is suspicious?
            }
        }
    }
    true
}
