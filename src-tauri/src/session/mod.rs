//! Session Management Module
//!
//! Opcode 2.0 - Production-grade session management with:
//! - Lock-free concurrent session handling (DashMap)
//! - Session-scoped event isolation
//! - Proper process lifecycle management
//! - Windows/macOS/Linux compatibility
//!
//! This replaces the old single-session ClaudeProcessState approach.

pub mod manager;
pub mod state;
pub mod events;

pub use manager::SessionManager;
pub use state::{SessionState, SessionStatus};
pub use events::{SessionEvent, SessionEventEmitter};
