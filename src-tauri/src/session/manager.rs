//! Session Manager
//!
//! Lock-free concurrent session management using DashMap.
//! Supports multiple simultaneous Claude sessions with proper isolation.

use dashmap::DashMap;
use log::{debug, error, info, warn};
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Child;
use tokio::sync::oneshot;

use super::events::{SessionEvent, SessionEventEmitter};
use super::state::{SessionInfo, SessionState, SessionStatus};

/// Managed process with kill capability
pub struct ManagedProcess {
    /// Session ID this process belongs to
    pub session_id: String,
    /// The child process handle
    pub child: Child,
    /// Process ID
    pub pid: u32,
    /// Kill switch sender - send to terminate
    pub kill_tx: Option<oneshot::Sender<()>>,
    /// stdout reader task handle
    pub stdout_handle: Option<tokio::task::JoinHandle<()>>,
    /// stderr reader task handle
    pub stderr_handle: Option<tokio::task::JoinHandle<()>>,
}

impl ManagedProcess {
    /// Create a new managed process
    pub fn new(session_id: impl Into<String>, mut child: Child) -> Self {
        let pid = child.id().unwrap_or(0);
        Self {
            session_id: session_id.into(),
            child,
            pid,
            kill_tx: None,
            stdout_handle: None,
            stderr_handle: None,
        }
    }

    /// Set the kill switch
    pub fn with_kill_switch(mut self, kill_tx: oneshot::Sender<()>) -> Self {
        self.kill_tx = Some(kill_tx);
        self
    }

    /// Set stdout reader handle
    pub fn with_stdout_handle(mut self, handle: tokio::task::JoinHandle<()>) -> Self {
        self.stdout_handle = Some(handle);
        self
    }

    /// Set stderr reader handle
    pub fn with_stderr_handle(mut self, handle: tokio::task::JoinHandle<()>) -> Self {
        self.stderr_handle = Some(handle);
        self
    }

    /// Graceful shutdown - try to terminate gracefully, then force kill
    pub async fn shutdown(&mut self, timeout: Duration) -> Result<(), std::io::Error> {
        // First, try sending kill signal through the kill switch
        if let Some(tx) = self.kill_tx.take() {
            let _ = tx.send(());
        }

        // Wait for graceful termination
        let graceful_result = tokio::time::timeout(timeout, self.child.wait()).await;

        match graceful_result {
            Ok(Ok(_)) => {
                debug!("Process {} terminated gracefully", self.pid);
            }
            _ => {
                // Force kill if graceful didn't work
                warn!("Force killing process {}", self.pid);
                self.child.kill().await?;
            }
        }

        // Abort reader tasks
        if let Some(handle) = self.stdout_handle.take() {
            handle.abort();
        }
        if let Some(handle) = self.stderr_handle.take() {
            handle.abort();
        }

        Ok(())
    }

    /// Force kill immediately
    pub async fn kill(&mut self) -> Result<(), std::io::Error> {
        if let Some(tx) = self.kill_tx.take() {
            let _ = tx.send(());
        }
        self.child.kill().await?;

        if let Some(handle) = self.stdout_handle.take() {
            handle.abort();
        }
        if let Some(handle) = self.stderr_handle.take() {
            handle.abort();
        }

        Ok(())
    }
}

/// Session Manager - handles multiple concurrent Claude sessions
pub struct SessionManager {
    /// Active sessions (session_id -> SessionState)
    sessions: Arc<DashMap<String, SessionState>>,
    /// Managed processes (session_id -> ManagedProcess)
    processes: Arc<DashMap<String, ManagedProcess>>,
    /// Maximum concurrent sessions allowed
    max_sessions: usize,
    /// Session timeout in seconds (for cleanup)
    session_timeout_secs: u64,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            processes: Arc::new(DashMap::new()),
            max_sessions: 10,  // Reasonable default
            session_timeout_secs: 3600, // 1 hour
        }
    }

    /// Create with custom limits
    pub fn with_limits(max_sessions: usize, session_timeout_secs: u64) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            processes: Arc::new(DashMap::new()),
            max_sessions,
            session_timeout_secs,
        }
    }

    /// Create a new session
    pub fn create_session(
        &self,
        session_id: impl Into<String>,
        project_path: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<String, SessionError> {
        let session_id = session_id.into();

        // Check if we're at capacity
        if self.sessions.len() >= self.max_sessions {
            // Try to clean up terminated sessions first
            self.cleanup_terminal_sessions();

            if self.sessions.len() >= self.max_sessions {
                return Err(SessionError::MaxSessionsReached(self.max_sessions));
            }
        }

        // Check for duplicate
        if self.sessions.contains_key(&session_id) {
            return Err(SessionError::SessionExists(session_id));
        }

        let state = SessionState::new(&session_id, project_path, model);
        self.sessions.insert(session_id.clone(), state);

        info!("Created session: {}", session_id);
        Ok(session_id)
    }

    /// Get a session by ID
    pub fn get_session(&self, session_id: &str) -> Option<dashmap::mapref::one::Ref<'_, String, SessionState>> {
        self.sessions.get(session_id)
    }

    /// Get mutable access to a session
    pub fn get_session_mut(&self, session_id: &str) -> Option<dashmap::mapref::one::RefMut<'_, String, SessionState>> {
        self.sessions.get_mut(session_id)
    }

    /// Subscribe to session events
    pub fn subscribe(&self, session_id: &str) -> Option<tokio::sync::broadcast::Receiver<SessionEvent>> {
        self.sessions.get(session_id).map(|s| s.subscribe())
    }

    /// Register a managed process for a session
    pub fn register_process(&self, session_id: &str, process: ManagedProcess) -> Result<(), SessionError> {
        let pid = process.pid;

        // Update session state
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.set_running(pid);
        } else {
            return Err(SessionError::SessionNotFound(session_id.to_string()));
        }

        self.processes.insert(session_id.to_string(), process);
        info!("Registered process {} for session {}", pid, session_id);
        Ok(())
    }

    /// Cancel a session - gracefully terminate its process
    pub async fn cancel_session(&self, session_id: &str) -> Result<(), SessionError> {
        info!("Cancelling session: {}", session_id);

        // Update session status first
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.set_status(SessionStatus::Terminating);
        }

        // Shutdown the process
        if let Some((_, mut process)) = self.processes.remove(session_id) {
            if let Err(e) = process.shutdown(Duration::from_secs(5)).await {
                error!("Error shutting down process for session {}: {}", session_id, e);
            }
        }

        // Mark session as cancelled
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.set_cancelled();
        }

        Ok(())
    }

    /// Force kill a session immediately
    pub async fn kill_session(&self, session_id: &str) -> Result<(), SessionError> {
        warn!("Force killing session: {}", session_id);

        if let Some((_, mut process)) = self.processes.remove(session_id) {
            if let Err(e) = process.kill().await {
                error!("Error killing process for session {}: {}", session_id, e);
            }
        }

        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.set_cancelled();
        }

        Ok(())
    }

    /// Mark a session as completed
    pub fn complete_session(&self, session_id: &str) -> Result<(), SessionError> {
        // Remove process entry
        self.processes.remove(session_id);

        // Update session state
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.set_completed();
            Ok(())
        } else {
            Err(SessionError::SessionNotFound(session_id.to_string()))
        }
    }

    /// Mark a session as failed
    pub fn fail_session(&self, session_id: &str, error: impl Into<String>) -> Result<(), SessionError> {
        // Remove process entry
        self.processes.remove(session_id);

        // Update session state
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.set_failed(error);
            Ok(())
        } else {
            Err(SessionError::SessionNotFound(session_id.to_string()))
        }
    }

    /// Get all active sessions
    pub fn list_active_sessions(&self) -> Vec<SessionInfo> {
        self.sessions
            .iter()
            .filter(|s| s.is_active())
            .map(|s| SessionInfo::from(s.value()))
            .collect()
    }

    /// Get all sessions
    pub fn list_all_sessions(&self) -> Vec<SessionInfo> {
        self.sessions
            .iter()
            .map(|s| SessionInfo::from(s.value()))
            .collect()
    }

    /// Clean up terminal (finished) sessions
    pub fn cleanup_terminal_sessions(&self) {
        let terminal_ids: Vec<String> = self.sessions
            .iter()
            .filter(|s| s.is_terminal())
            .map(|s| s.id.clone())
            .collect();

        for id in terminal_ids {
            self.sessions.remove(&id);
            self.processes.remove(&id);
        }
    }

    /// Clean up sessions older than timeout
    pub fn cleanup_stale_sessions(&self) {
        let now = chrono::Utc::now();
        let timeout = chrono::Duration::seconds(self.session_timeout_secs as i64);

        let stale_ids: Vec<String> = self.sessions
            .iter()
            .filter(|s| {
                s.is_terminal() && (now - s.last_activity) > timeout
            })
            .map(|s| s.id.clone())
            .collect();

        for id in stale_ids {
            info!("Cleaning up stale session: {}", id);
            self.sessions.remove(&id);
            self.processes.remove(&id);
        }
    }

    /// Get count of active sessions
    pub fn active_session_count(&self) -> usize {
        self.sessions.iter().filter(|s| s.is_active()).count()
    }

    /// Get total session count
    pub fn total_session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Check if a session exists
    pub fn session_exists(&self, session_id: &str) -> bool {
        self.sessions.contains_key(session_id)
    }

    /// Check if a session is active
    pub fn is_session_active(&self, session_id: &str) -> bool {
        self.sessions.get(session_id).map(|s| s.is_active()).unwrap_or(false)
    }

    /// Get process for a session (if running)
    pub fn get_process(&self, session_id: &str) -> Option<dashmap::mapref::one::Ref<'_, String, ManagedProcess>> {
        self.processes.get(session_id)
    }

    /// Shutdown all sessions - for cleanup on app exit
    pub async fn shutdown_all(&self) {
        info!("Shutting down all sessions...");

        let session_ids: Vec<String> = self.sessions.iter().map(|s| s.id.clone()).collect();

        for session_id in session_ids {
            if let Err(e) = self.cancel_session(&session_id).await {
                error!("Error cancelling session {}: {:?}", session_id, e);
            }
        }

        self.sessions.clear();
        self.processes.clear();

        info!("All sessions shut down");
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Session manager errors
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Session already exists: {0}")]
    SessionExists(String),

    #[error("Maximum sessions reached: {0}")]
    MaxSessionsReached(usize),

    #[error("Session not active: {0}")]
    SessionNotActive(String),

    #[error("Process error: {0}")]
    ProcessError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

impl From<SessionError> for String {
    fn from(err: SessionError) -> String {
        err.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_session() {
        let manager = SessionManager::new();
        let result = manager.create_session("test-1", "/path/to/project", "opus");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test-1");
        assert!(manager.session_exists("test-1"));
    }

    #[test]
    fn test_duplicate_session() {
        let manager = SessionManager::new();
        let _ = manager.create_session("test-1", "/path", "opus");
        let result = manager.create_session("test-1", "/path", "opus");
        assert!(matches!(result, Err(SessionError::SessionExists(_))));
    }

    #[test]
    fn test_max_sessions() {
        let manager = SessionManager::with_limits(2, 3600);
        let _ = manager.create_session("test-1", "/path", "opus");
        let _ = manager.create_session("test-2", "/path", "opus");
        let result = manager.create_session("test-3", "/path", "opus");
        assert!(matches!(result, Err(SessionError::MaxSessionsReached(2))));
    }

    #[test]
    fn test_list_sessions() {
        let manager = SessionManager::new();
        let _ = manager.create_session("test-1", "/path1", "opus");
        let _ = manager.create_session("test-2", "/path2", "sonnet");

        let all = manager.list_all_sessions();
        assert_eq!(all.len(), 2);
    }
}
