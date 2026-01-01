# Simple LLM API Wrapper

A minimal, straightforward wrapper around REST APIs for multiple LLM providers with native context caching support.

## Features

- **Simple API** - Just basic chat completions, no complex abstractions
- **Native Caching** - Support for provider-native caching:
  - Anthropic (Claude) prompt caching - saves up to 90% on long contexts
  - Gemini context caching - cache up to 1M tokens for up to 4 hours
- **Multiple Providers** - OpenAI, Anthropic, Gemini, Groq, XAI, DeepAI, Zai
- **Minimal Dependencies** - No streaming, no custom cache storage backends

## Quick Start

```rust
use construct::llm::{Client, Context, CacheConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client from your app config
    let client = Client::new(app_config);

    // Simple prompt
    let response = client.prompt("openai", "Hello, world!").await?;
    println!("Response: {}", response.content);

    // With conversation history
    let context = Context::new()
        .add_system_message("You are a helpful assistant.")
        .add_user_message("What is 2+2?")
        .add_assistant_message("2+2 equals 4.")
        .add_user_message("And what about 3+3?");

    let response = client.chat("anthropic", context).await?;
    println!("Response: {}", response.content);

    // With native caching (Anthropic/Gemini)
    let context = Context::new()
        .add_system_message("You are a coding expert.")
        .add_user_message("Explain Rust's ownership system.")
        .with_cache(CacheConfig {
            max_age_seconds: Some(3600), // 1 hour
        });

    let response = client.chat("gemini", context).await?;
    println!("Cached: {} (Tokens: {})", response.cached, response.usage.total_tokens);

    Ok(())
}
```

## Supported Providers

### OpenAI-Compatible API
Works with any provider that uses OpenAI's API format:
- **OpenAI** - `https://api.openai.com/v1`
- **Groq** - `https://api.groq.com/openai/v1`
- **xAI (Grok)** - `https://api.x.ai/v1`
- **DeepAI** - `https://api.deepai.com/v1`
- **Zai** - `https://api.z.ai/api/coding/paas/v4/responses`

API Key Environment Variables:
- `OPENAI_API_KEY`
- `GROQ_API_KEY`
- `XAI_API_KEY`
- `DEEPAI_API_KEY`
- `ZAI_API_KEY`

### Anthropic (Claude)
Native prompt caching support for system messages and long contexts.

API Key: `ANTHROPIC_API_KEY`

Default Models:
- `claude-3-5-sonnet-20241022`
- `claude-3-5-haiku-20241022`

### Gemini
Context caching support with up to 1M token capacity.

API Key: `GEMINI_API_KEY`

Default Models:
- `gemini-1.5-pro`
- `gemini-1.5-flash`

## API Reference

### Client

```rust
pub struct Client {
    // private fields
}
```

#### Methods

- `new(app_config: AppConfig) -> Self` - Create client from config
- `prompt(&self, provider: &str, prompt: &str) -> Result<Response, Error>` - Simple prompt
- `prompt_with_model(&self, provider: &str, model: &str, prompt: &str) -> Result<Response, Error>` - Prompt with specific model
- `chat(&self, provider: &str, context: Context) -> Result<Response, Error>` - Full chat with context
- `get_provider_config(&self, agent_name: &str) -> Result<ProviderConfig, Error>` - Get provider config for an agent

### Context

```rust
pub struct Context {
    pub messages: Vec<Message>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub cache: Option<CacheConfig>,
}
```

#### Methods

- `new() -> Self` - Create empty context
- `prompt(text: impl Into<String>) -> Self` - Create simple prompt context
- `with_model(self, model: impl Into<String>) -> Self` - Set model
- `with_temperature(self, temp: f32) -> Self` - Set temperature (0.0-1.0)
- `with_max_tokens(self, tokens: u32) -> Self` - Set max tokens
- `with_cache(self, cache: CacheConfig) -> Self` - Enable native caching
- `add_message(self, message: Message) -> Self` - Add message
- `add_system_message(self, content: impl Into<String>) -> Self` - Add system message
- `add_user_message(self, content: impl Into<String>) -> Self` - Add user message
- `add_assistant_message(self, content: impl Into<String>) -> Self` - Add assistant message

### Message

```rust
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}
```

#### Constructors

- `Message::system(content: impl Into<String>) -> Self`
- `Message::user(content: impl Into<String>) -> Self`
- `Message::assistant(content: impl Into<String>) -> Self`

### MessageRole

```rust
pub enum MessageRole {
    System,
    User,
    Assistant,
}
```

### Response

```rust
pub struct Response {
    pub content: String,
    pub model: String,
    pub usage: TokenUsage,
    pub cached: bool,
}
```

### TokenUsage

```rust
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub cached_tokens: Option<u32>,  // For Anthropic/Gemini native caching
}
```

### CacheConfig

```rust
pub struct CacheConfig {
    pub max_age_seconds: Option<u32>,  // Cache lifetime
}
```

### Provider

```rust
pub enum Provider {
    OpenAI,
    Anthropic,
    Gemini,
    Groq,
    XAI,
    DeepAI,
    Zai,
}
```

### Error

```rust
pub struct Error {
    pub message: String,
    pub provider: String,
}
```

## Configuration

Providers are configured through your `AppConfig` in `data/config.yaml`:

```yaml
agents:
  openai:
    protocol: "openai"
    model: "gpt-4o"
    api_key_env: "OPENAI_API_KEY"
    requests_per_minute: 50

  claude:
    protocol: "anthropic"
    model: "claude-3-5-sonnet-20241022"
    api_key_env: "ANTHROPIC_API_KEY"

  gemini:
    protocol: "gemini"
    model: "gemini-1.5-pro"
    api_key_env: "GEMINI_API_KEY"

  groq:
    protocol: "openai"
    model: "llama-3.3-70b-versatile"
    endpoint: "https://api.groq.com/openai/v1"
    api_key_env: "GROQ_API_KEY"
```

## Native Caching

### Anthropic Prompt Caching

When you enable caching with Anthropic, the first message in your context (typically a system message or large document) will be cached. Subsequent requests with the same cached content get discounted pricing (up to 90% savings).

```rust
let context = Context::new()
    .add_system_message("You are an expert on Rust programming...")
    .add_user_message("What is ownership?")
    .with_cache(CacheConfig {
        max_age_seconds: Some(300), // 5 minutes
    });
```

### Gemini Context Caching

Gemini allows caching up to 1M tokens for up to 4 hours. This is ideal for large codebases, documentation, or any long-form content that doesn't change frequently.

```rust
let context = Context::new()
    .add_system_message("You have access to the following codebase...")
    .add_user_message("Explain the authentication flow.")
    .with_cache(CacheConfig {
        max_age_seconds: Some(14400), // 4 hours
    });
```

## When to Use Caching

**Use caching for:**
- Long system messages that don't change
- Code repositories or documentation
- Frequently asked questions with fixed answers
- Multi-turn conversations with fixed context

**Don't use caching for:**
- One-off unique prompts
- Dynamic content that changes frequently
- Short interactions where overhead isn't worth it

## Migration from Old API

### Before (complex):
```rust
use construct::agent::{AgentContext, get_agent};

let context = AgentContext {
    prompt: "Hello".to_string(),
    working_dir: None,
    model: None,
    status_callback: None,
    abort_signal: None,
    project_state_manager: None,
};

let agent = get_agent("openai", config);
let response = agent.execute(&context).await?;
```

### After (simple):
```rust
use construct::llm::{Client, Context};

let client = Client::new(config);
let context = Context::prompt("Hello");
let response = client.chat("openai", context).await?;
```

## Design Philosophy

This library prioritizes simplicity over feature-completeness:

1. **No Streaming** - If you need streaming, you can add it. Most backend use cases don't need it.
2. **No Custom Cache Storage** - Use provider-native caching. If you need custom caching, add a layer yourself.
3. **No Rate Limiting** - Handle rate limits at your application level.
4. **No Retry Logic** - Handle retries according to your needs.
5. **Minimal Abstractions** - Direct API calls, no traits or complex patterns.

## License

See project LICENSE file.