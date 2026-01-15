use crate::strings::templates;

/// A builder for rendering prompts with context.
pub struct PromptRenderer<'a> {
    template: &'a str,
    replacements: Vec<(&'a str, String)>,
}

impl<'a> PromptRenderer<'a> {
    pub fn new(template: &'a str) -> Self {
        Self {
            template,
            replacements: Vec::new(),
        }
    }

    pub fn set(mut self, key: &'a str, value: impl Into<String>) -> Self {
        self.replacements.push((key, value.into()));
        self
    }

    pub fn render(self) -> String {
        let mut result = self.template.to_string();
        for (key, value) in self.replacements {
            result = result.replace(key, &value);
        }

        // Validate: Check for unreplaced placeholders
        if let Some(start) = result.find("{{") {
            if let Some(end) = result[start..].find("}}") {
                let placeholder = &result[start..start + end + 2];
                // Check if it looks like a valid UPPERCASE variable {{VAR_NAME}}
                // This prevents flagging literal {{ }} usage if we ever have it, though unlikely to be partial.
                // For safety, we flag ANY {{...}} pattern as suspicious if it remains.
                tracing::error!("Construct: [PROMPT RENDER ERROR] Unreplaced placeholder found in output: {}", placeholder);
            }
        }
        
        result
    }
}

pub const NEW_PROJECT_TEMPLATE: &str = include_str!("../../prompts/new_project.md");
pub const ARCHITECT_TEMPLATE: &str = include_str!("../../prompts/architect.md");
pub const DEVELOPER_TEMPLATE: &str = include_str!("../../prompts/developer.md");
pub const CONTEXT_TEMPLATE: &str = include_str!("../../prompts/context.md");
pub const ASSISTANT_TEMPLATE: &str = include_str!("../../prompts/assistant.md");
pub const TOOLS_TEMPLATE: &str = include_str!("../../prompts/tools.md");



fn build_context(
    history: &str,
    progress: &str,
    roadmap: &str,
    architecture: &str,
    tasks_checklist: &str,
    plan: &str,
    guidelines: &str,
) -> String {
    PromptRenderer::new(CONTEXT_TEMPLATE)
        .set("{{HISTORY}}", history)
        .set("{{PROGRESS}}", progress)
        .set("{{ROADMAP}}", roadmap)
        .set("{{ARCHITECTURE}}", architecture)
        .set("{{TASKS_CHECKLIST}}", tasks_checklist)
        .set("{{PLAN}}", plan)
        .set("{{GUIDELINES}}", guidelines)
        .render()
}

pub fn new_project_prompt(name: &str, requirements: &str, workdir: &str, date: &str) -> String {
    let context = build_context(
        "(New Project)",
        "(New Project - No history)",
        "(No roadmap yet)",
        "(No architecture yet)",
        "(New Project Initialization)",
        "(No plan yet)",
        "(Review templates/guidelines.md)"
    );

    let architect_layer = PromptRenderer::new(ARCHITECT_TEMPLATE)
        .set("{{TEMPLATE_PLAN}}", templates::PLAN_TEMPLATE)
        .set("{{TEMPLATE_PROGRESS}}", templates::PROGRESS_TEMPLATE)
        .set("{{TEMPLATE_WALKTHROUGH}}", templates::WALKTHROUGH_TEMPLATE)
        .set("{{TEMPLATE_TASKS}}", templates::TASKS_TEMPLATE)
        .set("{{TEMPLATE_ROADMAP}}", templates::ROADMAP_TEMPLATE)
        .set("{{TEMPLATE_ARCHITECTURE}}", templates::ARCHITECTURE_TEMPLATE)
        .set("{{TOOLS}}", TOOLS_TEMPLATE)
        .set("{{CWD}}", workdir)
        .set("{{ACTIVE_TASK}}", ".")
        .set("{{CONTEXT}}", &context)
        .set("{{CURRENT_DATE}}", date)
        .render();

    let specific_instructions = PromptRenderer::new(NEW_PROJECT_TEMPLATE)
        .set("{{TEMPLATE_ROADMAP}}", templates::ROADMAP_TEMPLATE)
        .set("{{TEMPLATE_ARCHITECTURE}}", templates::ARCHITECTURE_TEMPLATE)
        .set("{{TEMPLATE_PLAN}}", templates::PLAN_TEMPLATE)
        .set("{{TEMPLATE_PROGRESS}}", templates::PROGRESS_TEMPLATE)
        .set("{{TEMPLATE_WALKTHROUGH}}", templates::WALKTHROUGH_TEMPLATE)
        .set("{{TEMPLATE_TASKS}}", templates::TASKS_TEMPLATE)
        .set("{{TEMPLATE_GUIDELINES}}", templates::GUIDELINES_TEMPLATE)
        .set("{{NAME}}", name)
        .set("{{REQUIREMENTS}}", requirements)
        .set("{{WORKDIR}}", workdir)
        .set("{{ACTIVE_TASK}}", ".")
        .set("{{CURRENT_DATE}}", date)
        .render();

    format!(
        "{}\n\n# SPECIFIC INSTRUCTIONS FOR NEW PROJECT\n{}",
        architect_layer, specific_instructions
    )
}

pub fn planning_mode_turn(
    cwd: &str,
    roadmap: &str,
    tasks_checklist: &str,
    plan: &str,
    architecture: &str,
    progress: &str,
    active_task: &str,
    history: &str,
    date: &str,
    guidelines: &str,
) -> String {
    let context = build_context(
        history,
        progress,
        roadmap,
        architecture,
        tasks_checklist,
        plan,
        guidelines,
    );

    PromptRenderer::new(ARCHITECT_TEMPLATE)
        .set("{{TEMPLATE_PLAN}}", templates::PLAN_TEMPLATE)
        .set("{{TEMPLATE_PROGRESS}}", templates::PROGRESS_TEMPLATE)
        .set("{{TEMPLATE_WALKTHROUGH}}", templates::WALKTHROUGH_TEMPLATE)
        .set("{{TEMPLATE_TASKS}}", templates::TASKS_TEMPLATE)
        .set("{{TEMPLATE_ROADMAP}}", templates::ROADMAP_TEMPLATE)
        .set("{{TEMPLATE_ARCHITECTURE}}", templates::ARCHITECTURE_TEMPLATE)
        .set("{{TOOLS}}", TOOLS_TEMPLATE)
        .set("{{CWD}}", cwd)
        .set("{{ACTIVE_TASK}}", active_task)
        .set("{{CONTEXT}}", &context)
        .set("{{CURRENT_DATE}}", date)
        .render()
}

pub fn execution_mode_turn(
    cwd: &str,
    roadmap: &str,
    tasks_checklist: &str,
    plan: &str,
    architecture: &str,
    progress: &str,
    active_task: &str,
    history: &str,
    date: &str,
    guidelines: &str,
) -> String {
    let context = build_context(
        history,
        progress,
        roadmap,
        architecture,
        tasks_checklist,
        plan,
        guidelines,
    );

    PromptRenderer::new(DEVELOPER_TEMPLATE)
        .set("{{CWD}}", cwd)
        .set("{{CONTEXT}}", &context)
        .set("{{ACTIVE_TASK}}", active_task)
        .set("{{CURRENT_DATE}}", date)
        .set("{{TOOLS}}", TOOLS_TEMPLATE)
        .render()
}

pub fn assistant_mode_turn(
    cwd: &str,
    roadmap: &str,
    tasks_checklist: &str,
    plan: &str,
    architecture: &str,
    progress: &str,
    history: &str,
    guidelines: &str,
) -> String {
    let context = build_context(
        history,
        progress,
        roadmap,
        architecture,
        tasks_checklist,
        plan,
        guidelines,
    );

    PromptRenderer::new(ASSISTANT_TEMPLATE)
        .set("{{CWD}}", cwd)
        .set("{{CONTEXT}}", &context)

        .render()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_renderer_basic() {
        let renderer = PromptRenderer::new("Hello {{NAME}}")
            .set("{{NAME}}", "World");
        assert_eq!(renderer.render(), "Hello World");
    }

    #[test]
    fn test_prompt_renderer_missing_key() {
        // This should pass the assertion but Log an ERROR.
        let renderer = PromptRenderer::new("Hello {{MISSING}}");
        assert_eq!(renderer.render(), "Hello {{MISSING}}");
    }

    #[test]
    fn test_prompt_renderer_partial_replace() {
        let renderer = PromptRenderer::new("{{A}} and {{B}}")
            .set("{{A}}", "Apple");
        // {{B}} remains
        assert_eq!(renderer.render(), "Apple and {{B}}");
    }
}
