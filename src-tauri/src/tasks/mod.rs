//! Tasks Module
//!
//! Opcode 2.0 - Parallel tasks and background job management.
//! Handles async task execution, progress tracking, and cancellation.

pub mod manager;
pub mod types;

pub use manager::TaskManager;
pub use types::{Task, TaskStatus, TaskKind, TaskProgress, TaskResult};
