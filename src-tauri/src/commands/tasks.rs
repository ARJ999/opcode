//! Task Commands
//!
//! Tauri commands for managing parallel tasks.

use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

use crate::tasks::{TaskManager, TaskEvent, TaskInfo};

/// Task manager state
pub struct TaskManagerState(pub Arc<TaskManager>);

impl Default for TaskManagerState {
    fn default() -> Self {
        Self(Arc::new(TaskManager::new()))
    }
}

/// List all tasks
#[tauri::command]
pub async fn list_tasks(task_manager: State<'_, TaskManagerState>) -> Result<Vec<TaskInfo>, String> {
    Ok(task_manager.0.list_tasks())
}

/// List active tasks
#[tauri::command]
pub async fn list_active_tasks(
    task_manager: State<'_, TaskManagerState>,
) -> Result<Vec<TaskInfo>, String> {
    Ok(task_manager.0.list_active_tasks())
}

/// List background tasks
#[tauri::command]
pub async fn list_background_tasks(
    task_manager: State<'_, TaskManagerState>,
) -> Result<Vec<TaskInfo>, String> {
    Ok(task_manager.0.list_background_tasks())
}

/// Get task by ID
#[tauri::command]
pub async fn get_task(
    task_manager: State<'_, TaskManagerState>,
    id: String,
) -> Result<TaskInfo, String> {
    task_manager
        .0
        .get_task_info(&id)
        .ok_or_else(|| format!("Task not found: {}", id))
}

/// Cancel a task
#[tauri::command]
pub async fn cancel_task(
    task_manager: State<'_, TaskManagerState>,
    id: String,
) -> Result<(), String> {
    task_manager.0.cancel_task(&id)
}

/// Clear completed tasks
#[tauri::command]
pub async fn clear_completed_tasks(
    task_manager: State<'_, TaskManagerState>,
) -> Result<(), String> {
    task_manager.0.clear_completed();
    Ok(())
}

/// Get task count
#[tauri::command]
pub async fn get_task_count(
    task_manager: State<'_, TaskManagerState>,
) -> Result<serde_json::Value, String> {
    let all = task_manager.0.list_tasks();
    let active = task_manager.0.active_count();
    let completed = all.iter().filter(|t| t.status == "completed").count();
    let failed = all.iter().filter(|t| t.status == "failed").count();
    let cancelled = all.iter().filter(|t| t.status == "cancelled").count();

    Ok(serde_json::json!({
        "total": all.len(),
        "active": active,
        "completed": completed,
        "failed": failed,
        "cancelled": cancelled,
    }))
}

/// Subscribe to task events (used internally)
pub fn setup_task_event_emitter(app: AppHandle, task_manager: Arc<TaskManager>) {
    let mut rx = task_manager.subscribe();

    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            let event_name = match &event {
                TaskEvent::Created(_) => "task:created",
                TaskEvent::Started(_) => "task:started",
                TaskEvent::Progress(_, _) => "task:progress",
                TaskEvent::Completed(_, _) => "task:completed",
                TaskEvent::Cancelled(_) => "task:cancelled",
                TaskEvent::Failed(_, _) => "task:failed",
            };

            let payload = match &event {
                TaskEvent::Created(info) => {
                    serde_json::to_value(info).ok()
                }
                TaskEvent::Started(id) => {
                    Some(serde_json::json!({ "id": id }))
                }
                TaskEvent::Progress(id, progress) => {
                    Some(serde_json::json!({ "id": id, "progress": progress }))
                }
                TaskEvent::Completed(id, result) => {
                    Some(serde_json::json!({ "id": id, "result": result }))
                }
                TaskEvent::Cancelled(id) => {
                    Some(serde_json::json!({ "id": id }))
                }
                TaskEvent::Failed(id, error) => {
                    Some(serde_json::json!({ "id": id, "error": error }))
                }
            };

            if let Some(payload) = payload {
                let _ = app.emit(event_name, payload);
            }
        }
    });
}
