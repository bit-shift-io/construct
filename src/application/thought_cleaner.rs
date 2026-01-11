
fn clean_agent_thought(text: &str) -> String {
    let agent_thought = text
        .lines()
        .filter(|line| {
            let t = line.trim();
             !t.starts_with("Output:") 
            && !t.starts_with("```")
            && !t.starts_with("Compiling ")
            && !t.starts_with("Finished ")
            && !t.starts_with("Running ")
            && !t.starts_with("Checking ") // Cargo check
            && !t.starts_with("Creating ") // Cargo init
            && !t.starts_with("error")     // Catch error: and error[...]
            && !t.starts_with("warning")   // Catch warning: and warning[...]
            && !t.starts_with("|")         // Rustc error bars
            && !t.starts_with("=")         // Rustc notes
            && !t.starts_with("^")         // Rustc pointers
            && !t.starts_with("note:")     // Rustc notes
            && !t.starts_with("help:")     // Rustc help
            && !t.starts_with("...")
            && !t.is_empty()               // Remove blank lines
        })
        .collect::<Vec<_>>()
        .join("\n");
    
    agent_thought.trim().to_string()
}
