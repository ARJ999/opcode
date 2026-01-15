//! MCP (Model Context Protocol) Module
//!
//! This module implements the MCP specification 2025-11-25 with support for:
//! - Streamable HTTP transport (primary for remote servers)
//! - STDIO transport (for local servers)
//! - Bearer token / API key authentication
//! - Health monitoring
//! - Session management
//!
//! Opcode 2.0 - World's Greatest Claude Code Wrapper

pub mod transport;
pub mod streamable_http;
pub mod auth;
pub mod health;
pub mod types;
pub mod error;

pub use transport::{McpTransport, TransportConfig};
pub use streamable_http::StreamableHttpTransport;
pub use auth::{McpAuth, McpBearerAuth, McpApiKeyAuth};
pub use health::{McpHealthMonitor, HealthStatus, ServerHealth};
pub use types::*;
pub use error::McpError;
