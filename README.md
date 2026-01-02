# Construct AI Bot (V2)

Construct is an intelligent, Matrix-based coding assistant designed to help developers build software autonomously. It uses the **Model Context Protocol (MCP)** for safe filesystem/terminal access and integrates with various LLM providers (Zai, OpenAI, Anthropic, Gemini).

## 🏗️ Architecture (V2)

The project follows a clean, hexagonal/layered architecture:

- **Domain**: Core business logic, traits (`ChatProvider`, `LlmProvider`), and types (`AppConfig`).
- **Application**: Orchestration logic.
    - `Router`: Dispatches commands.
    - `Engine`: The cognitive loop (Think -> Act -> Observe).
    - `Feed`: Real-time UI updates (Sticky messages).
- **Infrastructure**: Concrete implementations.
    - `Matrix`: Chat interface adapter.
    - `LLM`: Multi-provider client (Anthropic Prompt Caching, Gemini Context Caching).
    - `MCP`: Safe tool execution layer (Shell, Filesystem).
- **Interface**: Specific command handlers (`.task`, `.new`, `.status`, etc.).

## 🚀 Features

- **Autonomous Coding**: Give it a task (`.task "Refactor login"`) and it will plan, edit, and verify changes.
- **Project Aware**: Understands project context (`roadmap.md`, `tasks.md`).
- **Safe Execution**: All file and shell operations are proxied through an MCP server.
- **Multi-Provider**: Use the best model for the job (Claude 3.5 Sonnet, GPT-4o, Gemini 1.5 Pro).
- **Smart Feed**: A "sticky" UI that stays at the bottom of the chat, updating in real-time.
- **Full Control**: Use `.stop` to instantly abort long-running tasks, and `.ok` to approve sensitive plans before execution.

## ⚙️ Configuration

Copy `config_example.yaml` to `data/config.yaml` and configure your services.

### Core Configuration

```yaml
services:
  matrix:
    homeserver: "https://matrix.org"
    username: "@bot:matrix.org"
    password: "secure_password"

mcp:
  server_path: "target/debug/mcp-server"
  allowed_directories:
    - "/home/user/projects"
  readonly: false

system:
  projects_dir: "/home/user/projects"
  admin:
    - "@admin:matrix.org" 
```

### Zai Provider Setup (Important)

If you are using **Zai (GLM models)**, please note that Zai requires specific endpoints depending on your use case.

#### 1. Coding Tasks (Recommended)
For code generation, refactoring, and software engineering tasks, you **MUST** use the dedicated coding endpoint.

```yaml
agents:
  zai_coding:
    provider: "zai"
    model: "glm-4.7"
    # DEDICATED CODING ENDPOINT
    endpoint: "https://api.z.ai/api/coding/paas/v4/"
    api_key: "your-zai-api-key"
```

#### 2. General Tasks
For general chat or Q&A that does not involve code generation:

```yaml
agents:
  zai_general:
    provider: "zai"
    model: "glm-4.7"
    # GENERAL ENDPOINT
    endpoint: "https://api.z.ai/api/paas/v4/"
    api_key: "your-zai-api-key"
```

**Note:** Using the general endpoint for coding tasks may result in suboptimal performance or refusal to generate code.

## 🛠️ Commands

| Command | Description |
|Args|
|---|---|
| `.task <instruction>` | Start a new autonomous task. |
| `.ok` | Approve the current plan or step active in the wizard. |
| `.stop` | **Instantly** halt the current agent task. |
| `.new <name>` | Scaffold a new project structure. |
| `.project <path>` | Set the active project for the current room. |
| `.status` | Show current bot status (Active Project, Model, Task state). |
| `.ask <query>` | Context-aware Q&A about the project. |
| `.read <file>` | Read a file (MCP proxy). |
| `.run <cmd>` | Execute a shell command (Admin only, MCP proxy). |
| `.list` | List available projects. |
| `.help` | Show help menu. |

## 📦 Logging

Logs are written to:
- **Console**: Standard Output.
- **File**: `data/session.log` (Clean text format).

_Matrix logging is disabled by default to prevent spam._

## 🏃 Running

```bash
cargo run
```

_Ensure your MCP server is compatible or built alongside the bot._
