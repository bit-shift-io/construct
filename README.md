# 🤖 Construct

**Construct** is a powerful infinite workspace designed to orchestrate AI agents for complex coding tasks. It serves as your loading program, allowing you to spawn agents, execute plans, and manage workflows directly from a chat room.

## ✨ Features

- **🧩 Modular Architecture**: Clean separation between configuration, state, and coordination logic.
- **🗺️ Multi-Room & Multi-Agent**: Manage multiple project bridges and agent workflows simultaneously.
- **🤝 Generic Agent Interface**: Support for `gemini`, `claude`, `zai`, `groq`, `xai`, `openai`, and `deepai` agents with asynchronous execution and progress reporting.
- **📝 Task-Driven Workflow**: Initiate tasks, generate plans, refine them with feedback, and execute them.
- **🛠️ Integrated DevOps**: Built-in support for Git operations (`commit`, `diff`, `discard`) and custom build/deploy commands.
- **📂 File Management**: Inspect project files directly from the chat.
- **⏱️ Command Timeouts**: Automatic timeouts prevent hanging commands (30s/120s/600s based on category).
- **📋 Three-Stage Feed System**: Progressive progress tracking reduces chat spam from 50+ messages to 1 updating feed.
- **📁 Project-Based State Storage**: All project state (feed, history, tasks) stored as markdown files in project directories.
- **⚡ Extensible**: Easily add new agent backends or custom shell commands via configuration.

## 🚀 Getting Started

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- A Matrix account for the bot.
- **AI Configuration** - Choose one or more providers:
  - **Gemini**: Requires `GEMINI_API_KEY` environment variable
  - **Claude/Anthropic**: Requires `ANTHROPIC_API_KEY` environment variable
  - **Zai**: Requires `ZAI_API_KEY` environment variable
  - **Groq**: Requires `GROQ_API_KEY` environment variable
  - **xAI**: Requires `XAI_API_KEY` environment variable
  - **OpenAI**: Requires `OPENAI_API_KEY` environment variable
  - **DeepAI**: Requires `DEEPAI_API_KEY` environment variable

### Installation

1. **Clone the repository**
   ```bash
   git clone <repository-url>
   cd construct
   ```

2. **Create the data directory**
   ```bash
   mkdir -p data
   ```

3. **Configure the bot**
   ```bash
   cp config_example.yaml data/config.yaml
   # Edit data/config.yaml with your settings
   ```

4. **Set your API keys**
   ```bash
   # Example for Zai
   export ZAI_API_KEY="your-zai-api-key"
   
   # Or add to your ~/.bashrc or ~/.zshrc
   echo 'export ZAI_API_KEY="your-zai-api-key"' >> ~/.bashrc
   ```

5. **Run the bot**
   ```bash
   cargo run
   ```

## 🎮 Commands

### Project Management
- `.list`: List available projects in the configured projects directory
- `.set project <path>`: Set the active project directory
- `.agent <name|index>`: Select the active AI agent for the room (e.g., `zai`, `gemini`, `claude`, `groq`)
- `.agents`: List available agents configured in the bot
- `.status`: Show current bot state (active project, current task, active agent)
- `.read <file>`: View the content of a file
- `.new`: Clear the current task state to start fresh

### Task Workflow
- `.task <desc>`: State a goal and generate a `plan.md` using the active agent
- `.modify <feedback>`: Provide feedback to refine an existing plan
- `.approve`: Execute the approved plan and generate a `walkthrough.md`
- `.ask <msg>`: Ask a question to the active agent without starting a formal task
- `.reject`: Clear the current plan and task

### Git & DevOps
- `.changes`: View current `git diff`
- `.commit <message>`: Stage all files and commit with the specified message
- `.discard`: Revert all uncommitted changes
- `.rebuild`: Execute the configured rebuild command
- `.deploy`: Execute the configured deployment command

### ⚡ Admin Only
- `, <command>`: Execute a raw shell command on the host (system admin only)

## 🤖 Supported AI Providers

### Zai (GLM Models)
Zai provides access to General Language Models (GLM) through an Anthropic-compatible API.

**Available Models:**
- `glm-4.7` - Latest flagship model (default, 64K tokens)
- `glm-4.6` - High performance with 200K context
- `glm-4.5` - Base model (32K tokens)
- `glm-4.5-x` - Enhanced version
- `glm-4.5-air` - Lightweight
- `glm-4.5-airx` - Ultra-lightweight
- `glm-4.5-flash` - Fast responses (8K tokens)

**Configuration Example:**
```yaml
agents:
  zai:
    protocol: "zai"
    model: "glm-4.7"  # Optional, defaults to glm-4.7
    # Requires ZAI_API_KEY environment variable
```

### Gemini
Google's Gemini models with dynamic model discovery and fallback support.

**Configuration Example:**
```yaml
agents:
  gemini:
    protocol: "gemini"
    model: "gemini-1.5-flash"
    model_order:
      - "flash"  # Prefer flash models
      - "pro"    # Then pro models
    model_fallbacks:
      - "gemini-1.5-flash"
      - "gemini-1.5-pro"
    requests_per_minute: 10
    # Requires GEMINI_API_KEY environment variable
```

### Other Providers
- **Claude/Anthropic**: Use protocol `"claude"` or `"anthropic"`
- **OpenAI**: Use protocol `"openai"`
- **Groq**: Use protocol `"groq"`
- **xAI**: Use protocol `"xai"`
- **DeepAI**: Use protocol `"deepai"`

## 📁 Project-Based State Storage

Construct stores all project-specific state directly in project directories as markdown files, eliminating the need for complex cleanup systems and providing natural state isolation.

### State Files

Each project directory contains:

```
project/
├── feed.md           # Real-time execution feed (3 modes: Active/Squashed/Final)
├── state.md          # Command history & execution context
├── roadmap.md        # Project planning and task definitions
├── tasks.md          # Task progress tracking (maintained by agent)
└── walkthrough.md    # Change documentation (maintained by agent)
```

### Key Benefits

- **Natural Isolation**: Each project's state is completely independent
- **Portable**: State travels with the project directory
- **Git-Friendly**: All state in diff-able markdown files
- **Minimal Room State**: Only ~1KB vs old 50KB+ approach
- **Multi-Room Support**: Multiple rooms can share the same project
- **No Cleanup Needed**: Switching projects naturally isolates state

### Feed Modes

The `feed.md` file evolves through three modes:

1. **Active Mode** (During execution)
   - Verbose real-time updates with command outputs
   - Status indicators (⏳ running, ✅ success, ❌ failed)
   - Auto-saves after each action

2. **Squashed Mode** (When task completes)
   - Compresses to concise one-liners per completed task
   - Clean, scannable format

3. **Final Mode** (When all tasks complete)
   - Simple bullet list of all completed work
   - Professional summary

### State Lifecycle

**Project Creation:**
```bash
.new my-project
# Creates: roadmap.md, state.md
```

**Project Switching:**
```bash
.set project other-project
# Automatically loads other-project/feed.md and state.md
# No cleanup needed - natural isolation
```

**Multi-Room Collaboration:**
```bash
# Room 1
.set project /shared/project

# Room 2 (different room, same project)
.set project /shared/project
# Both rooms see same feed.md, state.md, etc.
```

### Implementation Details

- **ProjectStateManager** (`src/state/project.rs`): Manages `state.md` with command history
- **FeedManager** (`src/features/feed.rs`): Manages `feed.md` with three-mode evolution
- **Room State**: Minimal (~1KB) - only stores navigation pointers, not execution history

## 📚 Documentation Strategy

Construct separates long-term project context from short-term session artifacts to keep agents focused.

### Artifacts
- **plan.md**: Generated task plans created by the active agent
- **walkthrough.md**: Execution summaries and implementation details
- **feed.md**: Progressive activity feed that tracks task execution in real-time
- **agent.log**: Interaction logs for debugging and audit trails

**Feed System**
Construct uses a sophisticated three-stage feed system to dramatically reduce chat spam while providing comprehensive visibility:

1. **Active Feed** (During execution)
   - Shows every command execution in real-time
   - Displays command outputs (truncated to 300 chars)
   - Updates via message edits
   - Keeps last 15 recent activities

2. **Squashed Feed** (When task completes)
   - Compresses verbose details into concise one-liners
   - Each completed task becomes a single timestamped entry
   - Clean, scannable format

3. **Final Feed** (When all tasks complete)
   - Simple bullet list of all completed tasks
   - Easy to scan and review
   - Professional summary

The feed is automatically saved as `feed.md` in your project directory for persistent record-keeping.

### Session State
- Active project directory
- Current task and plan
- Selected agent for the room
- Git status and pending changes

## 🛠️ Configuration

Configure the bot via `data/config.yaml`. You can define:

### System Settings
```yaml
system:
  projects_dir: "/home/user/Projects"  # Base directory for projects
  admin:
    - "@user:matrix.org"               # Admin users
```

### Matrix Service
```yaml
services:
  matrix:
    username: "@bot:matrix.org"
    password: "your_password"
    homeserver: "https://matrix.org"
    display_name: "Agent Bot"  # Optional
```

### Agents
```yaml
agents:
  zai:
    protocol: "zai"
    model: "glm-4.7"
    # Requires ZAI_API_KEY environment variable
  
  gemini:
    protocol: "gemini"
    model: "gemini-1.5-flash"
    model_order:
      - "flash"
      - "pro"
    model_fallbacks:
      - "gemini-1.5-flash"
    requests_per_minute: 10
    # Requires GEMINI_API_KEY environment variable
```

### Bridges
```yaml
bridges:
  "Development":
    - service: "matrix"
      channel: "!room_id:matrix.org"
    - service: "zai"  # Default agent for this room
  
  "Staging":
    - service: "matrix"
      channel: "!room_id_2:matrix.org"
    - agents:        # Restrict to specific agents
        - "gemini"
        - "groq"
```

### Commands
```yaml
commands:
  default: "ask"      # Default mode: "ask", "allow", "block"
  ask:                # Commands that require confirmation
    - "sudo"
  allowed:            # Always allowed commands
    - "ls"
    - "cat"
    - "grep"
    - "git"
  blocked:            # Blocked commands
    - "su"
  
  # Command timeouts (in seconds)
  # Prevents commands from hanging indefinitely
  timeouts:
    short: 30   # Quick commands (ls, cat, grep, etc.)
    medium: 120 # Standard commands (git, build, test, etc.)
    long: 600   # Long-running commands (cargo build, npm install, etc.)
```

**Command Timeout System**
Commands are automatically categorized and time out after specified durations:
- **Short timeout (30s)**: Quick commands like `ls`, `cat`, `grep`
- **Medium timeout (120s)**: Standard commands like `git`, `cargo test`, `npm test`
- **Long timeout (600s)**: Long-running commands like `cargo build`, `npm install`

If a command times out, the agent will be notified and can break it into smaller steps.

## 🔧 Development

### Project Structure
```
construct/
├── src/
│   ├── agent/          # Agent implementations
│   │   ├── adapter.rs  # Provider adapters (zai, gemini, claude, etc.)
│   │   ├── factory.rs  # Agent factory
│   │   └── discovery.rs # Model discovery
│   ├── services/       # Service integrations
│   │   └── matrix.rs   # Matrix client
│   ├── admin.rs        # Admin commands
│   ├── bridge.rs       # Bridge management
│   ├── commands.rs     # Bot commands
│   ├── config.rs       # Configuration handling
│   ├── main.rs         # Entry point
│   ├── prompts.rs      # Prompt templates
│   ├── sandbox.rs      # Security sandbox
│   ├── state.rs        # Bot state management
│   ├── util.rs         # Utilities
│   └── wizard.rs       # Setup wizard
├── data/               # Runtime data (user-created)
├── prompts/            # System prompts
└── res/                # Resources
```

### Adding a New Provider

To add a new AI provider:

1. **Update dependencies** in `Cargo.toml` if needed
2. **Add provider import** in `src/agent/adapter.rs`
3. **Add model defaults** in the model name resolution section
4. **Implement provider case** in the match statement:
   ```rust
   "new_provider" => {
       let client = new_provider::Client::from_env();
       let agent = client.agent(&model_name).build();
       agent
           .prompt(&context.prompt)
           .await
           .map_err(|e| e.to_string())
   }
   ```
5. **Update documentation** in README.md

## 🐛 Troubleshooting

### Common Issues

**Bot doesn't respond to commands**
- Check that the bot is invited to the room
- Verify the bot has permission to send messages
- Check `agent.log` for errors

**Agent returns errors**
- Verify API keys are set: `echo $ZAI_API_KEY`
- Check rate limits in your agent configuration
- Review `agent.log` for detailed error messages

**Model not found**
- For Gemini: Check `model_fallbacks` list
- Verify the model name is correct for your provider
- Check provider documentation for available models

## 📝 License

This project is licensed under the MIT License.

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.