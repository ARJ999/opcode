//! Skill Types
//!
//! Type definitions for the unified skills system.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Kind of skill
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillKind {
    /// Slash command - user-invocable prompt
    SlashCommand,
    /// Hook - triggers before/after tool execution
    Hook,
    /// Workflow - multi-step execution graph
    Workflow,
    /// Template - reusable prompt pattern
    Template,
    /// Agent - specialized agent configuration
    Agent,
}

/// Skill visibility
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillVisibility {
    /// Available in all projects
    Global,
    /// Available only in specific project
    Project,
    /// Available only in specific workspace
    Workspace,
}

/// Hook trigger type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookTrigger {
    /// Before tool execution
    PreTool,
    /// After tool execution
    PostTool,
    /// On session start
    SessionStart,
    /// On session end
    SessionEnd,
    /// On checkpoint create
    CheckpointCreate,
    /// On error
    OnError,
}

/// Workflow step type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStepKind {
    /// Execute a prompt
    Prompt,
    /// Run a tool
    Tool,
    /// Run a shell command
    Shell,
    /// Conditional branch
    Condition,
    /// Parallel execution
    Parallel,
    /// Wait for user input
    UserInput,
    /// Reference another skill
    SkillRef,
}

/// Workflow step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    /// Unique step ID
    pub id: String,
    /// Step kind
    pub kind: WorkflowStepKind,
    /// Step name for display
    pub name: String,
    /// Step configuration (kind-specific)
    pub config: serde_json::Value,
    /// IDs of steps this step depends on
    pub depends_on: Vec<String>,
    /// Optional condition expression
    pub condition: Option<String>,
    /// Timeout in seconds
    pub timeout_secs: Option<u64>,
    /// Retry configuration
    pub retry: Option<RetryConfig>,
}

/// Retry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum retry attempts
    pub max_attempts: u32,
    /// Backoff strategy
    pub backoff: BackoffStrategy,
    /// Retry on these error patterns
    pub retry_on: Vec<String>,
}

/// Backoff strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackoffStrategy {
    /// Fixed delay between retries
    Fixed { delay_ms: u64 },
    /// Exponential backoff
    Exponential { initial_ms: u64, max_ms: u64, multiplier: f64 },
    /// Linear backoff
    Linear { initial_ms: u64, increment_ms: u64 },
}

/// Skill metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    /// Author name or ID
    pub author: Option<String>,
    /// Version string (semver)
    pub version: String,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// License identifier
    pub license: Option<String>,
    /// Repository URL
    pub repository: Option<String>,
    /// Homepage URL
    pub homepage: Option<String>,
    /// Minimum Opcode version required
    pub min_opcode_version: Option<String>,
    /// Dependencies on other skills
    pub dependencies: Vec<SkillDependency>,
}

/// Skill dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDependency {
    /// Skill ID
    pub skill_id: String,
    /// Version requirement (semver range)
    pub version: Option<String>,
    /// Whether this dependency is optional
    pub optional: bool,
}

/// Skill configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillConfig {
    /// Slash command configuration (for SlashCommand kind)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slash_command: Option<SlashCommandConfig>,
    /// Hook configuration (for Hook kind)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook: Option<HookConfig>,
    /// Workflow configuration (for Workflow kind)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow: Option<WorkflowConfig>,
    /// Template configuration (for Template kind)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<TemplateConfig>,
    /// Agent configuration (for Agent kind)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentConfig>,
}

/// Slash command configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashCommandConfig {
    /// Command name (without leading slash)
    pub name: String,
    /// Short description for help
    pub description: String,
    /// Extended help text
    pub help: Option<String>,
    /// Prompt template with $ARGUMENTS placeholder
    pub prompt: String,
    /// Whether to require arguments
    pub requires_args: bool,
    /// Argument parser configuration
    pub args: Option<ArgsConfig>,
    /// Example usages
    pub examples: Vec<String>,
}

/// Argument parser configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgsConfig {
    /// Positional arguments
    pub positional: Vec<ArgDef>,
    /// Named flags/options
    pub named: Vec<ArgDef>,
}

/// Argument definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgDef {
    /// Argument name
    pub name: String,
    /// Description
    pub description: String,
    /// Whether required
    pub required: bool,
    /// Default value
    pub default: Option<String>,
    /// Valid values (for enum-like args)
    pub choices: Option<Vec<String>>,
}

/// Hook configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookConfig {
    /// When to trigger
    pub trigger: HookTrigger,
    /// Tool patterns to match (for PreTool/PostTool)
    pub tool_patterns: Option<Vec<String>>,
    /// Command to execute
    pub command: String,
    /// Timeout in seconds
    pub timeout_secs: u64,
    /// Whether hook can block/modify execution
    pub can_block: bool,
    /// Environment variables
    pub env: HashMap<String, String>,
}

/// Workflow configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowConfig {
    /// Workflow steps (DAG)
    pub steps: Vec<WorkflowStep>,
    /// Input variables schema
    pub inputs: Vec<InputDef>,
    /// Output variable mapping
    pub outputs: HashMap<String, String>,
    /// Global timeout
    pub timeout_secs: Option<u64>,
    /// Concurrency limit
    pub max_parallel: Option<u32>,
}

/// Input variable definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputDef {
    /// Variable name
    pub name: String,
    /// Description
    pub description: String,
    /// Variable type (string, number, boolean, json)
    pub var_type: String,
    /// Whether required
    pub required: bool,
    /// Default value
    pub default: Option<serde_json::Value>,
}

/// Template configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateConfig {
    /// Template content with placeholders
    pub content: String,
    /// Variables that can be substituted
    pub variables: Vec<TemplateVariable>,
}

/// Template variable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVariable {
    /// Variable name (matches {{name}} in template)
    pub name: String,
    /// Description
    pub description: String,
    /// Default value
    pub default: Option<String>,
}

/// Agent configuration (for skill-based agents)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Agent name
    pub name: String,
    /// System prompt
    pub system_prompt: String,
    /// Model to use
    pub model: String,
    /// Permission mode
    pub permission_mode: String,
    /// Allowed tools
    pub allowed_tools: Vec<String>,
    /// Denied tools
    pub denied_tools: Vec<String>,
    /// MCP servers to use
    pub mcp_servers: Vec<String>,
    /// Max conversation turns
    pub max_turns: Option<u32>,
}

/// Skill definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    /// Unique skill ID
    pub id: String,
    /// Skill kind
    pub kind: SkillKind,
    /// Display name
    pub name: String,
    /// Description
    pub description: String,
    /// Visibility scope
    pub visibility: SkillVisibility,
    /// Whether enabled
    pub enabled: bool,
    /// Skill configuration
    pub config: SkillConfig,
    /// Metadata
    pub metadata: SkillMetadata,
    /// Project path (for project-scoped skills)
    pub project_path: Option<String>,
    /// Source (local, github, registry)
    pub source: String,
    /// Created timestamp
    pub created_at: String,
    /// Updated timestamp
    pub updated_at: String,
}

impl Default for SkillMetadata {
    fn default() -> Self {
        Self {
            author: None,
            version: "1.0.0".to_string(),
            tags: vec![],
            license: None,
            repository: None,
            homepage: None,
            min_opcode_version: None,
            dependencies: vec![],
        }
    }
}

impl Default for SkillConfig {
    fn default() -> Self {
        Self {
            slash_command: None,
            hook: None,
            workflow: None,
            template: None,
            agent: None,
        }
    }
}

/// Skill execution context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillContext {
    /// Current project path
    pub project_path: String,
    /// Current session ID
    pub session_id: Option<String>,
    /// Input arguments
    pub arguments: HashMap<String, serde_json::Value>,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// User-defined variables
    pub variables: HashMap<String, serde_json::Value>,
}

/// Skill execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillResult {
    /// Whether execution succeeded
    pub success: bool,
    /// Output data
    pub output: Option<serde_json::Value>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Step results (for workflows)
    pub steps: Option<Vec<StepResult>>,
}

/// Workflow step result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    /// Step ID
    pub step_id: String,
    /// Step name
    pub step_name: String,
    /// Whether step succeeded
    pub success: bool,
    /// Step output
    pub output: Option<serde_json::Value>,
    /// Error message
    pub error: Option<String>,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Retry attempts used
    pub retries: u32,
}
