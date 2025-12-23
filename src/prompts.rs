use lazy_static::lazy_static;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct HelpStrings {
    pub main: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct WizardStrings {
    pub project_name: String,
    pub project_type: String,
    pub stack: String,
    pub description: String,
    pub task_description: String,
    pub confirmation_task: String,
    pub confirmation_project: String,
    pub cancelled: String,
    pub invalid_selection: String,
    pub type_ok_or_cancel: String,
}

#[derive(Debug, Deserialize)]
pub struct PromptStrings {
    pub system: String,
    pub task_instructions: String,
    pub task_format: String,
    pub new_project_instructions: String,
    pub new_project_format: String,
    pub modify_plan: String,
    pub task_requirements_prompt: String,
    pub new_project_prompt: String,
    pub roadmap_template: String,
    pub architecture_template: String,
    pub changelog_template: String,
    pub roadmap_context: String,
    pub architecture_context: String,
    pub initial_history_context: String,
    pub agent_history_entry: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct MessageStrings {
    pub status_header: String,
    pub agent_list_header: String,
    pub no_agents: String,
    pub fallback_agent: String,
    pub active_room: String,
    pub model_override: String,
    pub model_set: String,
    pub model_reset: String,
    pub invalid_model: String,
    pub no_projects_found: String,
    pub available_projects_header: String,
    pub invalid_project_name: String,
    pub admin_permission_denied: String,
    pub directory_changed: String,
    pub access_denied_sandbox: String,
    pub directory_not_found: String,
    pub command_no_output: String,
    pub provider_error_model_switch: String,
    pub provider_error_agent_switch: String,
    pub write_plan_error: String,
    pub write_tasks_error: String,
    pub feedback_modification: String,
    pub failed_modify: String,
    pub plan_updated: String,
    pub limit_reached: String,
    pub stop_requested: String,
    pub writing_file: String,
    pub file_written: String,
    pub write_failed: String,
    pub execution_complete: String,
    pub agent_run_code: String,
    pub agent_output: String,
    pub agent_says: String,
    pub agent_error: String,
    pub stop_request_wait: String,
    pub plan_approved: String,
    pub no_task_approve: String,
    pub resuming_execution: String,
    pub no_history_continue: String,
    pub plan_rejected: String,
    pub please_commit_msg: String,
    pub committed_msg: String,
    pub changes_discarded: String,
    pub building_msg: String,
    pub build_result: String,
    pub deploying_msg: String,
    pub deploy_result: String,
    pub code_block_output: String,
    pub command_blocked: String,
    pub command_ask: String,
    pub command_not_allowed: String,
    pub command_unknown: String,
    pub command_run_failed: String,
    pub shell_command_failed: String,
    pub command_approval_request: String,
    pub command_denied_user: String,
    pub no_pending_command: String,
    pub missing_env_var: String,
    pub gemini_fetch_failed: String,
    pub gemini_api_error: String,
    pub gemini_parse_error: String,
    pub anthropic_fetch_failed: String,
    pub anthropic_api_error: String,
    pub anthropic_parse_error: String,
    pub deepai_request_failed: String,
    pub deepai_api_error: String,
    pub deepai_parse_error: String,
    pub unsupported_provider: String,
    pub gemini_quota_exceeded: String,

    pub gemini_api_hint: String,
    pub no_projects_configured: String,
    pub provide_project_name: String,
    pub project_exists: String,
    pub create_dir_failed: String,
    pub project_created: String,
    pub use_task_to_start: String,
    pub task_started: String,
    pub plan_generated: String,
    pub plan_generation_failed: String,
    pub no_active_task_modify: String,
    pub current_changes_header: String,
    pub output_truncated: String,
}

#[derive(Debug, Deserialize)]
pub struct LogStrings {
    pub config_loaded: String,
    pub login_success: String,
    pub setting_display_name: String,
    pub set_display_name_fail: String,
    pub sync_loop_start: String,
    pub sync_loop_fail: String,
    pub shutdown: String,
    pub shutdown_fail: String,
    pub bridge_joining: String,
    pub bridge_join_fail: String,
    pub bridge_join_success: String,
    pub invite_received: String,
    pub join_invite_fail: String,
    pub join_invite_success: String,
}

#[derive(Debug, Deserialize)]
pub struct Strings {
    pub help: HelpStrings,
    pub wizard: WizardStrings,
    pub prompts: PromptStrings,
    pub messages: MessageStrings,
    pub logs: LogStrings,
}

lazy_static! {
    pub static ref STRINGS: Strings = {
        let content = fs::read_to_string("res/strings.yaml")
            .expect("Failed to read res/strings.yaml");
        serde_yaml::from_str(&content)
            .expect("Failed to parse res/strings.yaml")
    };
}
