//! Task Types
//!
//! Type definitions for the parallel tasks system.

use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Task status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task is queued
    Pending,
    /// Task is currently running
    Running,
    /// Task completed successfully
    Completed,
    /// Task failed with error
    Failed,
    /// Task was cancelled
    Cancelled,
    /// Task is paused
    Paused,
}

/// Task kind
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    /// Agent execution task
    AgentExecution,
    /// Skill execution task
    SkillExecution,
    /// Shell command task
    Shell,
    /// File operation task
    FileOperation,
    /// MCP tool call
    McpToolCall,
    /// Checkpoint operation
    Checkpoint,
    /// Background sync
    Sync,
    /// Generic async operation
    Async,
}

/// Task priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl Default for TaskPriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Task progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskProgress {
    /// Current step index
    pub current: u64,
    /// Total steps (if known)
    pub total: Option<u64>,
    /// Progress percentage (0-100)
    pub percentage: Option<f32>,
    /// Current step description
    pub message: String,
    /// Detailed status
    pub details: Option<String>,
}

impl Default for TaskProgress {
    fn default() -> Self {
        Self {
            current: 0,
            total: None,
            percentage: None,
            message: "Starting...".to_string(),
            details: None,
        }
    }
}

impl TaskProgress {
    /// Create indeterminate progress
    pub fn indeterminate(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            ..Default::default()
        }
    }

    /// Create progress with known total
    pub fn with_total(current: u64, total: u64, message: impl Into<String>) -> Self {
        let percentage = if total > 0 {
            Some((current as f32 / total as f32) * 100.0)
        } else {
            None
        };

        Self {
            current,
            total: Some(total),
            percentage,
            message: message.into(),
            details: None,
        }
    }

    /// Update progress
    pub fn update(&mut self, current: u64, message: impl Into<String>) {
        self.current = current;
        self.message = message.into();
        if let Some(total) = self.total {
            self.percentage = Some((current as f32 / total as f32) * 100.0);
        }
    }
}

/// Task result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Whether task succeeded
    pub success: bool,
    /// Result data
    pub data: Option<serde_json::Value>,
    /// Error message if failed
    pub error: Option<String>,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Output logs
    pub logs: Option<Vec<String>>,
}

impl TaskResult {
    /// Create success result
    pub fn success(data: Option<serde_json::Value>, duration_ms: u64) -> Self {
        Self {
            success: true,
            data,
            error: None,
            duration_ms,
            logs: None,
        }
    }

    /// Create failure result
    pub fn failure(error: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error.into()),
            duration_ms,
            logs: None,
        }
    }

    /// Add logs
    pub fn with_logs(mut self, logs: Vec<String>) -> Self {
        self.logs = Some(logs);
        self
    }
}

/// Task metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMetadata {
    /// Associated project path
    pub project_path: Option<String>,
    /// Associated session ID
    pub session_id: Option<String>,
    /// Associated agent ID
    pub agent_id: Option<i64>,
    /// Tags for filtering
    pub tags: Vec<String>,
    /// Custom properties
    pub properties: std::collections::HashMap<String, serde_json::Value>,
}

impl Default for TaskMetadata {
    fn default() -> Self {
        Self {
            project_path: None,
            session_id: None,
            agent_id: None,
            tags: vec![],
            properties: std::collections::HashMap::new(),
        }
    }
}

/// Task definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique task ID
    pub id: String,
    /// Task kind
    pub kind: TaskKind,
    /// Task name for display
    pub name: String,
    /// Task description
    pub description: Option<String>,
    /// Current status
    pub status: TaskStatus,
    /// Priority
    pub priority: TaskPriority,
    /// Progress information
    pub progress: TaskProgress,
    /// Result (when completed)
    pub result: Option<TaskResult>,
    /// Task metadata
    pub metadata: TaskMetadata,
    /// Whether task is cancellable
    pub cancellable: bool,
    /// Whether task runs in background
    pub background: bool,
    /// Created timestamp
    pub created_at: String,
    /// Started timestamp
    pub started_at: Option<String>,
    /// Completed timestamp
    pub completed_at: Option<String>,
}

impl Task {
    /// Create a new task
    pub fn new(kind: TaskKind, name: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            name: name.into(),
            description: None,
            status: TaskStatus::Pending,
            priority: TaskPriority::Normal,
            progress: TaskProgress::default(),
            result: None,
            metadata: TaskMetadata::default(),
            cancellable: true,
            background: false,
            created_at: chrono::Utc::now().to_rfc3339(),
            started_at: None,
            completed_at: None,
        }
    }

    /// Set description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set priority
    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set as background task
    pub fn as_background(mut self) -> Self {
        self.background = true;
        self
    }

    /// Set metadata
    pub fn with_metadata(mut self, metadata: TaskMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Mark as started
    pub fn start(&mut self) {
        self.status = TaskStatus::Running;
        self.started_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Mark as completed
    pub fn complete(&mut self, result: TaskResult) {
        self.status = if result.success {
            TaskStatus::Completed
        } else {
            TaskStatus::Failed
        };
        self.result = Some(result);
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Mark as cancelled
    pub fn cancel(&mut self) {
        self.status = TaskStatus::Cancelled;
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Check if task is active
    pub fn is_active(&self) -> bool {
        matches!(self.status, TaskStatus::Pending | TaskStatus::Running)
    }

    /// Check if task is terminal
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        )
    }

    /// Get duration in milliseconds
    pub fn duration_ms(&self) -> Option<u64> {
        let start = self.started_at.as_ref()?;
        let now_str = chrono::Utc::now().to_rfc3339();
        let end = self.completed_at.as_ref().unwrap_or(&now_str);

        let start_dt = chrono::DateTime::parse_from_rfc3339(start).ok()?;
        let end_dt = chrono::DateTime::parse_from_rfc3339(end).ok()?;

        Some((end_dt - start_dt).num_milliseconds() as u64)
    }
}

/// Task info for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInfo {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: String,
    pub progress: TaskProgress,
    pub background: bool,
    pub cancellable: bool,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub duration_ms: Option<u64>,
}

impl From<&Task> for TaskInfo {
    fn from(task: &Task) -> Self {
        Self {
            id: task.id.clone(),
            kind: format!("{:?}", task.kind).to_lowercase(),
            name: task.name.clone(),
            description: task.description.clone(),
            status: format!("{:?}", task.status).to_lowercase(),
            priority: format!("{:?}", task.priority).to_lowercase(),
            progress: task.progress.clone(),
            background: task.background,
            cancellable: task.cancellable,
            created_at: task.created_at.clone(),
            started_at: task.started_at.clone(),
            completed_at: task.completed_at.clone(),
            duration_ms: task.duration_ms(),
        }
    }
}
