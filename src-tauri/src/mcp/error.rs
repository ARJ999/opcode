//! MCP Error Types
//!
//! Comprehensive error handling for MCP operations

use thiserror::Error;

/// MCP-specific errors
#[derive(Error, Debug)]
pub enum McpError {
    // Transport errors
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Connection timeout after {0}ms")]
    ConnectionTimeout(u64),

    #[error("Transport not connected")]
    NotConnected,

    #[error("Transport error: {0}")]
    TransportError(String),

    // Protocol errors
    #[error("Protocol version mismatch: expected {expected}, got {actual}")]
    ProtocolVersionMismatch { expected: String, actual: String },

    #[error("Invalid JSON-RPC response: {0}")]
    InvalidResponse(String),

    #[error("JSON-RPC error {code}: {message}")]
    JsonRpcError { code: i32, message: String },

    #[error("Initialization failed: {0}")]
    InitializationFailed(String),

    // Authentication errors
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Token expired")]
    TokenExpired,

    #[error("Invalid credentials")]
    InvalidCredentials,

    // Operation errors
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Tool execution failed: {0}")]
    ToolExecutionFailed(String),

    #[error("Resource not found: {0}")]
    ResourceNotFound(String),

    #[error("Prompt not found: {0}")]
    PromptNotFound(String),

    // Health check errors
    #[error("Health check failed: {0}")]
    HealthCheckFailed(String),

    #[error("Server unhealthy: {0}")]
    ServerUnhealthy(String),

    // Serialization errors
    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    // Configuration errors
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Server not found: {0}")]
    ServerNotFound(String),

    // Generic errors
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Operation cancelled")]
    Cancelled,
}

impl From<reqwest::Error> for McpError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            McpError::ConnectionTimeout(30000)
        } else if err.is_connect() {
            McpError::ConnectionFailed(err.to_string())
        } else {
            McpError::TransportError(err.to_string())
        }
    }
}

impl From<serde_json::Error> for McpError {
    fn from(err: serde_json::Error) -> Self {
        McpError::DeserializationError(err.to_string())
    }
}

impl From<url::ParseError> for McpError {
    fn from(err: url::ParseError) -> Self {
        McpError::InvalidConfig(format!("Invalid URL: {}", err))
    }
}

/// Result type alias for MCP operations
pub type McpResult<T> = Result<T, McpError>;
