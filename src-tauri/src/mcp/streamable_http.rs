//! MCP Streamable HTTP Transport
//!
//! Implementation of the MCP 2025-11-25 Streamable HTTP transport.
//! This is the primary transport for remote MCP servers hosted on VPS.
//!
//! Key features:
//! - Single endpoint for all operations
//! - Session management via Mcp-Session-Id header
//! - Support for streaming responses via SSE
//! - Bearer token / API key authentication

use async_trait::async_trait;
use futures_util::StreamExt;
use log::{debug, error, info, warn};
use parking_lot::RwLock;
use reqwest::{Client, Response, StatusCode};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use url::Url;

use super::auth::McpAuth;
use super::error::{McpError, McpResult};
use super::transport::McpTransport;
use super::types::*;

/// Streamable HTTP Transport implementation
pub struct StreamableHttpTransport {
    /// HTTP client with configured timeouts
    client: Client,
    /// MCP server endpoint URL
    endpoint: Url,
    /// Authentication mechanism (Bearer token, API key, etc.)
    auth: Option<Box<dyn McpAuth>>,
    /// Current session ID from server
    session_id: Arc<RwLock<Option<String>>>,
    /// Connection status
    connected: Arc<RwLock<bool>>,
    /// Request timeout in milliseconds
    timeout_ms: u64,
    /// Request ID counter for JSON-RPC
    request_id: AtomicU64,
    /// Server capabilities (cached after initialization)
    server_capabilities: Arc<RwLock<Option<ServerCapabilities>>>,
    /// Server info (cached after initialization)
    server_info: Arc<RwLock<Option<ServerInfo>>>,
}

impl StreamableHttpTransport {
    /// Create a new Streamable HTTP transport
    pub fn new(
        endpoint: impl Into<String>,
        auth: Option<Box<dyn McpAuth>>,
        timeout_ms: u64,
    ) -> McpResult<Self> {
        let endpoint_str = endpoint.into();
        let endpoint = Url::parse(&endpoint_str)?;

        // Build HTTP client with appropriate settings
        let client = Client::builder()
            .timeout(std::time::Duration::from_millis(timeout_ms))
            .connect_timeout(std::time::Duration::from_secs(10))
            .pool_max_idle_per_host(5)
            .build()
            .map_err(|e| McpError::TransportError(e.to_string()))?;

        Ok(Self {
            client,
            endpoint,
            auth,
            session_id: Arc::new(RwLock::new(None)),
            connected: Arc::new(RwLock::new(false)),
            timeout_ms,
            request_id: AtomicU64::new(1),
            server_capabilities: Arc::new(RwLock::new(None)),
            server_info: Arc::new(RwLock::new(None)),
        })
    }

    /// Get the next request ID
    fn next_request_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Build a request with proper headers
    fn build_request(&self, body: &serde_json::Value) -> McpResult<reqwest::RequestBuilder> {
        let mut request = self.client
            .post(self.endpoint.clone())
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream");

        // Add session ID if we have one
        if let Some(ref session_id) = *self.session_id.read() {
            request = request.header("Mcp-Session-Id", session_id.as_str());
        }

        // Apply authentication
        if let Some(ref auth) = self.auth {
            request = auth.apply(request);
        }

        request = request.json(body);
        Ok(request)
    }

    /// Send a request and handle the response
    async fn send_and_receive(&self, request: JsonRpcRequest) -> McpResult<JsonRpcResponse> {
        let body = serde_json::to_value(&request)?;
        let http_request = self.build_request(&body)?;

        debug!("Sending MCP request: {} (id: {:?})", request.method, request.id);

        let response = http_request
            .send()
            .await
            .map_err(McpError::from)?;

        // Check for session ID in response headers
        if let Some(session_id) = response.headers().get("Mcp-Session-Id") {
            if let Ok(sid) = session_id.to_str() {
                let mut current_sid = self.session_id.write();
                if current_sid.is_none() || current_sid.as_deref() != Some(sid) {
                    info!("MCP session ID: {}", sid);
                    *current_sid = Some(sid.to_string());
                }
            }
        }

        self.handle_response(response, &request.id).await
    }

    /// Handle the HTTP response
    async fn handle_response(
        &self,
        response: Response,
        request_id: &serde_json::Value,
    ) -> McpResult<JsonRpcResponse> {
        let status = response.status();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        debug!("MCP response status: {}, content-type: {}", status, content_type);

        match status {
            StatusCode::OK | StatusCode::ACCEPTED => {
                if content_type.contains("text/event-stream") {
                    // Handle SSE streaming response
                    self.handle_sse_response(response, request_id).await
                } else {
                    // Handle regular JSON response
                    let json: JsonRpcResponse = response.json().await?;
                    if let Some(ref error) = json.error {
                        Err(McpError::JsonRpcError {
                            code: error.code,
                            message: error.message.clone(),
                        })
                    } else {
                        Ok(json)
                    }
                }
            }
            StatusCode::UNAUTHORIZED => {
                Err(McpError::AuthenticationFailed("Unauthorized".to_string()))
            }
            StatusCode::NOT_FOUND => {
                Err(McpError::ConnectionFailed("Endpoint not found".to_string()))
            }
            StatusCode::BAD_REQUEST => {
                let error_text = response.text().await.unwrap_or_default();
                Err(McpError::InvalidResponse(format!("Bad request: {}", error_text)))
            }
            _ => {
                let error_text = response.text().await.unwrap_or_default();
                Err(McpError::TransportError(format!(
                    "HTTP {}: {}",
                    status, error_text
                )))
            }
        }
    }

    /// Handle Server-Sent Events (SSE) streaming response
    async fn handle_sse_response(
        &self,
        response: Response,
        request_id: &serde_json::Value,
    ) -> McpResult<JsonRpcResponse> {
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut result: Option<JsonRpcResponse> = None;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| McpError::TransportError(e.to_string()))?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            // Process complete events
            while let Some(event_end) = buffer.find("\n\n") {
                let event_str = buffer[..event_end].to_string();
                buffer = buffer[event_end + 2..].to_string();

                if let Some(sse_event) = self.parse_sse_event(&event_str) {
                    // Try to parse as JSON-RPC response
                    if let Ok(response) = serde_json::from_str::<JsonRpcResponse>(&sse_event.data) {
                        if response.id == *request_id {
                            result = Some(response);
                        }
                    }
                }
            }
        }

        result.ok_or_else(|| McpError::InvalidResponse("No result in SSE stream".to_string()))
    }

    /// Parse an SSE event from text
    fn parse_sse_event(&self, text: &str) -> Option<SseEvent> {
        let mut event = None;
        let mut data = String::new();
        let mut id = None;

        for line in text.lines() {
            if line.starts_with("event:") {
                event = Some(line[6..].trim().to_string());
            } else if line.starts_with("data:") {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(line[5..].trim());
            } else if line.starts_with("id:") {
                id = Some(line[3..].trim().to_string());
            }
        }

        if data.is_empty() {
            None
        } else {
            Some(SseEvent { event, data, id })
        }
    }
}

#[async_trait]
impl McpTransport for StreamableHttpTransport {
    async fn connect(&mut self) -> McpResult<()> {
        info!("Connecting to MCP server at {}", self.endpoint);

        // Validate authentication
        if let Some(ref auth) = self.auth {
            if !auth.is_valid().await {
                return Err(McpError::TokenExpired);
            }
        }

        // Initialize the session
        let params = InitializeParams::default();
        let result = self.initialize(params).await?;

        // Store server info
        *self.server_info.write() = Some(result.server_info.clone());
        *self.server_capabilities.write() = Some(result.capabilities.clone());

        // Send initialized notification
        self.send_initialized().await?;

        *self.connected.write() = true;
        info!(
            "Connected to MCP server: {} (protocol: {})",
            result.server_info.name, result.protocol_version
        );

        Ok(())
    }

    async fn disconnect(&mut self) -> McpResult<()> {
        if !self.is_connected() {
            return Ok(());
        }

        info!("Disconnecting from MCP server");

        // Clear session state
        *self.session_id.write() = None;
        *self.connected.write() = false;
        *self.server_capabilities.write() = None;
        *self.server_info.write() = None;

        Ok(())
    }

    fn is_connected(&self) -> bool {
        *self.connected.read()
    }

    fn session_id(&self) -> Option<&str> {
        // This is a bit awkward due to the RwLock, but we need it for the trait
        // In practice, callers should use the Arc<RwLock<Option<String>>> directly
        None
    }

    async fn initialize(&mut self, params: InitializeParams) -> McpResult<InitializeResult> {
        let request = JsonRpcRequest::new(
            "initialize",
            Some(serde_json::to_value(&params)?),
            self.next_request_id(),
        );

        let response = self.send_and_receive(request).await?;

        let result: InitializeResult = response
            .result
            .ok_or_else(|| McpError::InvalidResponse("Missing result in initialize response".to_string()))
            .and_then(|v| serde_json::from_value(v).map_err(McpError::from))?;

        // Validate protocol version
        if result.protocol_version != MCP_PROTOCOL_VERSION {
            warn!(
                "Protocol version mismatch: expected {}, got {}",
                MCP_PROTOCOL_VERSION, result.protocol_version
            );
        }

        Ok(result)
    }

    async fn send_initialized(&self) -> McpResult<()> {
        self.send_notification("notifications/initialized", None).await
    }

    async fn list_tools(&self, cursor: Option<&str>) -> McpResult<ToolsListResult> {
        if !self.is_connected() {
            return Err(McpError::NotConnected);
        }

        let params = if let Some(c) = cursor {
            Some(serde_json::json!({ "cursor": c }))
        } else {
            None
        };

        let request = JsonRpcRequest::new("tools/list", params, self.next_request_id());
        let response = self.send_and_receive(request).await?;

        response
            .result
            .ok_or_else(|| McpError::InvalidResponse("Missing result".to_string()))
            .and_then(|v| serde_json::from_value(v).map_err(McpError::from))
    }

    async fn call_tool(&self, name: &str, arguments: Option<serde_json::Value>) -> McpResult<ToolCallResult> {
        if !self.is_connected() {
            return Err(McpError::NotConnected);
        }

        let params = ToolCallParams {
            name: name.to_string(),
            arguments,
        };

        let request = JsonRpcRequest::new(
            "tools/call",
            Some(serde_json::to_value(&params)?),
            self.next_request_id(),
        );

        let response = self.send_and_receive(request).await?;

        response
            .result
            .ok_or_else(|| McpError::InvalidResponse("Missing result".to_string()))
            .and_then(|v| serde_json::from_value(v).map_err(McpError::from))
    }

    async fn list_resources(&self, cursor: Option<&str>) -> McpResult<ResourcesListResult> {
        if !self.is_connected() {
            return Err(McpError::NotConnected);
        }

        let params = if let Some(c) = cursor {
            Some(serde_json::json!({ "cursor": c }))
        } else {
            None
        };

        let request = JsonRpcRequest::new("resources/list", params, self.next_request_id());
        let response = self.send_and_receive(request).await?;

        response
            .result
            .ok_or_else(|| McpError::InvalidResponse("Missing result".to_string()))
            .and_then(|v| serde_json::from_value(v).map_err(McpError::from))
    }

    async fn read_resource(&self, uri: &str) -> McpResult<serde_json::Value> {
        if !self.is_connected() {
            return Err(McpError::NotConnected);
        }

        let request = JsonRpcRequest::new(
            "resources/read",
            Some(serde_json::json!({ "uri": uri })),
            self.next_request_id(),
        );

        let response = self.send_and_receive(request).await?;

        response
            .result
            .ok_or_else(|| McpError::InvalidResponse("Missing result".to_string()))
    }

    async fn list_prompts(&self, cursor: Option<&str>) -> McpResult<PromptsListResult> {
        if !self.is_connected() {
            return Err(McpError::NotConnected);
        }

        let params = if let Some(c) = cursor {
            Some(serde_json::json!({ "cursor": c }))
        } else {
            None
        };

        let request = JsonRpcRequest::new("prompts/list", params, self.next_request_id());
        let response = self.send_and_receive(request).await?;

        response
            .result
            .ok_or_else(|| McpError::InvalidResponse("Missing result".to_string()))
            .and_then(|v| serde_json::from_value(v).map_err(McpError::from))
    }

    async fn get_prompt(&self, name: &str, arguments: Option<HashMap<String, String>>) -> McpResult<serde_json::Value> {
        if !self.is_connected() {
            return Err(McpError::NotConnected);
        }

        let params = serde_json::json!({
            "name": name,
            "arguments": arguments
        });

        let request = JsonRpcRequest::new("prompts/get", Some(params), self.next_request_id());
        let response = self.send_and_receive(request).await?;

        response
            .result
            .ok_or_else(|| McpError::InvalidResponse("Missing result".to_string()))
    }

    async fn send_request(&self, request: JsonRpcRequest) -> McpResult<JsonRpcResponse> {
        self.send_and_receive(request).await
    }

    async fn send_notification(&self, method: &str, params: Option<serde_json::Value>) -> McpResult<()> {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });

        let http_request = self.build_request(&notification)?;
        let response = http_request.send().await?;

        // Notifications should return 202 Accepted or 204 No Content
        match response.status() {
            StatusCode::OK | StatusCode::ACCEPTED | StatusCode::NO_CONTENT => Ok(()),
            status => {
                let error_text = response.text().await.unwrap_or_default();
                Err(McpError::TransportError(format!(
                    "Notification failed with HTTP {}: {}",
                    status, error_text
                )))
            }
        }
    }

    async fn ping(&self) -> McpResult<()> {
        let request = JsonRpcRequest::new("ping", None, self.next_request_id());
        self.send_and_receive(request).await?;
        Ok(())
    }

    fn transport_type(&self) -> &'static str {
        "streamable-http"
    }
}

impl std::fmt::Debug for StreamableHttpTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamableHttpTransport")
            .field("endpoint", &self.endpoint)
            .field("connected", &self.connected)
            .field("session_id", &self.session_id)
            .field("timeout_ms", &self.timeout_ms)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_transport() {
        let transport = StreamableHttpTransport::new(
            "https://mcp.example.com",
            None,
            30000,
        );
        assert!(transport.is_ok());
    }

    #[test]
    fn test_parse_sse_event() {
        let transport = StreamableHttpTransport::new(
            "https://mcp.example.com",
            None,
            30000,
        ).unwrap();

        let event_text = "event: message\ndata: {\"test\": true}\nid: 123";
        let event = transport.parse_sse_event(event_text);

        assert!(event.is_some());
        let event = event.unwrap();
        assert_eq!(event.event, Some("message".to_string()));
        assert_eq!(event.data, "{\"test\": true}");
        assert_eq!(event.id, Some("123".to_string()));
    }
}
