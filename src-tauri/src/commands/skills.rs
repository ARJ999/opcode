//! Skills Commands
//!
//! Tauri commands for managing the unified skills system.

use log::{debug, error, info, warn};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

use crate::commands::agents::AgentDb;
use crate::skills::{
    registry::SkillRegistry,
    loader::{SkillLoader, LoaderError},
    executor::SkillExecutor,
    types::{
        Skill, SkillKind, SkillVisibility, SkillConfig, SkillMetadata,
        SkillContext, SkillResult, SlashCommandConfig, HookConfig, WorkflowConfig,
        HookTrigger,
    },
};

/// Skill info for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfo {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub description: String,
    pub visibility: String,
    pub enabled: bool,
    pub source: String,
    pub project_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<&Skill> for SkillInfo {
    fn from(skill: &Skill) -> Self {
        Self {
            id: skill.id.clone(),
            kind: format!("{:?}", skill.kind).to_lowercase(),
            name: skill.name.clone(),
            description: skill.description.clone(),
            visibility: format!("{:?}", skill.visibility).to_lowercase(),
            enabled: skill.enabled,
            source: skill.source.clone(),
            project_path: skill.project_path.clone(),
            created_at: skill.created_at.clone(),
            updated_at: skill.updated_at.clone(),
        }
    }
}

/// Create slash command request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSlashCommandRequest {
    pub name: String,
    pub description: String,
    pub prompt: String,
    pub help: Option<String>,
    pub examples: Option<Vec<String>>,
    pub visibility: Option<String>,
    pub project_path: Option<String>,
}

/// Create hook request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateHookRequest {
    pub name: String,
    pub description: String,
    pub trigger: String,
    pub command: String,
    pub timeout_secs: Option<u64>,
    pub tool_patterns: Option<Vec<String>>,
    pub can_block: Option<bool>,
    pub visibility: Option<String>,
    pub project_path: Option<String>,
}

/// Initialize skills table in database
pub fn init_skills_table(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    SkillRegistry::init_database(conn)
}

/// List all skills
#[tauri::command]
pub async fn list_skills(
    db: State<'_, AgentDb>,
    kind: Option<String>,
    project_path: Option<String>,
) -> Result<Vec<SkillInfo>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    // Ensure table exists
    let _ = init_skills_table(&conn);

    let mut query = "SELECT id, kind, name, description, visibility, enabled, source, project_path, created_at, updated_at FROM skills WHERE 1=1".to_string();
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(ref k) = kind {
        query.push_str(" AND kind = ?");
        params_vec.push(Box::new(k.clone()));
    }

    if let Some(ref path) = project_path {
        query.push_str(" AND (visibility = 'global' OR project_path = ?)");
        params_vec.push(Box::new(path.clone()));
    }

    query.push_str(" ORDER BY created_at DESC");

    let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;

    let skills = stmt
        .query_map(rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())), |row| {
            Ok(SkillInfo {
                id: row.get(0)?,
                kind: row.get(1)?,
                name: row.get(2)?,
                description: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                visibility: row.get(4)?,
                enabled: row.get(5)?,
                source: row.get(6)?,
                project_path: row.get(7).ok(),
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(skills)
}

/// Get a skill by ID
#[tauri::command]
pub async fn get_skill(db: State<'_, AgentDb>, id: String) -> Result<Skill, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    let skill = conn
        .query_row(
            "SELECT id, kind, name, description, visibility, enabled, config, metadata, project_path, source, created_at, updated_at
             FROM skills WHERE id = ?1",
            params![id],
            |row| {
                let kind_str: String = row.get(1)?;
                let visibility_str: String = row.get(4)?;
                let config_str: String = row.get(6)?;
                let metadata_str: Option<String> = row.get(7)?;

                Ok(Skill {
                    id: row.get(0)?,
                    kind: serde_json::from_str(&format!("\"{}\"", kind_str)).unwrap_or(SkillKind::SlashCommand),
                    name: row.get(2)?,
                    description: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    visibility: serde_json::from_str(&format!("\"{}\"", visibility_str)).unwrap_or(SkillVisibility::Global),
                    enabled: row.get(5)?,
                    config: serde_json::from_str(&config_str).unwrap_or_default(),
                    metadata: metadata_str.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default(),
                    project_path: row.get(8).ok(),
                    source: row.get(9)?,
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                })
            },
        )
        .map_err(|e| format!("Skill not found: {}", e))?;

    Ok(skill)
}

/// Create a slash command skill
#[tauri::command]
pub async fn create_slash_command(
    db: State<'_, AgentDb>,
    request: CreateSlashCommandRequest,
) -> Result<SkillInfo, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    // Ensure table exists
    let _ = init_skills_table(&conn);

    let id = uuid::Uuid::new_v4().to_string();
    let visibility = request.visibility.as_deref().unwrap_or("global");

    let config = SkillConfig {
        slash_command: Some(SlashCommandConfig {
            name: request.name.clone(),
            description: request.description.clone(),
            help: request.help,
            prompt: request.prompt,
            requires_args: false, // Will be updated based on prompt content
            args: None,
            examples: request.examples.unwrap_or_default(),
        }),
        ..Default::default()
    };

    let config_str = serde_json::to_string(&config).map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO skills (id, kind, name, description, visibility, enabled, config, source, project_path, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, 'local', ?7, ?8, ?9)",
        params![
            id,
            "slash_command",
            format!("/{}", request.name),
            request.description,
            visibility,
            config_str,
            request.project_path,
            now,
            now
        ],
    ).map_err(|e| e.to_string())?;

    info!("Created slash command: /{} ({})", request.name, id);

    Ok(SkillInfo {
        id,
        kind: "slash_command".to_string(),
        name: format!("/{}", request.name),
        description: request.description,
        visibility: visibility.to_string(),
        enabled: true,
        source: "local".to_string(),
        project_path: request.project_path,
        created_at: now.clone(),
        updated_at: now,
    })
}

/// Create a hook skill
#[tauri::command]
pub async fn create_hook(
    db: State<'_, AgentDb>,
    request: CreateHookRequest,
) -> Result<SkillInfo, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    // Ensure table exists
    let _ = init_skills_table(&conn);

    let id = uuid::Uuid::new_v4().to_string();
    let visibility = request.visibility.as_deref().unwrap_or("global");

    let trigger: HookTrigger = match request.trigger.as_str() {
        "pre_tool" => HookTrigger::PreTool,
        "post_tool" => HookTrigger::PostTool,
        "session_start" => HookTrigger::SessionStart,
        "session_end" => HookTrigger::SessionEnd,
        "checkpoint_create" => HookTrigger::CheckpointCreate,
        "on_error" => HookTrigger::OnError,
        _ => return Err(format!("Invalid trigger type: {}", request.trigger)),
    };

    let config = SkillConfig {
        hook: Some(HookConfig {
            trigger,
            tool_patterns: request.tool_patterns,
            command: request.command,
            timeout_secs: request.timeout_secs.unwrap_or(30),
            can_block: request.can_block.unwrap_or(false),
            env: HashMap::new(),
        }),
        ..Default::default()
    };

    let config_str = serde_json::to_string(&config).map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO skills (id, kind, name, description, visibility, enabled, config, source, project_path, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, 'local', ?7, ?8, ?9)",
        params![
            id,
            "hook",
            request.name,
            request.description,
            visibility,
            config_str,
            request.project_path,
            now,
            now
        ],
    ).map_err(|e| e.to_string())?;

    info!("Created hook: {} ({})", request.name, id);

    Ok(SkillInfo {
        id,
        kind: "hook".to_string(),
        name: request.name,
        description: request.description,
        visibility: visibility.to_string(),
        enabled: true,
        source: "local".to_string(),
        project_path: request.project_path,
        created_at: now.clone(),
        updated_at: now,
    })
}

/// Update a skill
#[tauri::command]
pub async fn update_skill(
    db: State<'_, AgentDb>,
    id: String,
    name: Option<String>,
    description: Option<String>,
    enabled: Option<bool>,
    config: Option<serde_json::Value>,
) -> Result<SkillInfo, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    // Get current skill
    let current = conn
        .query_row(
            "SELECT name, description, enabled, config FROM skills WHERE id = ?1",
            params![id],
            |row| Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, bool>(2)?,
                row.get::<_, String>(3)?,
            )),
        )
        .map_err(|e| format!("Skill not found: {}", e))?;

    let new_name = name.unwrap_or(current.0);
    let new_description = description.unwrap_or(current.1);
    let new_enabled = enabled.unwrap_or(current.2);
    let new_config = config
        .map(|c| serde_json::to_string(&c).unwrap_or(current.3.clone()))
        .unwrap_or(current.3);

    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "UPDATE skills SET name = ?1, description = ?2, enabled = ?3, config = ?4, updated_at = ?5 WHERE id = ?6",
        params![new_name, new_description, new_enabled, new_config, now, id.clone()],
    ).map_err(|e| e.to_string())?;

    // Drop conn to release the borrow before using db again
    drop(conn);

    // Fetch updated skill
    get_skill(db, id).await.map(|s| SkillInfo::from(&s))
}

/// Delete a skill
#[tauri::command]
pub async fn delete_skill(db: State<'_, AgentDb>, id: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    conn.execute("DELETE FROM skills WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;

    info!("Deleted skill: {}", id);
    Ok(())
}

/// Execute a slash command
#[tauri::command]
pub async fn execute_slash_command(
    db: State<'_, AgentDb>,
    command_name: String,
    arguments: String,
    project_path: String,
) -> Result<serde_json::Value, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    // Find the skill by command name
    let skill: Skill = conn
        .query_row(
            "SELECT id, kind, name, description, visibility, enabled, config, metadata, project_path, source, created_at, updated_at
             FROM skills WHERE kind = 'slash_command' AND json_extract(config, '$.slash_command.name') = ?1",
            params![command_name],
            |row| {
                let kind_str: String = row.get(1)?;
                let visibility_str: String = row.get(4)?;
                let config_str: String = row.get(6)?;
                let metadata_str: Option<String> = row.get(7)?;

                Ok(Skill {
                    id: row.get(0)?,
                    kind: serde_json::from_str(&format!("\"{}\"", kind_str)).unwrap_or(SkillKind::SlashCommand),
                    name: row.get(2)?,
                    description: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    visibility: serde_json::from_str(&format!("\"{}\"", visibility_str)).unwrap_or(SkillVisibility::Global),
                    enabled: row.get(5)?,
                    config: serde_json::from_str(&config_str).unwrap_or_default(),
                    metadata: metadata_str.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default(),
                    project_path: row.get(8).ok(),
                    source: row.get(9)?,
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                })
            },
        )
        .map_err(|e| format!("Slash command not found: /{} - {}", command_name, e))?;

    drop(conn); // Release lock

    if !skill.enabled {
        return Err("Slash command is disabled".to_string());
    }

    // Get the prompt template and expand it
    let cmd_config = skill.config.slash_command
        .ok_or("Invalid slash command configuration")?;

    let mut prompt = cmd_config.prompt.clone();
    prompt = prompt.replace("$ARGUMENTS", &arguments);

    info!("Executing slash command /{}: {}", command_name, prompt);

    Ok(serde_json::json!({
        "command": command_name,
        "prompt": prompt,
        "description": cmd_config.description,
    }))
}

/// List slash commands
#[tauri::command]
pub async fn list_slash_commands(
    db: State<'_, AgentDb>,
    project_path: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    let mut query = "SELECT id, name, description, config FROM skills WHERE kind = 'slash_command' AND enabled = 1".to_string();

    if project_path.is_some() {
        query.push_str(" AND (visibility = 'global' OR project_path = ?1)");
    }

    query.push_str(" ORDER BY name ASC");

    let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;

    let commands: Vec<serde_json::Value> = if let Some(ref path) = project_path {
        stmt.query_map(params![path], |row| {
            let config_str: String = row.get(3)?;
            let config: SkillConfig = serde_json::from_str(&config_str).unwrap_or_default();
            let cmd = config.slash_command.unwrap_or_else(|| SlashCommandConfig {
                name: "unknown".to_string(),
                description: "".to_string(),
                help: None,
                prompt: "".to_string(),
                requires_args: false,
                args: None,
                examples: vec![],
            });

            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "name": cmd.name,
                "description": cmd.description,
                "help": cmd.help,
                "requires_args": cmd.requires_args,
                "examples": cmd.examples,
            }))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
    } else {
        stmt.query_map([], |row| {
            let config_str: String = row.get(3)?;
            let config: SkillConfig = serde_json::from_str(&config_str).unwrap_or_default();
            let cmd = config.slash_command.unwrap_or_else(|| SlashCommandConfig {
                name: "unknown".to_string(),
                description: "".to_string(),
                help: None,
                prompt: "".to_string(),
                requires_args: false,
                args: None,
                examples: vec![],
            });

            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "name": cmd.name,
                "description": cmd.description,
                "help": cmd.help,
                "requires_args": cmd.requires_args,
                "examples": cmd.examples,
            }))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
    };

    Ok(commands)
}

/// Import skills from Claude Code settings
#[tauri::command]
pub async fn import_claude_code_skills(
    db: State<'_, AgentDb>,
    settings_path: String,
) -> Result<Vec<SkillInfo>, String> {
    let path = PathBuf::from(&settings_path);
    if !path.exists() {
        return Err("Settings file not found".to_string());
    }

    let skills_dir = dirs::data_dir()
        .map(|d| d.join("opcode").join("skills"))
        .ok_or("Could not find data directory")?;

    let loader = SkillLoader::new(skills_dir);
    let skills = loader.import_claude_code_settings(&path).await
        .map_err(|e| e.to_string())?;

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let _ = init_skills_table(&conn);

    let mut imported = Vec::new();

    for skill in skills {
        let config_str = serde_json::to_string(&skill.config).map_err(|e| e.to_string())?;
        let metadata_str = serde_json::to_string(&skill.metadata).ok();
        let kind_str = format!("{:?}", skill.kind).to_lowercase();
        let visibility_str = format!("{:?}", skill.visibility).to_lowercase();

        conn.execute(
            "INSERT OR REPLACE INTO skills (id, kind, name, description, visibility, enabled, config, metadata, project_path, source, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                skill.id,
                kind_str,
                skill.name,
                skill.description,
                visibility_str,
                skill.enabled,
                config_str,
                metadata_str,
                skill.project_path,
                skill.source,
                skill.created_at,
                skill.updated_at
            ],
        ).map_err(|e| e.to_string())?;

        imported.push(SkillInfo::from(&skill));
    }

    info!("Imported {} skills from Claude Code settings", imported.len());
    Ok(imported)
}

/// Import a skill from GitHub
#[tauri::command]
pub async fn import_skill_from_github(
    db: State<'_, AgentDb>,
    repo: String,
    path: String,
    github_token: Option<String>,
) -> Result<SkillInfo, String> {
    let skills_dir = dirs::data_dir()
        .map(|d| d.join("opcode").join("skills"))
        .ok_or("Could not find data directory")?;

    let mut loader = SkillLoader::new(skills_dir);
    if let Some(token) = github_token {
        loader = loader.with_github_token(token);
    }

    let skill = loader.load_from_github(&repo, &path).await
        .map_err(|e| e.to_string())?;

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let _ = init_skills_table(&conn);

    let config_str = serde_json::to_string(&skill.config).map_err(|e| e.to_string())?;
    let metadata_str = serde_json::to_string(&skill.metadata).ok();
    let kind_str = format!("{:?}", skill.kind).to_lowercase();
    let visibility_str = format!("{:?}", skill.visibility).to_lowercase();

    conn.execute(
        "INSERT OR REPLACE INTO skills (id, kind, name, description, visibility, enabled, config, metadata, project_path, source, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            skill.id,
            kind_str,
            skill.name,
            skill.description,
            visibility_str,
            skill.enabled,
            config_str,
            metadata_str,
            skill.project_path,
            skill.source,
            skill.created_at,
            skill.updated_at
        ],
    ).map_err(|e| e.to_string())?;

    info!("Imported skill from GitHub: {} ({})", skill.name, skill.id);
    Ok(SkillInfo::from(&skill))
}
