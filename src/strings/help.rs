//! # Help Text
//!
//! detailed help messages and documentation for bot commands.
//! Displayed to the user via the `.help` command.

pub const MAIN: &str = concat!(
    "**🤖 Construct Help**\n",
    "Use: .command _args_\n",
    "\n",
    "**📂 Project**\n",
    "* project [path]: Set project directory\n",
    "* list: List projects\n",
    "* new: Reset/create project\n",
    "* ask [msg]: Chat with agent\n",
    "* task: Start a new task\n",
    "* start: Start/resume tasks\n",
    "* stop: Stop tasks\n",
    "\n",
    "**🐙 Git**\n",
    "* changes\n",
    "* commit [msg]\n",
    "* discard\n",
    "\n",
    "**🔨 Build**\n",
    "* check\n",
    "* build\n",
    "* deploy\n",
    "\n",
    "**⚡ Misc**\n",
    "* , [cmd]: Terminal command\n",
    "* agent: Configure agent & model\n",
    "* read [files]\n",
    "* status\n"
);
