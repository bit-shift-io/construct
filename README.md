# Construct AI Bot

Construct is a Matrix bot that acts as an intelligent coding assistant, capable of executing commands, managing projects, and integrating with various AI providers.

## Zai Provider Setup (Important)

If you are using **Zai (GLM models)**, please note that Zai requires specific endpoints depending on your use case.

### 1. Coding Tasks (Recommended)
For code generation, refactoring, and software engineering tasks, you **MUST** use the dedicated coding endpoint.

Update your `data/config.yaml`:

```yaml
agents:
  zai_coding:
    protocol: "openai"
    model: "glm-4.7"
    # DEDICATED CODING ENDPOINT
    endpoint: "https://api.z.ai/api/coding/paas/v4/"
    api_key: "your-zai-api-key"
    requests_per_minute: 60
```

### 2. General Tasks
For general chat or Q&A that does not involve code generation:

```yaml
agents:
  zai_general:
    protocol: "openai"
    model: "glm-4.7"
    # GENERAL ENDPOINT
    endpoint: "https://api.z.ai/api/paas/v4/"
    api_key: "your-zai-api-key"
```

**Note:** Using the general endpoint for coding tasks may result in suboptimal performance or refusal to generate code.

## Quick Start

1.  Copy `config_example.yaml` to `data/config.yaml`.
2.  Configure your `services` (Matrix) and `agents` (AI Provider).
3.  Run `cargo run`.
