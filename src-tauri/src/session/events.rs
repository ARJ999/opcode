//! Session Events
//!
//! Event types and emitter for session-scoped communication.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use super::state::SessionStatus;

/// Events that can be emitted during a session
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SessionEvent {
    /// Session status changed
    StatusChanged {
        session_id: String,
        old_status: SessionStatus,
        new_status: SessionStatus,
    },

    /// Output received from Claude
    Output {
        session_id: String,
        content: String,
        #[serde(rename = "messageType")]
        message_type: OutputType,
    },

    /// Error occurred
    Error {
        session_id: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<String>,
    },

    /// Tool use started
    ToolStart {
        session_id: String,
        tool_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_id: Option<String>,
    },

    /// Tool use completed
    ToolComplete {
        session_id: String,
        tool_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_id: Option<String>,
        success: bool,
    },

    /// Token usage update
    TokenUsage {
        session_id: String,
        input_tokens: u64,
        output_tokens: u64,
        cache_read_tokens: u64,
        cache_write_tokens: u64,
    },

    /// Progress update for long operations
    Progress {
        session_id: String,
        progress: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        total: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },

    /// Thinking/reasoning content (for Ctrl+O mode)
    Thinking {
        session_id: String,
        content: String,
    },

    /// Session completed
    Completed {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
    },

    /// Session cancelled
    Cancelled {
        session_id: String,
    },
}

/// Type of output message
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputType {
    /// Assistant text response
    Assistant,
    /// System message
    System,
    /// User echo
    User,
    /// Tool output
    Tool,
    /// Error output
    Error,
    /// Stderr output
    Stderr,
}

impl SessionEvent {
    /// Get the session ID from any event
    pub fn session_id(&self) -> &str {
        match self {
            Self::StatusChanged { session_id, .. } => session_id,
            Self::Output { session_id, .. } => session_id,
            Self::Error { session_id, .. } => session_id,
            Self::ToolStart { session_id, .. } => session_id,
            Self::ToolComplete { session_id, .. } => session_id,
            Self::TokenUsage { session_id, .. } => session_id,
            Self::Progress { session_id, .. } => session_id,
            Self::Thinking { session_id, .. } => session_id,
            Self::Completed { session_id, .. } => session_id,
            Self::Cancelled { session_id } => session_id,
        }
    }

    /// Get the Tauri event name for this event
    pub fn event_name(&self) -> String {
        let base = match self {
            Self::StatusChanged { .. } => "session-status",
            Self::Output { .. } => "session-output",
            Self::Error { .. } => "session-error",
            Self::ToolStart { .. } => "session-tool-start",
            Self::ToolComplete { .. } => "session-tool-complete",
            Self::TokenUsage { .. } => "session-tokens",
            Self::Progress { .. } => "session-progress",
            Self::Thinking { .. } => "session-thinking",
            Self::Completed { .. } => "session-completed",
            Self::Cancelled { .. } => "session-cancelled",
        };
        format!("{}:{}", base, self.session_id())
    }

    /// Get the global event name (for broadcast)
    pub fn global_event_name(&self) -> &'static str {
        match self {
            Self::StatusChanged { .. } => "session-status",
            Self::Output { .. } => "session-output",
            Self::Error { .. } => "session-error",
            Self::ToolStart { .. } => "session-tool-start",
            Self::ToolComplete { .. } => "session-tool-complete",
            Self::TokenUsage { .. } => "session-tokens",
            Self::Progress { .. } => "session-progress",
            Self::Thinking { .. } => "session-thinking",
            Self::Completed { .. } => "session-completed",
            Self::Cancelled { .. } => "session-cancelled",
        }
    }
}

/// Session event emitter for Tauri
pub struct SessionEventEmitter {
    app_handle: AppHandle,
}

impl SessionEventEmitter {
    /// Create a new event emitter
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }

    /// Emit an event to specific session subscribers
    pub fn emit_to_session(&self, event: &SessionEvent) -> Result<(), tauri::Error> {
        // Emit to session-specific channel
        self.app_handle.emit(&event.event_name(), event)?;

        // Also emit to global channel for monitoring
        self.app_handle.emit(event.global_event_name(), event)?;

        Ok(())
    }

    /// Emit output to a session
    pub fn emit_output(
        &self,
        session_id: &str,
        content: impl Into<String>,
        message_type: OutputType,
    ) -> Result<(), tauri::Error> {
        let event = SessionEvent::Output {
            session_id: session_id.to_string(),
            content: content.into(),
            message_type,
        };
        self.emit_to_session(&event)
    }

    /// Emit error to a session
    pub fn emit_error(
        &self,
        session_id: &str,
        message: impl Into<String>,
        code: Option<String>,
    ) -> Result<(), tauri::Error> {
        let event = SessionEvent::Error {
            session_id: session_id.to_string(),
            message: message.into(),
            code,
        };
        self.emit_to_session(&event)
    }

    /// Emit stderr output to a session
    pub fn emit_stderr(&self, session_id: &str, content: impl Into<String>) -> Result<(), tauri::Error> {
        self.emit_output(session_id, content, OutputType::Stderr)
    }

    /// Emit completion event
    pub fn emit_completed(&self, session_id: &str, summary: Option<String>) -> Result<(), tauri::Error> {
        let event = SessionEvent::Completed {
            session_id: session_id.to_string(),
            summary,
        };
        self.emit_to_session(&event)
    }

    /// Emit cancellation event
    pub fn emit_cancelled(&self, session_id: &str) -> Result<(), tauri::Error> {
        let event = SessionEvent::Cancelled {
            session_id: session_id.to_string(),
        };
        self.emit_to_session(&event)
    }

    /// Emit thinking content (for Ctrl+O mode)
    pub fn emit_thinking(&self, session_id: &str, content: impl Into<String>) -> Result<(), tauri::Error> {
        let event = SessionEvent::Thinking {
            session_id: session_id.to_string(),
            content: content.into(),
        };
        self.emit_to_session(&event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_names() {
        let event = SessionEvent::Output {
            session_id: "test-123".to_string(),
            content: "Hello".to_string(),
            message_type: OutputType::Assistant,
        };

        assert_eq!(event.event_name(), "session-output:test-123");
        assert_eq!(event.global_event_name(), "session-output");
        assert_eq!(event.session_id(), "test-123");
    }
}
