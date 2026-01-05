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
        .replace("{{ACTIVE_TASK}}", "tasks/001-init")
        .replace("{{ORIGINAL_REQUIREMENTS}}", requirements)
        .replace("{{ROADMAP}}", "(New Project: Initializing - No roadmap yet)")
        .replace("{{TASKS}}", "(New Project: Initializing - No tasks yet)")
        .replace("{{PLAN}}", "(New Project: Initializing - No plan yet)");


    let specific_instructions = NEW_PROJECT_TEMPLATE
        .replace("{{NAME}}", name)
        .replace("{{REQUIREMENTS}}", requirements)
        .replace("{{WORKDIR}}", workdir)
        .replace("{{ACTIVE_TASK}}", "tasks/001-init")
        .replace("{{TEMPLATE_ROADMAP}}", crate::strings::templates::ROADMAP_TEMPLATE)
        .replace("{{TEMPLATE_ARCHITECTURE}}", crate::strings::templates::ARCHITECTURE_TEMPLATE)
        .replace("{{TEMPLATE_PLAN}}", crate::strings::templates::PLAN_TEMPLATE)
        .replace("{{TEMPLATE_WALKTHROUGH}}", crate::strings::templates::WALKTHROUGH_TEMPLATE);
    
    // Combine them: Architect Persona + Specific Project Actions
    format!("{}\n\n# SPECIFIC INSTRUCTIONS FOR NEW PROJECT\n{}", architect_layer, specific_instructions)
}

pub fn planning_mode_turn(cwd: &str, roadmap: &str, request: &str, plan: &str, architecture: &str, active_task: &str) -> String {
    ARCHITECT_TEMPLATE
        .replace("{{CWD}}", cwd)
        .replace("{{ACTIVE_TASK}}", active_task)
        .replace("{{ORIGINAL_REQUIREMENTS}}", request)
        .replace("{{ROADMAP}}", roadmap)
        // TASKS placeholder is legacy, we map request to it if needed or remove from template
        // We will repurpose {{TASKS}} in template to be {{REQUEST}} but for now just map request to it
        .replace("{{TASKS}}", request) 
        .replace("{{PLAN}}", plan)
        .replace("{{ARCHITECTURE}}", architecture)
        .replace("{{TEMPLATE_PLAN}}", crate::strings::templates::PLAN_TEMPLATE)
        .replace("{{TEMPLATE_WALKTHROUGH}}", crate::strings::templates::WALKTHROUGH_TEMPLATE)

}

pub fn execution_mode_turn(cwd: &str, roadmap: &str, request: &str, plan: &str, architecture: &str, _active_task: &str) -> String {
    DEVELOPER_TEMPLATE
        .replace("{{CWD}}", cwd)
        .replace("{{ROADMAP}}", roadmap)
        .replace("{{ARCHITECTURE}}", architecture)
        .replace("{{TASKS}}", request)
        .replace("{{PLAN}}", plan)
}


