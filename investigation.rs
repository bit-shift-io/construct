
fn check_command_safety(command: &str, projects_root: Option<&str>) -> bool {
    let tokens: Vec<&str> = command.split_whitespace().collect();
    
    for token in tokens {
        let clean_token = token.trim_matches(|c| c == '\'' || c == '"');
        
        if clean_token.contains("..") { return false; }

        if clean_token.starts_with('/') {
            if let Some(root) = projects_root {
                 let normalized_root = if root.ends_with('/') { &root[..root.len()-1] } else { root };
                 println!("Debug: Checking '{}' starts_with '{}'", clean_token, normalized_root);
                 if !clean_token.starts_with(normalized_root) {
                     return false; 
                 }
            } else {
                return false; 
            }
        }
    }
    true
}

fn main() {
    let cmd = "cargo init --name a1 /a1";
    let root = "/a1";
    
    println!("Testing cmd='{}' root='{}'", cmd, root);
    let safe = check_command_safety(cmd, Some(root));
    println!("Safe? {}", safe);
    
    let root2 = "/home/bronson/Projects";
    println!("Testing cmd='{}' root='{}'", cmd, root2);
    let safe2 = check_command_safety(cmd, Some(root2));
    println!("Safe? {}", safe2);

    // Test sanitization assumption
    let real_cwd = "/a1";
    let projects_root_conf = "/a1";
    // If config root matches cwd, sanitize_path should give /
    // If engine prints sanitized path as relative to root, and it returns "/", usually sanitized logic checks relative path.
}
