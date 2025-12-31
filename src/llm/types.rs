//! Simple types for LLM API wrapper

/// Message role
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

impl MessageRole {
    pub fn as_str(&self) -> &str {
        match self {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
        }
    }
}

/// A chat message
#[derive(Debug, Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
        }
    }
}

/// Cache configuration for native provider caching
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum age for cached content (for Gemini)
    pub max_age_seconds: Option<u32>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_age_seconds: None,
        }
    }
}

/// Context for an LLM request
#[derive(Debug, Clone)]
pub struct Context {
    pub messages: Vec<Message>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub cache: Option<CacheConfig>,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            model: None,
            temperature: None,
            max_tokens: None,
            cache: None,
        }
    }
}

impl Context {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn prompt(text: impl Into<String>) -> Self {
        Self {
            messages: vec![Message::user(text)],
            ..Default::default()
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    pub fn with_cache(mut self, cache: CacheConfig) -> Self {
        self.cache = Some(cache);
        self
    }

    pub fn add_message(mut self, message: Message) -> Self {
        self.messages.push(message);
        self
    }

    pub fn add_system_message(mut self, content: impl Into<String>) -> Self {
        self.messages.push(Message::system(content));
        self
    }

    pub fn add_user_message(mut self, content: impl Into<String>) -> Self {
        self.messages.push(Message::user(content));
        self
    }

    pub fn add_assistant_message(mut self, content: impl Into<String>) -> Self {
        self.messages.push(Message::assistant(content));
        self
    }
}

/// Token usage information
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub cached_tokens: Option<u32>,
}

/// Response from an LLM
#[derive(Debug, Clone)]
pub struct Response {
    pub content: String,
    pub model: String,
    pub usage: TokenUsage,
    pub cached: bool,
}

/// LLM provider type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    OpenAI,
    Anthropic,
    Gemini,
    Groq,
    XAI,
    DeepAI,
    Zai,
}

impl Provider {
    pub fn as_str(&self) -> &str {
        match self {
            Provider::OpenAI => "openai",
            Provider::Anthropic => "anthropic",
            Provider::Gemini => "gemini",
            Provider::Groq => "groq",
            Provider::XAI => "xai",
            Provider::DeepAI => "deepai",
            Provider::Zai => "zai",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "openai" => Some(Provider::OpenAI),
            "anthropic" | "claude" => Some(Provider::Anthropic),
            "gemini" => Some(Provider::Gemini),
            "groq" => Some(Provider::Groq),
            "xai" => Some(Provider::XAI),
            "deepai" | "deep_ai" => Some(Provider::DeepAI),
            "zai" => Some(Provider::Zai),
            _ => None,
        }
    }
}

/// Error type
#[derive(Debug)]
pub struct Error {
    pub message: String,
    pub provider: String,
}

impl Error {
    pub fn new(provider: &str, message: impl Into<String>) -> Self {
        Self {
            provider: provider.to_string(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.provider, self.message)
    }
}

impl std::error::Error for Error {}
