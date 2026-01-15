//! MCP Health Monitoring Service
//!
//! Provides continuous health monitoring for remote MCP servers.
//! Features:
//! - Periodic health checks with configurable intervals
//! - Latency tracking
//! - Automatic reconnection attempts
//! - Health status events for UI updates

use dashmap::DashMap;
use log::{debug, error, info, warn};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio::time::interval;

use super::error::{McpError, McpResult};
use super::transport::McpTransport;
use super::types::RemoteMcpServer;

/// Health status levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Server is responding normally
    Healthy,
    /// Server is responding but with degraded performance
    Degraded,
    /// Server is not responding
    Unhealthy,
    /// Health status is unknown (never checked or checking in progress)
    Unknown,
}

impl Default for HealthStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Health information for a server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerHealth {
    pub server_id: String,
    pub status: HealthStatus,
    /// Latency in milliseconds
    pub latency_ms: Option<u64>,
    /// Last successful health check timestamp (RFC3339)
    pub last_check: Option<String>,
    /// Last error message if unhealthy
    pub last_error: Option<String>,
    /// Number of consecutive failures
    pub consecutive_failures: u32,
    /// Number of consecutive successes
    pub consecutive_successes: u32,
    /// Average latency over last N checks
    pub avg_latency_ms: Option<u64>,
}

impl ServerHealth {
    pub fn new(server_id: impl Into<String>) -> Self {
        Self {
            server_id: server_id.into(),
            status: HealthStatus::Unknown,
            latency_ms: None,
            last_check: None,
            last_error: None,
            consecutive_failures: 0,
            consecutive_successes: 0,
            avg_latency_ms: None,
        }
    }

    fn record_success(&mut self, latency_ms: u64) {
        self.status = HealthStatus::Healthy;
        self.latency_ms = Some(latency_ms);
        self.last_check = Some(chrono::Utc::now().to_rfc3339());
        self.last_error = None;
        self.consecutive_failures = 0;
        self.consecutive_successes += 1;

        // Update average latency (simple moving average)
        if let Some(avg) = self.avg_latency_ms {
            self.avg_latency_ms = Some((avg * 4 + latency_ms) / 5);
        } else {
            self.avg_latency_ms = Some(latency_ms);
        }

        // Check for degraded performance (latency > 2x average)
        if let Some(avg) = self.avg_latency_ms {
            if latency_ms > avg * 2 && latency_ms > 1000 {
                self.status = HealthStatus::Degraded;
            }
        }
    }

    fn record_failure(&mut self, error: impl Into<String>) {
        self.status = HealthStatus::Unhealthy;
        self.latency_ms = None;
        self.last_check = Some(chrono::Utc::now().to_rfc3339());
        self.last_error = Some(error.into());
        self.consecutive_failures += 1;
        self.consecutive_successes = 0;
    }
}

/// Health check event types
#[derive(Debug, Clone)]
pub enum HealthEvent {
    /// Health status changed for a server
    StatusChanged {
        server_id: String,
        old_status: HealthStatus,
        new_status: HealthStatus,
    },
    /// Health check completed
    CheckCompleted {
        server_id: String,
        health: ServerHealth,
    },
    /// Server became unreachable after N consecutive failures
    ServerUnreachable {
        server_id: String,
        consecutive_failures: u32,
    },
    /// Server recovered after being unreachable
    ServerRecovered {
        server_id: String,
    },
}

/// MCP Health Monitor
pub struct McpHealthMonitor {
    /// Server health status map
    health_status: Arc<DashMap<String, ServerHealth>>,
    /// Event broadcaster
    event_tx: broadcast::Sender<HealthEvent>,
    /// Whether the monitor is running
    running: Arc<RwLock<bool>>,
    /// Default check interval in seconds
    default_interval_secs: u64,
    /// Default timeout in seconds
    default_timeout_secs: u64,
    /// Threshold for marking server as unreachable
    unreachable_threshold: u32,
}

impl McpHealthMonitor {
    /// Create a new health monitor
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self {
            health_status: Arc::new(DashMap::new()),
            event_tx,
            running: Arc::new(RwLock::new(false)),
            default_interval_secs: 60,
            default_timeout_secs: 10,
            unreachable_threshold: 3,
        }
    }

    /// Create with custom settings
    pub fn with_settings(
        default_interval_secs: u64,
        default_timeout_secs: u64,
        unreachable_threshold: u32,
    ) -> Self {
        let mut monitor = Self::new();
        monitor.default_interval_secs = default_interval_secs;
        monitor.default_timeout_secs = default_timeout_secs;
        monitor.unreachable_threshold = unreachable_threshold;
        monitor
    }

    /// Subscribe to health events
    pub fn subscribe(&self) -> broadcast::Receiver<HealthEvent> {
        self.event_tx.subscribe()
    }

    /// Get health status for a server
    pub fn get_health(&self, server_id: &str) -> Option<ServerHealth> {
        self.health_status.get(server_id).map(|h| h.clone())
    }

    /// Get all health statuses
    pub fn get_all_health(&self) -> Vec<ServerHealth> {
        self.health_status.iter().map(|r| r.clone()).collect()
    }

    /// Check health of a specific server using transport
    pub async fn check_server_health(
        &self,
        server_id: &str,
        transport: &dyn McpTransport,
    ) -> McpResult<ServerHealth> {
        let start = Instant::now();

        // Get or create health entry
        let mut health = self
            .health_status
            .entry(server_id.to_string())
            .or_insert_with(|| ServerHealth::new(server_id))
            .clone();

        let old_status = health.status;

        // Perform ping
        match transport.ping().await {
            Ok(()) => {
                let latency = start.elapsed().as_millis() as u64;
                health.record_success(latency);

                // Check for recovery
                if old_status == HealthStatus::Unhealthy && health.status == HealthStatus::Healthy {
                    let _ = self.event_tx.send(HealthEvent::ServerRecovered {
                        server_id: server_id.to_string(),
                    });
                }

                debug!(
                    "Health check passed for {}: {}ms (status: {:?})",
                    server_id, latency, health.status
                );
            }
            Err(e) => {
                health.record_failure(e.to_string());

                // Check for unreachable threshold
                if health.consecutive_failures >= self.unreachable_threshold {
                    let _ = self.event_tx.send(HealthEvent::ServerUnreachable {
                        server_id: server_id.to_string(),
                        consecutive_failures: health.consecutive_failures,
                    });
                }

                warn!(
                    "Health check failed for {}: {} (failures: {})",
                    server_id, e, health.consecutive_failures
                );
            }
        }

        // Emit status changed event if status changed
        if old_status != health.status {
            let _ = self.event_tx.send(HealthEvent::StatusChanged {
                server_id: server_id.to_string(),
                old_status,
                new_status: health.status,
            });
        }

        // Emit check completed event
        let _ = self.event_tx.send(HealthEvent::CheckCompleted {
            server_id: server_id.to_string(),
            health: health.clone(),
        });

        // Update stored health
        self.health_status.insert(server_id.to_string(), health.clone());

        Ok(health)
    }

    /// Check health using HTTP endpoint directly (for servers not yet connected)
    pub async fn check_endpoint_health(
        &self,
        server_id: &str,
        endpoint: &str,
        timeout_secs: Option<u64>,
    ) -> McpResult<ServerHealth> {
        let timeout = timeout_secs.unwrap_or(self.default_timeout_secs);
        let start = Instant::now();

        // Get or create health entry
        let mut health = self
            .health_status
            .entry(server_id.to_string())
            .or_insert_with(|| ServerHealth::new(server_id))
            .clone();

        let old_status = health.status;

        // Try to reach the endpoint
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout))
            .build()
            .map_err(|e| McpError::TransportError(e.to_string()))?;

        match client.head(endpoint).send().await {
            Ok(response) => {
                let latency = start.elapsed().as_millis() as u64;

                if response.status().is_success() || response.status().as_u16() == 405 {
                    // 405 Method Not Allowed is OK - endpoint exists but doesn't support HEAD
                    health.record_success(latency);
                } else if response.status().is_server_error() {
                    health.record_failure(format!("Server error: {}", response.status()));
                } else {
                    // Other status codes - mark as degraded
                    health.record_success(latency);
                    health.status = HealthStatus::Degraded;
                }
            }
            Err(e) => {
                health.record_failure(e.to_string());
            }
        }

        // Emit events
        if old_status != health.status {
            let _ = self.event_tx.send(HealthEvent::StatusChanged {
                server_id: server_id.to_string(),
                old_status,
                new_status: health.status,
            });
        }

        let _ = self.event_tx.send(HealthEvent::CheckCompleted {
            server_id: server_id.to_string(),
            health: health.clone(),
        });

        // Update stored health
        self.health_status.insert(server_id.to_string(), health.clone());

        Ok(health)
    }

    /// Start periodic health monitoring for all registered servers
    pub fn start_monitoring(&self, servers: Vec<(String, String)>) -> tokio::task::JoinHandle<()> {
        let health_status = self.health_status.clone();
        let event_tx = self.event_tx.clone();
        let running = self.running.clone();
        let interval_secs = self.default_interval_secs;
        let timeout_secs = self.default_timeout_secs;
        let unreachable_threshold = self.unreachable_threshold;

        *running.write() = true;

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(interval_secs));

            while *running.read() {
                ticker.tick().await;

                for (server_id, endpoint) in &servers {
                    if !*running.read() {
                        break;
                    }

                    let start = Instant::now();

                    let mut health = health_status
                        .entry(server_id.clone())
                        .or_insert_with(|| ServerHealth::new(server_id))
                        .clone();

                    let old_status = health.status;

                    // Perform health check
                    let client = match reqwest::Client::builder()
                        .timeout(Duration::from_secs(timeout_secs))
                        .build()
                    {
                        Ok(c) => c,
                        Err(e) => {
                            error!("Failed to create HTTP client: {}", e);
                            continue;
                        }
                    };

                    match client.head(endpoint).send().await {
                        Ok(response) => {
                            let latency = start.elapsed().as_millis() as u64;
                            if response.status().is_success() || response.status().as_u16() == 405 {
                                health.record_success(latency);
                                if old_status == HealthStatus::Unhealthy {
                                    let _ = event_tx.send(HealthEvent::ServerRecovered {
                                        server_id: server_id.clone(),
                                    });
                                }
                            } else {
                                health.record_failure(format!("HTTP {}", response.status()));
                            }
                        }
                        Err(e) => {
                            health.record_failure(e.to_string());
                            if health.consecutive_failures >= unreachable_threshold {
                                let _ = event_tx.send(HealthEvent::ServerUnreachable {
                                    server_id: server_id.clone(),
                                    consecutive_failures: health.consecutive_failures,
                                });
                            }
                        }
                    }

                    if old_status != health.status {
                        let _ = event_tx.send(HealthEvent::StatusChanged {
                            server_id: server_id.clone(),
                            old_status,
                            new_status: health.status,
                        });
                    }

                    let _ = event_tx.send(HealthEvent::CheckCompleted {
                        server_id: server_id.clone(),
                        health: health.clone(),
                    });

                    health_status.insert(server_id.clone(), health);
                }
            }

            info!("Health monitoring stopped");
        })
    }

    /// Stop health monitoring
    pub fn stop_monitoring(&self) {
        *self.running.write() = false;
    }

    /// Check if monitoring is running
    pub fn is_running(&self) -> bool {
        *self.running.read()
    }
}

impl Default for McpHealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_health_success() {
        let mut health = ServerHealth::new("test");
        health.record_success(100);

        assert_eq!(health.status, HealthStatus::Healthy);
        assert_eq!(health.latency_ms, Some(100));
        assert_eq!(health.consecutive_failures, 0);
        assert_eq!(health.consecutive_successes, 1);
    }

    #[test]
    fn test_server_health_failure() {
        let mut health = ServerHealth::new("test");
        health.record_failure("Connection refused");

        assert_eq!(health.status, HealthStatus::Unhealthy);
        assert_eq!(health.latency_ms, None);
        assert_eq!(health.consecutive_failures, 1);
        assert_eq!(health.consecutive_successes, 0);
        assert!(health.last_error.is_some());
    }

    #[test]
    fn test_degraded_detection() {
        let mut health = ServerHealth::new("test");

        // Build up an average
        for _ in 0..5 {
            health.record_success(100);
        }

        // Spike in latency should trigger degraded
        health.record_success(5000);

        assert_eq!(health.status, HealthStatus::Degraded);
    }
}
