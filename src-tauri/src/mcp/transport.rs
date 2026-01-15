//! MCP Transport Abstraction
//!
//! Provides a unified interface for different transport mechanisms:
//! - Streamable HTTP (primary for remote servers)
//! - STDIO (for local servers)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::auth::McpAuth;
use super::error::McpResult;
use super::types::*;

/// Transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TransportConfig {
    #[serde(rename = "streamable-http")]
    StreamableHttp {
        endpoint: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    #[serde(rename = "stdio")]
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
    },
    #[serde(rename = "sse")]
    #[deprecated(note = "Use streamable-http instead")]
    Sse {
        url: String,
    },
}

/// MCP Transport trait - defines the interface for all transport mechanisms
#[async_trait]
pub trait McpTransport: Send + Sync {
    /// Connect to the MCP server
    async fn connect(&mut self) -> McpResult<()>;

    /// Disconnect from the MCP server
    async fn disconnect(&mut self) -> McpResult<()>;

    /// Check if the transport is connected
    fn is_connected(&self) -> bool;

    /// Get the current session ID (if any)
    fn session_id(&self) -> Option<&str>;

    /// Initialize the MCP session
    async fn initialize(&mut self, params: InitializeParams) -> McpResult<InitializeResult>;

    /// Send initialized notification
    async fn send_initialized(&self) -> McpResult<()>;

    /// List available tools
    async fn list_tools(&self, cursor: Option<&str>) -> McpResult<ToolsListResult>;

    /// Call a tool
    async fn call_tool(&self, name: &str, arguments: Option<serde_json::Value>) -> McpResult<ToolCallResult>;

    /// List available resources
    async fn list_resources(&self, cursor: Option<&str>) -> McpResult<ResourcesListResult>;

    /// Read a resource
    async fn read_resource(&self, uri: &str) -> McpResult<serde_json::Value>;

    /// List available prompts
    async fn list_prompts(&self, cursor: Option<&str>) -> McpResult<PromptsListResult>;

    /// Get a prompt
    async fn get_prompt(&self, name: &str, arguments: Option<HashMap<String, String>>) -> McpResult<serde_json::Value>;

    /// Send a raw JSON-RPC request
    async fn send_request(&self, request: JsonRpcRequest) -> McpResult<JsonRpcResponse>;

    /// Send a notification (no response expected)
    async fn send_notification(&self, method: &str, params: Option<serde_json::Value>) -> McpResult<()>;

    /// Ping the server for health check
    async fn ping(&self) -> McpResult<()>;

    /// Get transport type name
    fn transport_type(&self) -> &'static str;
}

/// Transport events for streaming
#[derive(Debug, Clone)]
pub enum TransportEvent {
    /// Server sent a progress notification
    Progress(ProgressNotification),
    /// Server sent a log message
    Log { level: String, message: String },
    /// Server capabilities changed
    CapabilitiesChanged,
    /// Connection state changed
    ConnectionStateChanged { connected: bool },
    /// Error occurred
    Error(String),
}

/// Callback type for transport events
pub type TransportEventCallback = Box<dyn Fn(TransportEvent) + Send + Sync>;

/// Extended transport trait with event support
#[async_trait]
pub trait McpTransportWithEvents: McpTransport {
    /// Subscribe to transport events
    fn subscribe_events(&mut self, callback: TransportEventCallback);

    /// Unsubscribe from events
    fn unsubscribe_events(&mut self);
}

/// Transport factory for creating transports from configuration
pub struct TransportFactory;

impl TransportFactory {
    /// Create a transport from configuration
    pub fn create(
        config: TransportConfig,
        auth: Option<Box<dyn McpAuth>>,
    ) -> McpResult<Box<dyn McpTransport>> {
        match config {
            TransportConfig::StreamableHttp { endpoint, timeout_ms } => {
                use super::streamable_http::StreamableHttpTransport;
                let transport = StreamableHttpTransport::new(
                    endpoint,
                    auth,
                    timeout_ms.unwrap_or(30000),
                )?;
                Ok(Box::new(transport))
            }
            TransportConfig::Stdio { command, args, env } => {
                // TODO: Implement STDIO transport
                Err(super::error::McpError::InvalidConfig(
                    "STDIO transport not yet implemented in Opcode 2.0".to_string(),
                ))
            }
            #[allow(deprecated)]
            TransportConfig::Sse { url } => {
                // Convert SSE to Streamable HTTP (they share the same endpoint usually)
                log::warn!("SSE transport is deprecated, using Streamable HTTP instead");
                use super::streamable_http::StreamableHttpTransport;
                let transport = StreamableHttpTransport::new(url, auth, 30000)?;
                Ok(Box::new(transport))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_config_serialization() {
        let config = TransportConfig::StreamableHttp {
            endpoint: "https://mcp.example.com".to_string(),
            timeout_ms: Some(5000),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("streamable-http"));
    }
}
