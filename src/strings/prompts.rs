//! # Prompts
//!
//! Defines the system prompts and instructional text sent to the LLM.
//! These prompts guide the agent's behavior, role, and capabilities.

pub const SYSTEM: &str = include_str!("../../prompts/system.md");

// Unused prompts removed

pub fn new_project_prompt(name: &str, requirements: &str, workdir: &str) -> String {
    format!(
        "Create a new project named '{name}'. \n\nGoal: Comprehensive implementation.\n\nRequirements:\n{requirements}\n\nIMPORTANT: The project directory is ALREADY created. You are ALREADY inside it (`{workdir}`).\n\nREQUIRED ACTIONS:\n1. Initialize specific project artifacts: `roadmap.md`, `tasks.md` (immediate steps), `architecture.md` (tech decisions), `changelog.md`, and `walkthrough.md` (verification log).\n2. Create a detailed technical plan in `plan.md`.\n3. STOP IMMEDIATELLY. Do NOT implement any code yet. Do NOT modify any other files. \n4. Output 'DONE' to finish this turn and wait for user approval."
    )
}

pub fn interactive_turn(cwd: &str, roadmap: &str, tasks: &str) -> String {
    format!(
        "{cwd}\n\n# Project Context\n## Roadmap\n{roadmap}\n\n## Tasks\n{tasks}\n\n# Current Status\nBased on the plan and previous outputs, what is the NEXT single command to run?\n\n## RULES\n1. Check if `tasks.md` needs to be updated (mark completed items with `[x]`). If so, WRITE IT FIRST.\n2. Check if `walkthrough.md` needs to be updated with new changes or verification results. If so, WRITE IT.\n3. Return commands in code blocks, e.g., ```bash\\ncat file.txt\\n```.\n4. For multi-line file creation, use heredocs: ```bash\\ncat << 'EOF' > filename.txt\\ncontent\\nEOF\\n```.\n5. If finished with the plan, return ```bash\\necho DONE\\n```.\n6. All commands are automatically sandboxed - unsafe paths will be blocked.\n7. Do not output multiple commands in one turn unless chained with `&&`.\n8. Wait for the result before proceeding.\n9. CRITICAL: Do NOT put commentary or explanations inside the code block. ONLY the command itself. Conversational text belongs outside the code block.\n10. NEVER use interactive tools (`nano`, `vim`, `less`).\n11. All commands are subject to sandbox validation and automatic timeouts.\n12. Use standard shell redirection for file operations.\n"
    )
}
