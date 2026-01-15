//! MCP Authentication Module
//!
//! Supports Bearer tokens and API keys for remote MCP servers.
//! Designed for simplicity and security as per user requirements.

use async_trait::async_trait;
use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::error::{McpError, McpResult};
use super::types::McpAuthConfig;

/// Trait for MCP authentication mechanisms
#[async_trait]
pub trait McpAuth: Send + Sync {
    /// Apply authentication to a request
    fn apply(&self, request: RequestBuilder) -> RequestBuilder;

    /// Check if the authentication is valid/not expired
    async fn is_valid(&self) -> bool;

    /// Refresh credentials if needed (returns true if refreshed)
    async fn refresh(&mut self) -> McpResult<bool>;

    /// Get auth type name for logging
    fn auth_type(&self) -> &'static str;
}

/// Bearer token authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpBearerAuth {
    token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl McpBearerAuth {
    /// Create a new bearer token authentication
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            expires_at: None,
        }
    }

    /// Create with expiration time
    pub fn with_expiry(token: impl Into<String>, expires_at: chrono::DateTime<chrono::Utc>) -> Self {
        Self {
            token: token.into(),
            expires_at: Some(expires_at),
        }
    }

    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            chrono::Utc::now() >= expires_at
        } else {
            false
        }
    }

    /// Get the token value
    pub fn token(&self) -> &str {
        &self.token
    }
}

#[async_trait]
impl McpAuth for McpBearerAuth {
    fn apply(&self, request: RequestBuilder) -> RequestBuilder {
        request.header("Authorization", format!("Bearer {}", self.token))
    }

    async fn is_valid(&self) -> bool {
        !self.is_expired()
    }

    async fn refresh(&mut self) -> McpResult<bool> {
        // Bearer tokens don't auto-refresh - they need to be replaced
        if self.is_expired() {
            Err(McpError::TokenExpired)
        } else {
            Ok(false)
        }
    }

    fn auth_type(&self) -> &'static str {
        "Bearer"
    }
}

/// API Key authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpApiKeyAuth {
    /// Header name (e.g., "X-API-Key", "Authorization")
    header_name: String,
    /// The API key value
    api_key: String,
    /// Optional prefix (e.g., "ApiKey " for "Authorization: ApiKey xxx")
    #[serde(skip_serializing_if = "Option::is_none")]
    prefix: Option<String>,
}

impl McpApiKeyAuth {
    /// Create a new API key authentication with custom header
    pub fn new(header_name: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            header_name: header_name.into(),
            api_key: api_key.into(),
            prefix: None,
        }
    }

    /// Create with standard X-API-Key header
    pub fn x_api_key(api_key: impl Into<String>) -> Self {
        Self::new("X-API-Key", api_key)
    }

    /// Create with prefix
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Get the header name
    pub fn header_name(&self) -> &str {
        &self.header_name
    }
}

#[async_trait]
impl McpAuth for McpApiKeyAuth {
    fn apply(&self, request: RequestBuilder) -> RequestBuilder {
        let value = if let Some(prefix) = &self.prefix {
            format!("{}{}", prefix, self.api_key)
        } else {
            self.api_key.clone()
        };
        request.header(&self.header_name, value)
    }

    async fn is_valid(&self) -> bool {
        // API keys don't expire through this mechanism
        true
    }

    async fn refresh(&mut self) -> McpResult<bool> {
        // API keys don't refresh
        Ok(false)
    }

    fn auth_type(&self) -> &'static str {
        "ApiKey"
    }
}

/// Custom headers authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCustomHeadersAuth {
    headers: HashMap<String, String>,
}

impl McpCustomHeadersAuth {
    /// Create a new custom headers authentication
    pub fn new(headers: HashMap<String, String>) -> Self {
        Self { headers }
    }

    /// Add a header
    pub fn add_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }
}

#[async_trait]
impl McpAuth for McpCustomHeadersAuth {
    fn apply(&self, mut request: RequestBuilder) -> RequestBuilder {
        for (name, value) in &self.headers {
            request = request.header(name, value);
        }
        request
    }

    async fn is_valid(&self) -> bool {
        true
    }

    async fn refresh(&mut self) -> McpResult<bool> {
        Ok(false)
    }

    fn auth_type(&self) -> &'static str {
        "CustomHeaders"
    }
}

/// No authentication
#[derive(Debug, Clone, Default)]
pub struct McpNoAuth;

#[async_trait]
impl McpAuth for McpNoAuth {
    fn apply(&self, request: RequestBuilder) -> RequestBuilder {
        request
    }

    async fn is_valid(&self) -> bool {
        true
    }

    async fn refresh(&mut self) -> McpResult<bool> {
        Ok(false)
    }

    fn auth_type(&self) -> &'static str {
        "None"
    }
}

/// Create authentication from configuration
pub fn create_auth_from_config(config: &McpAuthConfig) -> Box<dyn McpAuth> {
    match config {
        McpAuthConfig::None => Box::new(McpNoAuth),
        McpAuthConfig::Bearer { token } => Box::new(McpBearerAuth::new(token.clone())),
        McpAuthConfig::ApiKey { header, value } => {
            Box::new(McpApiKeyAuth::new(header.clone(), value.clone()))
        }
        McpAuthConfig::CustomHeader { headers } => {
            Box::new(McpCustomHeadersAuth::new(headers.clone()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bearer_auth() {
        let auth = McpBearerAuth::new("test_token");
        assert_eq!(auth.token(), "test_token");
        assert!(!auth.is_expired());
    }

    #[test]
    fn test_api_key_auth() {
        let auth = McpApiKeyAuth::x_api_key("my_key");
        assert_eq!(auth.header_name(), "X-API-Key");
    }

    #[test]
    fn test_expired_bearer() {
        let past = chrono::Utc::now() - chrono::Duration::hours(1);
        let auth = McpBearerAuth::with_expiry("token", past);
        assert!(auth.is_expired());
    }
}
