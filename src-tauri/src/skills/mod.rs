//! Skills Module
//!
//! Opcode 2.0 - Unified skills system for Claude Code.
//! Skills are reusable, shareable components that extend Claude's capabilities.
//!
//! Features:
//! - Slash commands (user-invocable prompts)
//! - Hooks (pre/post tool execution)
//! - Workflows (multi-step DAG execution)
//! - Templates (reusable patterns)

pub mod types;
pub mod registry;
pub mod loader;
pub mod executor;

pub use types::{Skill, SkillKind, SkillConfig, SkillMetadata};
pub use registry::SkillRegistry;
pub use loader::SkillLoader;
pub use executor::SkillExecutor;
