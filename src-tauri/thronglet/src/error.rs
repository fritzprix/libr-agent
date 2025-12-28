use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("Initialization failed: {0}")]
    InitializationError(String),

    #[error("LLM Provider error: {0}")]
    LLMError(String),

    #[error("Tool Provider error: {0}")]
    ToolError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}
