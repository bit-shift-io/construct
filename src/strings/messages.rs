

pub fn model_set(name: &str) -> String {
    format!("✅ **Model set to**: `{name}`")
}

pub const MODEL_RESET: &str = "✅ **Model reset to default**.";
pub const INVALID_MODEL: &str = "⚠️ **Invalid model index or name**.";
pub const NO_PROJECTS_FOUND: &str = "📂 **No projects found**.";
pub const AVAILABLE_PROJECTS_HEADER: &str = "**📂 Available Projects**\n";
pub const INVALID_PROJECT_NAME: &str = "⚠️ **Invalid project name**.";

pub fn admin_permission_denied(user: &str) -> String {
    format!("{user} you do not have permission to run terminal commands.")
}

pub fn directory_changed(path: &str) -> String {
    format!("📂 **Directory changed**: `{path}`")
}

pub const ACCESS_DENIED_SANDBOX: &str = "❌ Access denied: Path outside the sandbox.";
pub const DIRECTORY_NOT_FOUND: &str = "❌ Directory not found.";
pub const COMMAND_NO_OUTPUT: &str = "✅ (Command executed successfully, no output)";

pub fn write_plan_error(err: &str) -> String {
    format!("⚠️ Failed to write plan.md: {err}")
}

pub fn write_tasks_error(err: &str) -> String {
    format!("⚠️ Failed to write tasks.md: {err}")
}

pub fn feedback_modification(feedback: &str) -> String {
    format!("🔄 Modifying plan with feedback: *{feedback}*")
}

pub fn failed_modify(err: &str) -> String {
    format!("⚠️ **Failed to modify plan**:\n{err}")
}

pub fn plan_updated(content: &str) -> String {
    format!("📜 **Plan Updated**:\n\n{content}")
}

pub const LIMIT_REACHED: &str = "⚠️ **Limit Reached**: Stopped to prevent infinite loop.";
pub const STOP_REQUESTED: &str = "🛑 **Execution stopped by user.**";

pub fn execution_complete(result: &str, output: &str) -> String {
    format!("🏁 **Execution Complete**\n\n{result}{output}")
}



pub fn agent_says(msg: &str) -> String {
    format!("🤔 **Agent says**:\n{msg}")
}



pub const STOP_REQUEST_WAIT: &str = "🛑 **Stop requested**. Waiting for current step to finish...";

pub fn plan_approved(job: &str) -> String {
    format!("✅ Plan approved for: **{job}**\nStarting interactive execution...")
}

pub const NO_TASK_APPROVE: &str = "⚠️ **No task to approve**.";
pub const RESUMING_EXECUTION: &str = "🔄 **Resuming execution**...";
pub const NO_HISTORY_CONTINUE: &str = "⚠️ **No execution history found to continue**. Start a new task.";

pub const PLEASE_COMMIT_MSG: &str = "⚠️ **Please provide a commit message**: `.commit _message_`";

pub fn committed_msg(output: &str) -> String {
    format!("🚀 **Committed**:\n{output}")
}

pub const CHANGES_DISCARDED: &str = "🧹 **Changes discarded**.";
pub const CHECKING_MSG: &str = "🔍 **Checking**...";

pub fn check_result(result: &str) -> String {
    format!("🔍 **Check Result**:\n{result}")
}

pub const BUILDING_MSG: &str = "🔨 **Building**...";

pub fn build_result(result: &str) -> String {
    format!("🔨 **Build Result**:\n{result}")
}

pub const DEPLOYING_MSG: &str = "🚀 **Deploying**...";

pub fn deploy_result(result: &str) -> String {
    format!("🚀 **Deploy Result**:\n{result}")
}

pub fn code_block_output(content: &str) -> String {
    format!("```\n{content}\n```")
}









pub fn command_run_failed(err: &str) -> String {
    format!("Failed to run command: {err}")
}

pub fn shell_command_failed(err: &str) -> String {
    format!("Failed to run shell command: {err}")
}



pub const COMMAND_DENIED_USER: &str = "🚫 **Command denied by user**.";
pub const NO_PENDING_COMMAND: &str = "⚠️ **No pending command to approve/deny**.";











pub const NO_PROJECTS_CONFIGURED: &str = "⚠️ No `projects_dir` configured.";
pub const PROVIDE_PROJECT_NAME: &str = "⚠️ **Please provide a project name**: `.new _name_`";

pub fn project_exists(path: &str) -> String {
    format!("📂 **Project already exists**. Switched to: `{path}`\nSpecs detected.")
}

pub fn create_dir_failed(path: &str, err: &str) -> String {
    format!("\n❌ **Failed to create directory** `{path}`: {err}")
}

pub fn project_created(path: &str) -> String {
    format!("\n📂 **Created and set project directory to**: `{path}`\n📄 **Initialized specs**: `roadmap.md`, `changelog.md`")
}

pub const USE_TASK_TO_START: &str = "\n\nUse `.task` to start a new workflow.";

pub fn plan_generated(plan: &str, tasks: &str) -> String {
    format!("### Plan\n\n{plan}\n\n### Tasks generated.{tasks}\n")
}

pub fn plan_generation_failed(err: &str) -> String {
    format!("⚠️ **Failed to generate plan**:\n{err}")
}

pub const NO_ACTIVE_TASK_MODIFY: &str = "⚠️ No active task to modify. Use `.task` first.";

pub fn current_changes_header(diff: &str) -> String {
    format!("🔍 **Current Changes**:\n```diff\n{diff}\n```")
}

pub const INVALID_AGENT_SELECTION: &str = "⚠️ Invalid agent selection.";pub const AVAILABLE_AGENTS_HEADER: &str = "**🤖 Available Agents**\n\n";
pub const NO_AGENTS_AVAILABLE: &str = "No agents available.\n";
pub const AGENT_SWITCH_INSTRUCTION: &str = "\nUse `.agent <name|number>` to switch.";
pub const ACTIVE_AGENT_CONFIG_NOT_FOUND: &str = "⚠️ Active agent configuration not found.";
pub const NO_MODELS_FOUND: &str = "No models found or discovery not supported for this agent.\n";
pub const MODEL_SWITCH_INSTRUCTION: &str = "\nUse `.model <name|number>` to switch active model.";

pub fn models_header(agent: &str) -> String {
    format!("**🤖 Models for Agent: {}**\n\n", agent)
}



pub fn list_projects_failed(err: &str) -> String {
    format!("⚠️ Failed to list projects: {err}")
}

pub const NO_PROJECT_SET: &str = "📂 **No project set**. Use `.project _path_`";

pub fn project_set_to(path: &str) -> String {
    format!("📂 **Project info set to**: `{path}`")
}

pub fn path_not_dir(path: &str) -> String {
    format!("⚠️ `{path}` is not a directory or does not exist.")
}

pub const SPECIFY_FILES_READ: &str = "⚠️ **Please specify files**: `.read _file1_ _file2_`";

pub fn file_content_header(file: &str, content: &str) -> String {
    format!("**📄 `{file}`**\n```\n{content}\n```\n\n")
}

pub fn read_file_error(file: &str, err: &str) -> String {
    format!("❌ Failed to read `{file}`: {err}\n\n")
}

pub const SET_USAGE: &str = "⚠️ **Usage**: `.set _key_ _value_`";

pub fn unknown_set_variable(var: &str) -> String {
    format!("⚠️ Unknown variable `{var}`. Supported: `project`, `agent`")
}

pub const EMPTY_COMMAND: &str = "Empty command";

pub fn command_timed_out(duration: std::time::Duration) -> String {
    format!("Command timed out after {duration:?}. Consider breaking this into smaller steps or running in the background.")
}
