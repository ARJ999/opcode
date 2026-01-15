//! Session State Management
//!
//! Tracks the state of individual Claude Code sessions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;

use super::events::SessionEvent;

/// Status of a session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    /// Session is being created
    Initializing,
    /// Session is active and running
    Running,
    /// Session is paused/suspended
    Paused,
    /// Session completed successfully
    Completed,
    /// Session was cancelled by user
    Cancelled,
    /// Session failed with an error
    Failed,
    /// Session is being terminated
    Terminating,
}

impl Default for SessionStatus {
    fn default() -> Self {
        Self::Initializing
    }
}

impl std::fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Initializing => write!(f, "initializing"),
            Self::Running => write!(f, "running"),
            Self::Paused => write!(f, "paused"),
            Self::Completed => write!(f, "completed"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Failed => write!(f, "failed"),
            Self::Terminating => write!(f, "terminating"),
        }
    }
}

/// Complete state of a single session
#[derive(Debug)]
pub struct SessionState {
    /// Unique session identifier
    pub id: String,
    /// Project path this session is associated with
    pub project_path: String,
    /// Current status
    pub status: SessionStatus,
    /// Model being used
    pub model: String,
    /// Process ID if running
    pub pid: Option<u32>,
    /// Event broadcaster for this session
    pub event_tx: broadcast::Sender<SessionEvent>,
    /// When the session was created
    pub created_at: DateTime<Utc>,
    /// When the session was last active
    pub last_activity: DateTime<Utc>,
    /// Initial prompt that started the session
    pub initial_prompt: Option<String>,
    /// Error message if failed
    pub error_message: Option<String>,
    /// Token usage tracking
    pub tokens_used: TokenUsage,
    /// Metadata for extensibility
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

/// Token usage tracking
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}

impl SessionState {
    /// Create a new session state
    pub fn new(id: impl Into<String>, project_path: impl Into<String>, model: impl Into<String>) -> Self {
        let (event_tx, _) = broadcast::channel(256);
        let now = Utc::now();

        Self {
            id: id.into(),
            project_path: project_path.into(),
            status: SessionStatus::Initializing,
            model: model.into(),
            pid: None,
            event_tx,
            created_at: now,
            last_activity: now,
            initial_prompt: None,
            error_message: None,
            tokens_used: TokenUsage::default(),
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Subscribe to session events
    pub fn subscribe(&self) -> broadcast::Receiver<SessionEvent> {
        self.event_tx.subscribe()
    }

    /// Emit an event to all subscribers
    pub fn emit(&self, event: SessionEvent) -> Result<usize, broadcast::error::SendError<SessionEvent>> {
        self.event_tx.send(event)
    }

    /// Update status and emit event
    pub fn set_status(&mut self, status: SessionStatus) {
        let old_status = self.status;
        self.status = status;
        self.last_activity = Utc::now();

        let _ = self.emit(SessionEvent::StatusChanged {
            session_id: self.id.clone(),
            old_status,
            new_status: status,
        });
    }

    /// Mark session as running with PID
    pub fn set_running(&mut self, pid: u32) {
        self.pid = Some(pid);
        self.set_status(SessionStatus::Running);
    }

    /// Mark session as completed
    pub fn set_completed(&mut self) {
        self.pid = None;
        self.set_status(SessionStatus::Completed);
    }

    /// Mark session as failed with error
    pub fn set_failed(&mut self, error: impl Into<String>) {
        self.error_message = Some(error.into());
        self.pid = None;
        self.set_status(SessionStatus::Failed);
    }

    /// Mark session as cancelled
    pub fn set_cancelled(&mut self) {
        self.pid = None;
        self.set_status(SessionStatus::Cancelled);
    }

    /// Update token usage
    pub fn add_tokens(&mut self, input: u64, output: u64, cache_read: u64, cache_write: u64) {
        self.tokens_used.input_tokens += input;
        self.tokens_used.output_tokens += output;
        self.tokens_used.cache_read_tokens += cache_read;
        self.tokens_used.cache_write_tokens += cache_write;
        self.last_activity = Utc::now();
    }

    /// Check if session is active (can receive input)
    pub fn is_active(&self) -> bool {
        matches!(self.status, SessionStatus::Running | SessionStatus::Paused)
    }

    /// Check if session is terminal (finished)
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            SessionStatus::Completed | SessionStatus::Cancelled | SessionStatus::Failed
        )
    }

    /// Get session duration in seconds
    pub fn duration_secs(&self) -> i64 {
        (self.last_activity - self.created_at).num_seconds()
    }
}

/// Serializable session info for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub project_path: String,
    pub status: SessionStatus,
    pub model: String,
    pub pid: Option<u32>,
    pub created_at: String,
    pub last_activity: String,
    pub initial_prompt: Option<String>,
    pub error_message: Option<String>,
    pub tokens_used: TokenUsage,
    pub duration_secs: i64,
}

impl From<&SessionState> for SessionInfo {
    fn from(state: &SessionState) -> Self {
        Self {
            id: state.id.clone(),
            project_path: state.project_path.clone(),
            status: state.status,
            model: state.model.clone(),
            pid: state.pid,
            created_at: state.created_at.to_rfc3339(),
            last_activity: state.last_activity.to_rfc3339(),
            initial_prompt: state.initial_prompt.clone(),
            error_message: state.error_message.clone(),
            tokens_used: state.tokens_used.clone(),
            duration_secs: state.duration_secs(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_state_creation() {
        let state = SessionState::new("test-123", "/path/to/project", "opus");
        assert_eq!(state.id, "test-123");
        assert_eq!(state.status, SessionStatus::Initializing);
        assert!(!state.is_active());
        assert!(!state.is_terminal());
    }

    #[test]
    fn test_session_status_transitions() {
        let mut state = SessionState::new("test", "/path", "opus");

        state.set_running(12345);
        assert_eq!(state.status, SessionStatus::Running);
        assert_eq!(state.pid, Some(12345));
        assert!(state.is_active());

        state.set_completed();
        assert_eq!(state.status, SessionStatus::Completed);
        assert!(state.pid.is_none());
        assert!(state.is_terminal());
    }

    #[test]
    fn test_token_tracking() {
        let mut state = SessionState::new("test", "/path", "opus");
        state.add_tokens(100, 50, 20, 10);
        state.add_tokens(100, 50, 0, 0);

        assert_eq!(state.tokens_used.input_tokens, 200);
        assert_eq!(state.tokens_used.output_tokens, 100);
        assert_eq!(state.tokens_used.cache_read_tokens, 20);
    }
}
