//! Task Manager
//!
//! Concurrent task management with DashMap.
//! Handles task lifecycle, progress tracking, and cancellation.

use dashmap::DashMap;
use log::{debug, error, info, warn};
use std::sync::Arc;
use tokio::sync::{broadcast, oneshot};

use super::types::{Task, TaskInfo, TaskKind, TaskPriority, TaskProgress, TaskResult, TaskStatus};

/// Task handle for controlling running tasks
pub struct TaskHandle {
    /// Task ID
    pub task_id: String,
    /// Cancel signal sender
    cancel_tx: Option<oneshot::Sender<()>>,
    /// Join handle for the task
    join_handle: Option<tokio::task::JoinHandle<TaskResult>>,
}

impl TaskHandle {
    /// Create a new task handle
    pub fn new(task_id: String) -> Self {
        Self {
            task_id,
            cancel_tx: None,
            join_handle: None,
        }
    }

    /// Set cancel signal
    pub fn with_cancel(mut self, tx: oneshot::Sender<()>) -> Self {
        self.cancel_tx = Some(tx);
        self
    }

    /// Set join handle
    pub fn with_handle(mut self, handle: tokio::task::JoinHandle<TaskResult>) -> Self {
        self.join_handle = Some(handle);
        self
    }

    /// Cancel the task
    pub fn cancel(&mut self) -> bool {
        if let Some(tx) = self.cancel_tx.take() {
            tx.send(()).is_ok()
        } else {
            false
        }
    }

    /// Abort the task immediately
    pub fn abort(&mut self) {
        if let Some(handle) = self.join_handle.take() {
            handle.abort();
        }
    }
}

/// Task event for broadcasts
#[derive(Debug, Clone)]
pub enum TaskEvent {
    /// Task created
    Created(TaskInfo),
    /// Task started
    Started(String),
    /// Task progress updated
    Progress(String, TaskProgress),
    /// Task completed
    Completed(String, TaskResult),
    /// Task cancelled
    Cancelled(String),
    /// Task failed
    Failed(String, String),
}

/// Task manager for handling concurrent tasks
pub struct TaskManager {
    /// Active tasks (task_id -> Task)
    tasks: Arc<DashMap<String, Task>>,
    /// Task handles (task_id -> TaskHandle)
    handles: Arc<DashMap<String, TaskHandle>>,
    /// Event broadcaster
    event_tx: broadcast::Sender<TaskEvent>,
    /// Maximum concurrent tasks
    max_concurrent: usize,
    /// Maximum history size
    max_history: usize,
}

impl TaskManager {
    /// Create a new task manager
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(256);

        Self {
            tasks: Arc::new(DashMap::new()),
            handles: Arc::new(DashMap::new()),
            event_tx,
            max_concurrent: 10,
            max_history: 100,
        }
    }

    /// Create with custom limits
    pub fn with_limits(max_concurrent: usize, max_history: usize) -> Self {
        let (event_tx, _) = broadcast::channel(256);

        Self {
            tasks: Arc::new(DashMap::new()),
            handles: Arc::new(DashMap::new()),
            event_tx,
            max_concurrent,
            max_history,
        }
    }

    /// Subscribe to task events
    pub fn subscribe(&self) -> broadcast::Receiver<TaskEvent> {
        self.event_tx.subscribe()
    }

    /// Create and register a new task
    pub fn create_task(&self, kind: TaskKind, name: impl Into<String>) -> Task {
        let task = Task::new(kind, name);
        let info = TaskInfo::from(&task);

        self.tasks.insert(task.id.clone(), task.clone());
        let _ = self.event_tx.send(TaskEvent::Created(info));

        debug!("Created task: {} ({})", task.name, task.id);
        task
    }

    /// Register a task handle
    pub fn register_handle(&self, task_id: &str, handle: TaskHandle) {
        self.handles.insert(task_id.to_string(), handle);
    }

    /// Start a task
    pub fn start_task(&self, task_id: &str) -> Result<(), String> {
        if let Some(mut task) = self.tasks.get_mut(task_id) {
            task.start();
            let _ = self.event_tx.send(TaskEvent::Started(task_id.to_string()));
            info!("Started task: {} ({})", task.name, task.id);
            Ok(())
        } else {
            Err(format!("Task not found: {}", task_id))
        }
    }

    /// Update task progress
    pub fn update_progress(&self, task_id: &str, progress: TaskProgress) {
        if let Some(mut task) = self.tasks.get_mut(task_id) {
            task.progress = progress.clone();
            let _ = self.event_tx.send(TaskEvent::Progress(task_id.to_string(), progress));
        }
    }

    /// Complete a task
    pub fn complete_task(&self, task_id: &str, result: TaskResult) {
        if let Some(mut task) = self.tasks.get_mut(task_id) {
            let success = result.success;
            task.complete(result.clone());

            if success {
                info!("Completed task: {} ({})", task.name, task.id);
                let _ = self.event_tx.send(TaskEvent::Completed(task_id.to_string(), result));
            } else {
                warn!("Task failed: {} ({})", task.name, task.id);
                let _ = self.event_tx.send(TaskEvent::Failed(
                    task_id.to_string(),
                    result.error.clone().unwrap_or_default(),
                ));
            }

            // Remove handle
            self.handles.remove(task_id);
        }

        // Clean up old completed tasks
        self.cleanup_history();
    }

    /// Cancel a task
    pub fn cancel_task(&self, task_id: &str) -> Result<(), String> {
        // Try to send cancel signal
        if let Some(mut handle) = self.handles.get_mut(task_id) {
            if handle.cancel() {
                debug!("Sent cancel signal to task: {}", task_id);
            }
        }

        // Update task status
        if let Some(mut task) = self.tasks.get_mut(task_id) {
            if !task.cancellable {
                return Err("Task is not cancellable".to_string());
            }

            task.cancel();
            info!("Cancelled task: {} ({})", task.name, task.id);
            let _ = self.event_tx.send(TaskEvent::Cancelled(task_id.to_string()));
        } else {
            return Err(format!("Task not found: {}", task_id));
        }

        // Remove handle
        self.handles.remove(task_id);

        Ok(())
    }

    /// Abort a task immediately
    pub fn abort_task(&self, task_id: &str) -> Result<(), String> {
        if let Some(mut handle) = self.handles.get_mut(task_id) {
            handle.abort();
        }

        self.cancel_task(task_id)
    }

    /// Get a task by ID
    pub fn get_task(&self, task_id: &str) -> Option<Task> {
        self.tasks.get(task_id).map(|t| t.clone())
    }

    /// Get task info for frontend
    pub fn get_task_info(&self, task_id: &str) -> Option<TaskInfo> {
        self.tasks.get(task_id).map(|t| TaskInfo::from(t.value()))
    }

    /// List all tasks
    pub fn list_tasks(&self) -> Vec<TaskInfo> {
        self.tasks.iter().map(|t| TaskInfo::from(t.value())).collect()
    }

    /// List active tasks
    pub fn list_active_tasks(&self) -> Vec<TaskInfo> {
        self.tasks
            .iter()
            .filter(|t| t.is_active())
            .map(|t| TaskInfo::from(t.value()))
            .collect()
    }

    /// List background tasks
    pub fn list_background_tasks(&self) -> Vec<TaskInfo> {
        self.tasks
            .iter()
            .filter(|t| t.background && t.is_active())
            .map(|t| TaskInfo::from(t.value()))
            .collect()
    }

    /// List completed tasks
    pub fn list_completed_tasks(&self) -> Vec<TaskInfo> {
        self.tasks
            .iter()
            .filter(|t| t.is_terminal())
            .map(|t| TaskInfo::from(t.value()))
            .collect()
    }

    /// Count active tasks
    pub fn active_count(&self) -> usize {
        self.tasks.iter().filter(|t| t.is_active()).count()
    }

    /// Check if can start new task
    pub fn can_start_task(&self) -> bool {
        self.active_count() < self.max_concurrent
    }

    /// Clean up old completed tasks
    pub fn cleanup_history(&self) {
        let mut completed: Vec<(String, String)> = self
            .tasks
            .iter()
            .filter(|t| t.is_terminal())
            .map(|t| (t.id.clone(), t.completed_at.clone().unwrap_or_default()))
            .collect();

        // Sort by completion time (oldest first)
        completed.sort_by(|a, b| a.1.cmp(&b.1));

        // Remove excess tasks
        let excess = completed.len().saturating_sub(self.max_history);
        for (task_id, _) in completed.into_iter().take(excess) {
            self.tasks.remove(&task_id);
            self.handles.remove(&task_id);
        }
    }

    /// Clear all completed tasks
    pub fn clear_completed(&self) {
        let to_remove: Vec<String> = self
            .tasks
            .iter()
            .filter(|t| t.is_terminal())
            .map(|t| t.id.clone())
            .collect();

        for task_id in to_remove {
            self.tasks.remove(&task_id);
            self.handles.remove(&task_id);
        }
    }

    /// Cancel all active tasks
    pub async fn cancel_all(&self) {
        let active: Vec<String> = self
            .tasks
            .iter()
            .filter(|t| t.is_active())
            .map(|t| t.id.clone())
            .collect();

        for task_id in active {
            let _ = self.cancel_task(&task_id);
        }
    }
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_task() {
        let manager = TaskManager::new();
        let task = manager.create_task(TaskKind::Shell, "Test task");
        assert!(!task.id.is_empty());
        assert_eq!(task.name, "Test task");
        assert_eq!(task.status, TaskStatus::Pending);
    }

    #[test]
    fn test_task_lifecycle() {
        let manager = TaskManager::new();
        let task = manager.create_task(TaskKind::Shell, "Test task");
        let task_id = task.id.clone();

        manager.start_task(&task_id).unwrap();

        let task = manager.get_task(&task_id).unwrap();
        assert_eq!(task.status, TaskStatus::Running);

        manager.complete_task(&task_id, TaskResult::success(None, 100));

        let task = manager.get_task(&task_id).unwrap();
        assert_eq!(task.status, TaskStatus::Completed);
    }

    #[test]
    fn test_cancel_task() {
        let manager = TaskManager::new();
        let task = manager.create_task(TaskKind::Shell, "Test task");
        let task_id = task.id.clone();

        manager.start_task(&task_id).unwrap();
        manager.cancel_task(&task_id).unwrap();

        let task = manager.get_task(&task_id).unwrap();
        assert_eq!(task.status, TaskStatus::Cancelled);
    }
}
