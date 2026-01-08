//! # Prompts
//!
//! Defines the system prompts and instructional text sent to the LLM.
//! These prompts guide the agent's behavior, role, and capabilities.

pub const NEW_PROJECT_TEMPLATE: &str = include_str!("../../prompts/new_project.md");
pub const ARCHITECT_TEMPLATE: &str = include_str!("../../prompts/architect.md");
pub const DEVELOPER_TEMPLATE: &str = include_str!("../../prompts/developer.md");
pub const CONTEXT_TEMPLATE: &str = include_str!("../../prompts/context.md");

// Unused prompts removed

// 1. Remove from new_project_prompt signature
pub fn new_project_prompt(name: &str, requirements: &str, workdir: &str, date: &str) -> String {
    let context = CONTEXT_TEMPLATE
        .replace("{{HISTORY}}", "(New Project)")
        .replace("{{PROGRESS}}", "(New Project - No history)")
        .replace("{{ROADMAP}}", "(No roadmap yet)")
        .replace("{{ARCHITECTURE}}", "(No architecture yet)")
        .replace("{{REQUEST}}", requirements) // Request is the requirements initially
        .replace("{{TASKS_CHECKLIST}}", "(New Project Initialization)")
        .replace("{{PLAN}}", "(No plan yet)");

    let architect_layer = ARCHITECT_TEMPLATE
        .replace("{{CWD}}", workdir)
        .replace("{{ACTIVE_TASK}}", "tasks/001-init")
        .replace("{{CONTEXT}}", &context)
        .replace("{{CURRENT_DATE}}", date) // Architect template has CURRENT_DATE
        .replace(
            "{{TEMPLATE_PLAN}}",
            crate::strings::templates::PLAN_TEMPLATE,
        )
        .replace(
            "{{TEMPLATE_PROGRESS}}",
            crate::strings::templates::PROGRESS_TEMPLATE,
        )
        .replace(
            "{{TEMPLATE_WALKTHROUGH}}",
            crate::strings::templates::WALKTHROUGH_TEMPLATE,
        )
        .replace("{{TOOLS}}", TOOLS_TEMPLATE);

    let specific_instructions = NEW_PROJECT_TEMPLATE
        .replace("{{NAME}}", name)
        .replace("{{REQUIREMENTS}}", requirements)
        .replace("{{WORKDIR}}", workdir)
        .replace("{{ACTIVE_TASK}}", "tasks/001-init")
        .replace("{{CURRENT_DATE}}", date) // Inject date into new project instructions
        .replace(
            "{{TEMPLATE_ROADMAP}}",
            crate::strings::templates::ROADMAP_TEMPLATE,
        )
        .replace(
            "{{TEMPLATE_ARCHITECTURE}}",
            crate::strings::templates::ARCHITECTURE_TEMPLATE,
        )
        .replace(
            "{{TEMPLATE_PLAN}}",
            crate::strings::templates::PLAN_TEMPLATE,
        )
        .replace(
            "{{TEMPLATE_PROGRESS}}",
            crate::strings::templates::PROGRESS_TEMPLATE,
        )
        .replace(
            "{{TEMPLATE_WALKTHROUGH}}",
            crate::strings::templates::WALKTHROUGH_TEMPLATE,
        );

    // Combine them: Architect Persona + Specific Project Actions
    format!(
        "{}\n\n# SPECIFIC INSTRUCTIONS FOR NEW PROJECT\n{}",
        architect_layer, specific_instructions
    )
}

pub fn planning_mode_turn(
    cwd: &str,
    roadmap: &str,
    request: &str,
    tasks_checklist: &str,
    plan: &str,
    architecture: &str,
    progress: &str,
    active_task: &str,
    history: &str,
    date: &str,
) -> String {
    let context = CONTEXT_TEMPLATE
        .replace("{{HISTORY}}", history)
        .replace("{{PROGRESS}}", progress)
        .replace("{{ROADMAP}}", roadmap)
        .replace("{{ARCHITECTURE}}", architecture)
        .replace("{{REQUEST}}", request)
        .replace("{{TASKS_CHECKLIST}}", tasks_checklist)
        .replace("{{PLAN}}", plan);

    ARCHITECT_TEMPLATE
        .replace("{{CWD}}", cwd)
        .replace("{{ACTIVE_TASK}}", active_task)
        .replace("{{CONTEXT}}", &context)
        .replace("{{CURRENT_DATE}}", date)
        .replace(
            "{{TEMPLATE_PLAN}}",
            crate::strings::templates::PLAN_TEMPLATE,
        )
        .replace(
            "{{TEMPLATE_PROGRESS}}",
            crate::strings::templates::PROGRESS_TEMPLATE,
        )
        .replace(
            "{{TEMPLATE_WALKTHROUGH}}",
            crate::strings::templates::WALKTHROUGH_TEMPLATE,
        )
        .replace("{{TOOLS}}", TOOLS_TEMPLATE)
}

pub fn execution_mode_turn(
    cwd: &str,
    roadmap: &str,
    request: &str,
    tasks_checklist: &str,
    plan: &str,
    architecture: &str,
    progress: &str,
    _active_task: &str,
    history: &str,
    date: &str,
) -> String {
    // Context Optimization: In Execution Mode, we prioritize Tasks and Plan.
    // Roadmap and Architecture are provided but conceptually we might want them summarized if we had a summarizer.
    // For now, we will pass them as is but we acknowledge this is where we would optimize.

    let context = CONTEXT_TEMPLATE
        .replace("{{HISTORY}}", history)
        .replace("{{PROGRESS}}", progress)
        .replace("{{ROADMAP}}", roadmap)
        .replace("{{ARCHITECTURE}}", architecture)
        .replace("{{REQUEST}}", request)
        .replace("{{TASKS_CHECKLIST}}", tasks_checklist)
        .replace("{{PLAN}}", plan);

    DEVELOPER_TEMPLATE
        .replace("{{CWD}}", cwd)
        .replace("{{CONTEXT}}", &context)
        .replace("{{CURRENT_DATE}}", date)
        .replace("{{TOOLS}}", TOOLS_TEMPLATE)
}

pub const CONVERSATIONAL_TEMPLATE: &str = include_str!("../../prompts/conversational.md");
pub const TOOLS_TEMPLATE: &str = include_str!("../../prompts/tools.md");
pub const CONVERSATIONAL_TOOLS: &str = r#"
# AVAILABLE TOOLS
1. **Switch Mode**:
```switch_mode planning``` (Use this when the user asks for a change or feature)
```switch_mode execution``` (Use this ONLY when a plan is approved and ready to build)

2. **List Directory**:
```list path/to/dir```

3. **Read File**:
```read path/to/file```

# LOCKED TOOLS
- **Write File**: NOT AVAILABLE in Conversational Mode. Switch to Planning/Execution.
- **Run Command**: NOT AVAILABLE in Conversational Mode. Switch to Planning/Execution.
"#;

pub fn conversational_mode_turn(
    cwd: &str,
    roadmap: &str,
    request: &str,
    plan: &str,
    history: &str,
) -> String {
    CONVERSATIONAL_TEMPLATE
        .replace("{{CWD}}", cwd)
        .replace("{{HISTORY}}", history)
        .replace("{{ROADMAP}}", roadmap)
        .replace("{{PLAN}}", plan)
        .replace("{{REQUEST}}", request)
        .replace("{{TOOLS}}", CONVERSATIONAL_TOOLS)
}
