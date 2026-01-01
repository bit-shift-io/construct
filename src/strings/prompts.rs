//! # Prompts
//!
//! Defines the system prompts and instructional text sent to the LLM.
//! These prompts guide the agent's behavior, role, and capabilities.

pub const SYSTEM: &str = include_str!("../../prompts/system.md");

pub const TASK_INSTRUCTIONS: &str = concat!(
    "1. Use the 'Project Roadmap' above for understanding the big picture and constraints.\n",
    "2. Your scope is STRICTLY limited to the 'Task' described above. Do NOT try to complete other roadmap items.\n",
    "3. Generate two files:\n",
    "   - `plan.md`: A detailed but CONCISE technical plan for THIS specific task.\n",
    "   - `tasks.md`: A checklist of the subtasks for THIS specific task.\n"
);

pub const TASK_FORMAT: &str = concat!(
    "plan.md\n",
    "```markdown\n",
    "...content...\n",
    "```\n\n",
    "tasks.md\n",
    "```markdown\n",
    "...content...\n",
    "```\n"
);

pub const NEW_PROJECT_INSTRUCTIONS: &str = "4. **NEW PROJECT DETECTED**: You MUST also generate `roadmap.md` based on the task requirements to replace the default placeholders.\n";

pub const NEW_PROJECT_FORMAT: &str = concat!(
    "\nroadmap.md\n",
    "```markdown\n",
    "...content...\n",
    "```\n"
);

pub fn modify_plan(system: &str, task: &str, plan: &str, feedback: &str) -> String {
    format!(
        "{system}\n\nOriginal Task: {task}\n\nCurrent Plan:\n{plan}\n\nFeedback: {feedback}\n\nPlease update the plan.md based on the feedback.\n\nIMPORTANT: Return the content of plan.md in a code block.\n"
    )
}

pub fn task_requirements_prompt(requirements: &str) -> String {
    format!("Task Requirements:\n\n{requirements}")
}

pub fn new_project_prompt(name: &str, requirements: &str, workdir: &str) -> String {
    format!(
        "Create a new project named '{name}'. \n\nGoal: Comprehensive implementation of the following requirements.\n\nRequirements:\n{requirements}\n\nIMPORTANT: The project directory is ALREADY created. You are ALREADY inside it (`{workdir}`). Do NOT run `mkdir` or `cd`."
    )
}

pub const ROADMAP_TEMPLATE: &str = "# Roadmap\n\n- [ ] Initial Setup";
pub const CHANGELOG_TEMPLATE: &str = "# Changelog\n\n## 0.1.0\n- Initialized";

pub fn roadmap_context(content: &str) -> String {
    format!("\n\n### Project Roadmap (Context Only)\n{content}\n")
}

pub fn initial_history_context(task: &str, plan: &str, tasks: &str, workdir: &str) -> String {
    format!(
        "Task: {task}\n\nCurrent Plan:\n{plan}\n\nTasks Checklist:\n{tasks}\n\nYou are executing this plan. We will do this step-by-step.\n\nYou are currently in directory: {workdir}\n"
    )
}

pub fn interactive_turn(cwd: &str, roadmap: &str, tasks: &str) -> String {
    format!(
        "{cwd}\n\n# Project Context\n## Roadmap\n{roadmap}\n\n## Tasks\n{tasks}\n\n# Current Status\nBased on the plan and previous outputs, what is the NEXT single command to run?\n\n## RULES\n1. Check if `tasks.md` needs to be updated (mark completed items with `[x]`). If so, WRITE IT FIRST.\n2. Check if `walkthrough.md` needs to be updated with new changes or verification results. If so, WRITE IT.\n3. Return commands in code blocks, e.g., ```bash\\ncat file.txt\\n```.\n4. For multi-line file creation, use heredocs: ```bash\\ncat << 'EOF' > filename.txt\\ncontent\\nEOF\\n```.\n5. If finished with the plan, return ```bash\\necho DONE\\n```.\n6. All commands are automatically sandboxed - unsafe paths will be blocked.\n7. Do not output multiple commands in one turn unless chained with `&&`.\n8. Wait for the result before proceeding.\n9. CRITICAL: Do NOT put commentary or explanations inside the code block. ONLY the command itself. Conversational text belongs outside the code block.\n10. NEVER use interactive tools (`nano`, `vim`, `less`).\n11. All commands are subject to sandbox validation and automatic timeouts.\n12. Use standard shell redirection for file operations.\n"
    )
}
