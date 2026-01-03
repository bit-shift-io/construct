//! # Prompts
//!
//! Defines the system prompts and instructional text sent to the LLM.
//! These prompts guide the agent's behavior, role, and capabilities.

pub const NEW_PROJECT_TEMPLATE: &str = include_str!("../../prompts/new_project.md");
pub const ARCHITECT_TEMPLATE: &str = include_str!("../../prompts/architect.md");
pub const DEVELOPER_TEMPLATE: &str = include_str!("../../prompts/developer.md");

// Unused prompts removed

pub fn new_project_prompt(name: &str, requirements: &str, workdir: &str) -> String {
    let architect_layer = ARCHITECT_TEMPLATE
        .replace("{{CWD}}", workdir)
        .replace("{{ROADMAP}}", "(New Project: Initializing - No roadmap yet)")
        .replace("{{TASKS}}", "(New Project: Initializing - No tasks yet)")
        .replace("{{ARTIFACTS_INSTRUCTION}}", "Refer to the Specific Instructions below for required artifacts.");

    let specific_instructions = NEW_PROJECT_TEMPLATE
        .replace("{{NAME}}", name)
        .replace("{{REQUIREMENTS}}", requirements)
        .replace("{{WORKDIR}}", workdir);
    
    // Combine them: Architect Persona + Specific Project Actions
    format!("{}\n\n# SPECIFIC INSTRUCTIONS FOR NEW PROJECT\n{}", architect_layer, specific_instructions)
}

pub fn planning_mode_turn(cwd: &str, roadmap: &str, tasks: &str) -> String {
    ARCHITECT_TEMPLATE
        .replace("{{CWD}}", cwd)
        .replace("{{ROADMAP}}", roadmap)
        .replace("{{TASKS}}", tasks)
        .replace("{{ARTIFACTS_INSTRUCTION}}", "1. `tasks.md`: Detailed checklist of implementation steps.\n2. `implementation_plan.md`: Technical design and verification strategy.")
}

pub fn execution_mode_turn(cwd: &str, roadmap: &str, tasks: &str, plan: &str) -> String {
    DEVELOPER_TEMPLATE
        .replace("{{CWD}}", cwd)
        .replace("{{ROADMAP}}", roadmap)
        .replace("{{TASKS}}", tasks)
        .replace("{{PLAN}}", plan)
}

pub fn interactive_turn(cwd: &str, roadmap: &str, tasks: &str) -> String {
    DEVELOPER_TEMPLATE
        .replace("{{CWD}}", cwd)
        .replace("{{ROADMAP}}", roadmap)
        .replace("{{TASKS}}", tasks)
        .replace("{{PLAN}}", "(Interactive Mode)")
}
