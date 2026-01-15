//! Remote MCP Server Commands
//!
//! Tauri commands for managing remote MCP servers with Streamable HTTP transport.
//! Supports Bearer token and API key authentication.

use log::{error, info};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;

use crate::commands::agents::AgentDb;
use crate::mcp::{
    auth::{create_auth_from_config, McpApiKeyAuth, McpBearerAuth},
    error::McpError,
    health::{HealthStatus, McpHealthMonitor, ServerHealth},
    streamable_http::StreamableHttpTransport,
    transport::McpTransport,
    types::{ConnectionStatus, HealthCheckConfig, McpAuthConfig, RemoteMcpServer, ServerCapabilities, Tool},
};

/// Remote MCP server for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteMcpServerInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub endpoint: String,
    pub auth_type: String,
    pub status: String,
    pub health_enabled: bool,
    pub health_interval: u64,
    pub last_health_check: Option<String>,
    pub latency_ms: Option<u64>,
    pub created_at: String,
    pub updated_at: String,
}

/// Add remote MCP server request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddRemoteServerRequest {
    pub name: String,
    pub description: Option<String>,
    pub endpoint: String,
    pub auth_type: String,
    /// Bearer token (if auth_type is "bearer")
    pub token: Option<String>,
    /// API key header name (if auth_type is "api-key")
    pub api_key_header: Option<String>,
    /// API key value (if auth_type is "api-key")
    pub api_key_value: Option<String>,
    /// Custom headers as JSON (if auth_type is "custom-header")
    pub custom_headers: Option<String>,
    /// Health check enabled
    pub health_enabled: Option<bool>,
    /// Health check interval in seconds
    pub health_interval: Option<u64>,
}

/// Initialize remote MCP servers table
pub fn init_remote_mcp_table(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS remote_mcp_servers (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            endpoint TEXT NOT NULL,
            auth_type TEXT NOT NULL DEFAULT 'none',
            auth_config TEXT,
            health_enabled BOOLEAN DEFAULT 1,
            health_interval INTEGER DEFAULT 60,
            health_timeout INTEGER DEFAULT 10,
            status TEXT DEFAULT 'unknown',
            last_health_check TEXT,
            latency_ms INTEGER,
            capabilities TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    info!("Remote MCP servers table initialized");
    Ok(())
}

/// List all remote MCP servers
#[tauri::command]
pub async fn list_remote_mcp_servers(db: State<'_, AgentDb>) -> Result<Vec<RemoteMcpServerInfo>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    // Ensure table exists
    let _ = init_remote_mcp_table(&conn);

    let mut stmt = conn
        .prepare(
            "SELECT id, name, description, endpoint, auth_type, status, health_enabled,
             health_interval, last_health_check, latency_ms, created_at, updated_at
             FROM remote_mcp_servers ORDER BY created_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let servers = stmt
        .query_map([], |row| {
            Ok(RemoteMcpServerInfo {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                endpoint: row.get(3)?,
                auth_type: row.get(4)?,
                status: row.get::<_, String>(5).unwrap_or_else(|_| "unknown".to_string()),
                health_enabled: row.get::<_, bool>(6).unwrap_or(true),
                health_interval: row.get::<_, u64>(7).unwrap_or(60),
                last_health_check: row.get(8).ok(),
                latency_ms: row.get(9).ok(),
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(servers)
}

/// Add a new remote MCP server
#[tauri::command]
pub async fn add_remote_mcp_server(
    db: State<'_, AgentDb>,
    request: AddRemoteServerRequest,
) -> Result<RemoteMcpServerInfo, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    // Ensure table exists
    let _ = init_remote_mcp_table(&conn);

    // Generate ID
    let id = uuid::Uuid::new_v4().to_string();

    // Build auth config JSON
    let auth_config = match request.auth_type.as_str() {
        "bearer" => {
            let token = request.token.ok_or("Bearer token is required")?;
            serde_json::json!({
                "type": "bearer",
                "token": token
            })
        }
        "api-key" => {
            let header = request.api_key_header.unwrap_or_else(|| "X-API-Key".to_string());
            let value = request.api_key_value.ok_or("API key value is required")?;
            serde_json::json!({
                "type": "api-key",
                "header": header,
                "value": value
            })
        }
        "custom-header" => {
            let headers_str = request.custom_headers.ok_or("Custom headers are required")?;
            let headers: HashMap<String, String> = serde_json::from_str(&headers_str)
                .map_err(|e| format!("Invalid custom headers JSON: {}", e))?;
            serde_json::json!({
                "type": "custom-header",
                "headers": headers
            })
        }
        _ => serde_json::json!({ "type": "none" }),
    };

    let auth_config_str = serde_json::to_string(&auth_config).map_err(|e| e.to_string())?;
    let health_enabled = request.health_enabled.unwrap_or(true);
    let health_interval = request.health_interval.unwrap_or(60);

    conn.execute(
        "INSERT INTO remote_mcp_servers (id, name, description, endpoint, auth_type, auth_config, health_enabled, health_interval)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            id,
            request.name,
            request.description,
            request.endpoint,
            request.auth_type,
            auth_config_str,
            health_enabled,
            health_interval
        ],
    )
    .map_err(|e| e.to_string())?;

    info!("Added remote MCP server: {} ({})", request.name, id);

    // Return the created server
    Ok(RemoteMcpServerInfo {
        id,
        name: request.name,
        description: request.description,
        endpoint: request.endpoint,
        auth_type: request.auth_type,
        status: "unknown".to_string(),
        health_enabled,
        health_interval,
        last_health_check: None,
        latency_ms: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    })
}

/// Remove a remote MCP server
#[tauri::command]
pub async fn remove_remote_mcp_server(db: State<'_, AgentDb>, id: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    conn.execute("DELETE FROM remote_mcp_servers WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;

    info!("Removed remote MCP server: {}", id);
    Ok(())
}

/// Test connection to a remote MCP server
#[tauri::command]
pub async fn test_remote_mcp_connection(
    db: State<'_, AgentDb>,
    id: String,
) -> Result<ServerHealth, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    // Get server details
    let (endpoint, auth_config_str): (String, Option<String>) = conn
        .query_row(
            "SELECT endpoint, auth_config FROM remote_mcp_servers WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Server not found: {}", e))?;

    drop(conn); // Release lock before async operations

    // Parse auth config
    let auth: Option<Box<dyn crate::mcp::auth::McpAuth>> = if let Some(config_str) = auth_config_str {
        let config: McpAuthConfig = serde_json::from_str(&config_str)
            .map_err(|e| format!("Invalid auth config: {}", e))?;
        Some(create_auth_from_config(&config))
    } else {
        None
    };

    // Create transport and test connection
    let mut transport = StreamableHttpTransport::new(&endpoint, auth, 30000)
        .map_err(|e| format!("Failed to create transport: {}", e))?;

    let start = std::time::Instant::now();
    let result = transport.connect().await;
    let latency = start.elapsed().as_millis() as u64;

    let health = match result {
        Ok(()) => {
            // Update status in database
            let conn = db.0.lock().map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE remote_mcp_servers SET status = 'connected', last_health_check = ?1, latency_ms = ?2, updated_at = ?3 WHERE id = ?4",
                params![chrono::Utc::now().to_rfc3339(), latency as i64, chrono::Utc::now().to_rfc3339(), id],
            ).map_err(|e| e.to_string())?;

            ServerHealth {
                server_id: id,
                status: HealthStatus::Healthy,
                latency_ms: Some(latency),
                last_check: Some(chrono::Utc::now().to_rfc3339()),
                last_error: None,
                consecutive_failures: 0,
                consecutive_successes: 1,
                avg_latency_ms: Some(latency),
            }
        }
        Err(e) => {
            // Update status in database
            let conn = db.0.lock().map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE remote_mcp_servers SET status = 'error', last_health_check = ?1, updated_at = ?2 WHERE id = ?3",
                params![chrono::Utc::now().to_rfc3339(), chrono::Utc::now().to_rfc3339(), id],
            ).map_err(|e| e.to_string())?;

            ServerHealth {
                server_id: id,
                status: HealthStatus::Unhealthy,
                latency_ms: None,
                last_check: Some(chrono::Utc::now().to_rfc3339()),
                last_error: Some(e.to_string()),
                consecutive_failures: 1,
                consecutive_successes: 0,
                avg_latency_ms: None,
            }
        }
    };

    Ok(health)
}

/// List tools from a remote MCP server
#[tauri::command]
pub async fn list_remote_mcp_tools(db: State<'_, AgentDb>, id: String) -> Result<Vec<Tool>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    // Get server details
    let (endpoint, auth_config_str): (String, Option<String>) = conn
        .query_row(
            "SELECT endpoint, auth_config FROM remote_mcp_servers WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Server not found: {}", e))?;

    drop(conn);

    // Parse auth config
    let auth: Option<Box<dyn crate::mcp::auth::McpAuth>> = if let Some(config_str) = auth_config_str {
        let config: McpAuthConfig = serde_json::from_str(&config_str)
            .map_err(|e| format!("Invalid auth config: {}", e))?;
        Some(create_auth_from_config(&config))
    } else {
        None
    };

    // Connect and list tools
    let mut transport = StreamableHttpTransport::new(&endpoint, auth, 30000)
        .map_err(|e| format!("Failed to create transport: {}", e))?;

    transport
        .connect()
        .await
        .map_err(|e| format!("Failed to connect: {}", e))?;

    let result = transport
        .list_tools(None)
        .await
        .map_err(|e| format!("Failed to list tools: {}", e))?;

    Ok(result.tools)
}

/// Call a tool on a remote MCP server
#[tauri::command]
pub async fn call_remote_mcp_tool(
    db: State<'_, AgentDb>,
    server_id: String,
    tool_name: String,
    arguments: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    // Get server details
    let (endpoint, auth_config_str): (String, Option<String>) = conn
        .query_row(
            "SELECT endpoint, auth_config FROM remote_mcp_servers WHERE id = ?1",
            params![server_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Server not found: {}", e))?;

    drop(conn);

    // Parse auth config
    let auth: Option<Box<dyn crate::mcp::auth::McpAuth>> = if let Some(config_str) = auth_config_str {
        let config: McpAuthConfig = serde_json::from_str(&config_str)
            .map_err(|e| format!("Invalid auth config: {}", e))?;
        Some(create_auth_from_config(&config))
    } else {
        None
    };

    // Connect and call tool
    let mut transport = StreamableHttpTransport::new(&endpoint, auth, 60000) // Longer timeout for tool calls
        .map_err(|e| format!("Failed to create transport: {}", e))?;

    transport
        .connect()
        .await
        .map_err(|e| format!("Failed to connect: {}", e))?;

    let result = transport
        .call_tool(&tool_name, arguments)
        .await
        .map_err(|e| format!("Tool call failed: {}", e))?;

    // Convert result to JSON
    serde_json::to_value(&result).map_err(|e| e.to_string())
}

/// Update remote MCP server configuration
#[tauri::command]
pub async fn update_remote_mcp_server(
    db: State<'_, AgentDb>,
    id: String,
    name: Option<String>,
    description: Option<String>,
    endpoint: Option<String>,
    auth_type: Option<String>,
    token: Option<String>,
    api_key_header: Option<String>,
    api_key_value: Option<String>,
    health_enabled: Option<bool>,
    health_interval: Option<u64>,
) -> Result<RemoteMcpServerInfo, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    // Get current values
    let current: RemoteMcpServerInfo = conn
        .query_row(
            "SELECT id, name, description, endpoint, auth_type, status, health_enabled,
             health_interval, last_health_check, latency_ms, created_at, updated_at
             FROM remote_mcp_servers WHERE id = ?1",
            params![id],
            |row| {
                Ok(RemoteMcpServerInfo {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    endpoint: row.get(3)?,
                    auth_type: row.get(4)?,
                    status: row.get(5)?,
                    health_enabled: row.get(6)?,
                    health_interval: row.get(7)?,
                    last_health_check: row.get(8).ok(),
                    latency_ms: row.get(9).ok(),
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                })
            },
        )
        .map_err(|e| format!("Server not found: {}", e))?;

    // Build updated values
    let new_name = name.unwrap_or(current.name);
    let new_description = description.or(current.description);
    let new_endpoint = endpoint.unwrap_or(current.endpoint);
    let new_auth_type = auth_type.unwrap_or(current.auth_type);
    let new_health_enabled = health_enabled.unwrap_or(current.health_enabled);
    let new_health_interval = health_interval.unwrap_or(current.health_interval);

    // Build new auth config if auth changed
    let auth_config = match new_auth_type.as_str() {
        "bearer" => {
            if let Some(t) = token {
                Some(serde_json::json!({ "type": "bearer", "token": t }))
            } else {
                None
            }
        }
        "api-key" => {
            if let (Some(h), Some(v)) = (api_key_header, api_key_value) {
                Some(serde_json::json!({ "type": "api-key", "header": h, "value": v }))
            } else {
                None
            }
        }
        _ => Some(serde_json::json!({ "type": "none" })),
    };

    let auth_config_str = auth_config.map(|c| serde_json::to_string(&c).unwrap_or_default());

    // Update database
    if let Some(ref config) = auth_config_str {
        conn.execute(
            "UPDATE remote_mcp_servers SET name = ?1, description = ?2, endpoint = ?3, auth_type = ?4, auth_config = ?5, health_enabled = ?6, health_interval = ?7, updated_at = ?8 WHERE id = ?9",
            params![new_name, new_description, new_endpoint, new_auth_type, config, new_health_enabled, new_health_interval as i64, chrono::Utc::now().to_rfc3339(), id],
        ).map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "UPDATE remote_mcp_servers SET name = ?1, description = ?2, endpoint = ?3, auth_type = ?4, health_enabled = ?5, health_interval = ?6, updated_at = ?7 WHERE id = ?8",
            params![new_name, new_description, new_endpoint, new_auth_type, new_health_enabled, new_health_interval as i64, chrono::Utc::now().to_rfc3339(), id],
        ).map_err(|e| e.to_string())?;
    }

    info!("Updated remote MCP server: {}", id);

    // Return updated server
    Ok(RemoteMcpServerInfo {
        id,
        name: new_name,
        description: new_description,
        endpoint: new_endpoint,
        auth_type: new_auth_type,
        status: current.status,
        health_enabled: new_health_enabled,
        health_interval: new_health_interval,
        last_health_check: current.last_health_check,
        latency_ms: current.latency_ms,
        created_at: current.created_at,
        updated_at: chrono::Utc::now().to_rfc3339(),
    })
}
