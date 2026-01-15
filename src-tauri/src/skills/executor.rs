//! Skill Executor
//!
//! Execute skills including slash commands, hooks, and workflows.

use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::broadcast;

use super::registry::SkillRegistry;
use super::types::{
    HookConfig, HookTrigger, Skill, SkillConfig, SkillContext, SkillKind, SkillResult,
    SlashCommandConfig, StepResult, WorkflowConfig, WorkflowStep, WorkflowStepKind,
};

/// Skill executor for running skills
pub struct SkillExecutor {
    /// Reference to the skill registry
    registry: std::sync::Arc<SkillRegistry>,
    /// Default timeout for skill execution (seconds)
    default_timeout_secs: u64,
}

impl SkillExecutor {
    /// Create a new skill executor
    pub fn new(registry: std::sync::Arc<SkillRegistry>) -> Self {
        Self {
            registry,
            default_timeout_secs: 300, // 5 minutes
        }
    }

    /// Set default timeout
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.default_timeout_secs = timeout_secs;
        self
    }

    /// Execute a skill by ID
    pub async fn execute(&self, skill_id: &str, context: SkillContext) -> SkillResult {
        let start = Instant::now();

        let skill = match self.registry.get_skill(skill_id) {
            Some(s) => s,
            None => {
                return SkillResult {
                    success: false,
                    output: None,
                    error: Some(format!("Skill not found: {}", skill_id)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    steps: None,
                };
            }
        };

        if !skill.enabled {
            return SkillResult {
                success: false,
                output: None,
                error: Some("Skill is disabled".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                steps: None,
            };
        }

        info!("Executing skill: {} ({})", skill.name, skill.id);

        match skill.kind {
            SkillKind::SlashCommand => self.execute_slash_command(&skill, context).await,
            SkillKind::Hook => self.execute_hook(&skill, context).await,
            SkillKind::Workflow => self.execute_workflow(&skill, context).await,
            SkillKind::Template => self.execute_template(&skill, context).await,
            SkillKind::Agent => self.execute_agent(&skill, context).await,
        }
    }

    /// Execute a slash command by name
    pub async fn execute_slash_command_by_name(
        &self,
        command_name: &str,
        arguments: &str,
        project_path: &str,
    ) -> SkillResult {
        let start = Instant::now();

        let skill = match self.registry.get_slash_command(command_name) {
            Some(s) => s,
            None => {
                return SkillResult {
                    success: false,
                    output: None,
                    error: Some(format!("Slash command not found: /{}", command_name)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    steps: None,
                };
            }
        };

        let mut args = HashMap::new();
        args.insert("ARGUMENTS".to_string(), serde_json::json!(arguments));

        let context = SkillContext {
            project_path: project_path.to_string(),
            session_id: None,
            arguments: args,
            env: std::env::vars().collect(),
            variables: HashMap::new(),
        };

        self.execute_slash_command(&skill, context).await
    }

    /// Execute a slash command skill
    async fn execute_slash_command(&self, skill: &Skill, context: SkillContext) -> SkillResult {
        let start = Instant::now();

        let cmd_config = match &skill.config.slash_command {
            Some(c) => c,
            None => {
                return SkillResult {
                    success: false,
                    output: None,
                    error: Some("Invalid slash command configuration".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    steps: None,
                };
            }
        };

        // Expand template variables in prompt
        let mut prompt = cmd_config.prompt.clone();

        // Replace $ARGUMENTS with actual arguments
        if let Some(args) = context.arguments.get("ARGUMENTS") {
            let args_str = args.as_str().unwrap_or("");
            prompt = prompt.replace("$ARGUMENTS", args_str);
        }

        // Replace other variables
        for (key, value) in &context.variables {
            let placeholder = format!("${{{}}}", key);
            let value_str = match value {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            prompt = prompt.replace(&placeholder, &value_str);
        }

        info!("Slash command /{} expanded prompt: {}", cmd_config.name, prompt);

        SkillResult {
            success: true,
            output: Some(serde_json::json!({
                "prompt": prompt,
                "command": cmd_config.name,
            })),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            steps: None,
        }
    }

    /// Execute a hook skill
    async fn execute_hook(&self, skill: &Skill, context: SkillContext) -> SkillResult {
        let start = Instant::now();

        let hook_config = match &skill.config.hook {
            Some(h) => h,
            None => {
                return SkillResult {
                    success: false,
                    output: None,
                    error: Some("Invalid hook configuration".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    steps: None,
                };
            }
        };

        // Execute the hook command
        let result = self
            .run_shell_command(
                &hook_config.command,
                &context.project_path,
                hook_config.timeout_secs,
                &hook_config.env,
            )
            .await;

        match result {
            Ok((stdout, stderr, exit_code)) => {
                let success = exit_code == 0;
                SkillResult {
                    success,
                    output: Some(serde_json::json!({
                        "stdout": stdout,
                        "stderr": stderr,
                        "exit_code": exit_code,
                    })),
                    error: if success { None } else { Some(stderr) },
                    duration_ms: start.elapsed().as_millis() as u64,
                    steps: None,
                }
            }
            Err(e) => SkillResult {
                success: false,
                output: None,
                error: Some(e),
                duration_ms: start.elapsed().as_millis() as u64,
                steps: None,
            },
        }
    }

    /// Execute hooks for a specific trigger
    pub async fn execute_hooks_for_trigger(
        &self,
        trigger: HookTrigger,
        context: SkillContext,
    ) -> Vec<SkillResult> {
        let trigger_str = format!("{:?}", trigger).to_lowercase();
        let hooks = self.registry.get_hooks_for_trigger(&trigger_str);

        let mut results = Vec::new();
        for hook in hooks {
            let result = self.execute(&hook.id, context.clone()).await;
            results.push(result);
        }

        results
    }

    /// Execute a workflow skill
    async fn execute_workflow(&self, skill: &Skill, context: SkillContext) -> SkillResult {
        let start = Instant::now();

        let workflow = match &skill.config.workflow {
            Some(w) => w,
            None => {
                return SkillResult {
                    success: false,
                    output: None,
                    error: Some("Invalid workflow configuration".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    steps: None,
                };
            }
        };

        // Build dependency graph and execute in order
        let mut step_results = Vec::new();
        let mut completed: HashMap<String, serde_json::Value> = HashMap::new();
        let mut variables = context.variables.clone();

        // Simple sequential execution (TODO: parallel execution for independent steps)
        for step in &workflow.steps {
            // Check dependencies
            let deps_met = step
                .depends_on
                .iter()
                .all(|dep| completed.contains_key(dep));

            if !deps_met {
                step_results.push(StepResult {
                    step_id: step.id.clone(),
                    step_name: step.name.clone(),
                    success: false,
                    output: None,
                    error: Some("Dependencies not met".to_string()),
                    duration_ms: 0,
                    retries: 0,
                });
                continue;
            }

            // Execute step
            let step_result = self
                .execute_workflow_step(step, &context, &completed, &mut variables)
                .await;

            if step_result.success {
                if let Some(ref output) = step_result.output {
                    completed.insert(step.id.clone(), output.clone());
                }
            }

            let step_failed = !step_result.success;
            step_results.push(step_result);

            if step_failed {
                break; // Stop on first failure
            }
        }

        let all_success = step_results.iter().all(|r| r.success);

        SkillResult {
            success: all_success,
            output: Some(serde_json::json!({
                "completed": completed,
                "variables": variables,
            })),
            error: if all_success {
                None
            } else {
                step_results
                    .iter()
                    .find(|r| !r.success)
                    .and_then(|r| r.error.clone())
            },
            duration_ms: start.elapsed().as_millis() as u64,
            steps: Some(step_results),
        }
    }

    /// Execute a single workflow step
    async fn execute_workflow_step(
        &self,
        step: &WorkflowStep,
        context: &SkillContext,
        completed: &HashMap<String, serde_json::Value>,
        variables: &mut HashMap<String, serde_json::Value>,
    ) -> StepResult {
        let start = Instant::now();
        info!("Executing workflow step: {} ({})", step.name, step.id);

        let result = match step.kind {
            WorkflowStepKind::Shell => {
                let command = step
                    .config
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                match self
                    .run_shell_command(
                        command,
                        &context.project_path,
                        step.timeout_secs.unwrap_or(60),
                        &context.env,
                    )
                    .await
                {
                    Ok((stdout, stderr, code)) => {
                        let success = code == 0;
                        (
                            success,
                            Some(serde_json::json!({
                                "stdout": stdout,
                                "stderr": stderr,
                                "exit_code": code,
                            })),
                            if success { None } else { Some(stderr) },
                        )
                    }
                    Err(e) => (false, None, Some(e)),
                }
            }
            WorkflowStepKind::Prompt => {
                let prompt = step
                    .config
                    .get("prompt")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // Just return the prompt for now
                (
                    true,
                    Some(serde_json::json!({ "prompt": prompt })),
                    None,
                )
            }
            WorkflowStepKind::SkillRef => {
                let skill_id = step
                    .config
                    .get("skill_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let result = self.execute(skill_id, context.clone()).await;
                (result.success, result.output, result.error)
            }
            _ => (true, None, None),
        };

        StepResult {
            step_id: step.id.clone(),
            step_name: step.name.clone(),
            success: result.0,
            output: result.1,
            error: result.2,
            duration_ms: start.elapsed().as_millis() as u64,
            retries: 0,
        }
    }

    /// Execute a template skill
    async fn execute_template(&self, skill: &Skill, context: SkillContext) -> SkillResult {
        let start = Instant::now();

        let template = match &skill.config.template {
            Some(t) => t,
            None => {
                return SkillResult {
                    success: false,
                    output: None,
                    error: Some("Invalid template configuration".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    steps: None,
                };
            }
        };

        // Expand template variables
        let mut content = template.content.clone();

        for var in &template.variables {
            let placeholder = format!("{{{{{}}}}}", var.name);
            let value = context
                .variables
                .get(&var.name)
                .map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .or(var.default.clone())
                .unwrap_or_default();

            content = content.replace(&placeholder, &value);
        }

        SkillResult {
            success: true,
            output: Some(serde_json::json!({
                "content": content,
            })),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            steps: None,
        }
    }

    /// Execute an agent skill
    async fn execute_agent(&self, skill: &Skill, context: SkillContext) -> SkillResult {
        let start = Instant::now();

        let agent_config = match &skill.config.agent {
            Some(a) => a,
            None => {
                return SkillResult {
                    success: false,
                    output: None,
                    error: Some("Invalid agent configuration".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    steps: None,
                };
            }
        };

        // Return the agent configuration for the caller to execute
        SkillResult {
            success: true,
            output: Some(serde_json::json!({
                "agent": {
                    "name": agent_config.name,
                    "model": agent_config.model,
                    "system_prompt": agent_config.system_prompt,
                    "permission_mode": agent_config.permission_mode,
                    "allowed_tools": agent_config.allowed_tools,
                    "denied_tools": agent_config.denied_tools,
                    "mcp_servers": agent_config.mcp_servers,
                    "max_turns": agent_config.max_turns,
                }
            })),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            steps: None,
        }
    }

    /// Run a shell command
    async fn run_shell_command(
        &self,
        command: &str,
        working_dir: &str,
        timeout_secs: u64,
        env: &HashMap<String, String>,
    ) -> Result<(String, String, i32), String> {
        let shell = if cfg!(windows) { "cmd" } else { "sh" };
        let shell_arg = if cfg!(windows) { "/C" } else { "-c" };

        let mut cmd = Command::new(shell);
        cmd.arg(shell_arg)
            .arg(command)
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Add environment variables
        for (key, value) in env {
            cmd.env(key, value);
        }

        let child = cmd.spawn().map_err(|e| format!("Failed to spawn command: {}", e))?;

        let output = tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            child.wait_with_output(),
        )
        .await
        .map_err(|_| "Command timed out".to_string())?
        .map_err(|e| format!("Command failed: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        Ok((stdout, stderr, exit_code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_executor_creation() {
        let registry = std::sync::Arc::new(SkillRegistry::new());
        let executor = SkillExecutor::new(registry);
        assert_eq!(executor.default_timeout_secs, 300);
    }
}
