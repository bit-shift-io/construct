# 🤖 Construct

**Construct** is a powerful infinite workspace designed to orchestrate AI agents for complex coding tasks. It serves as your loading program, allowing you to spawn agents, execute plans, and manage workflows directly from a chat room.

## ✨ Features

- **🧩 Modular Architecture**: Clean separation between configuration, state, and coordination logic.
- **🗺️ Multi-Room & Multi-Agent**: Manage multiple project bridges and agent workflows simultaneously.
- **🤝 Generic Agent Interface**: Support for `gemini`, `claude`, `zai`, `groq`, `xai`, `openai`, and `deepai` agents with asynchronous execution and progress reporting.
- **📝 Task-Driven Workflow**: Initiate tasks, generate plans, refine them with feedback, and execute them.
- **🛠️ Integrated DevOps**: Built-in support for Git operations (`commit`, `diff`, `discard`) and custom build/deploy commands.
- **📂 File Management**: Inspect project files directly from the chat.
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

## 📚 Documentation Strategy

Construct separates long-term project context from short-term session artifacts to keep agents focused.

### Artifacts
- **plan.md**: Generated task plans created by the active agent
- **walkthrough.md**: Execution summaries and implementation details
- **agent.log**: Interaction logs for debugging and audit trails

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
```

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