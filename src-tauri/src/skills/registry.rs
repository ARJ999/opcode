//! Skill Registry
//!
//! Central registry for managing skills.

use dashmap::DashMap;
use log::{debug, error, info, warn};
use rusqlite::{params, Connection};
use std::sync::Arc;

use super::types::{Skill, SkillKind, SkillVisibility, SkillConfig, SkillMetadata};

/// Skill registry for managing and accessing skills
pub struct SkillRegistry {
    /// In-memory skill cache (skill_id -> Skill)
    skills: Arc<DashMap<String, Skill>>,
    /// Slash command index (command_name -> skill_id)
    slash_commands: Arc<DashMap<String, String>>,
    /// Hook index (trigger_type -> Vec<skill_id>)
    hooks: Arc<DashMap<String, Vec<String>>>,
}

impl SkillRegistry {
    /// Create a new skill registry
    pub fn new() -> Self {
        Self {
            skills: Arc::new(DashMap::new()),
            slash_commands: Arc::new(DashMap::new()),
            hooks: Arc::new(DashMap::new()),
        }
    }

    /// Initialize the skills database table
    pub fn init_database(conn: &Connection) -> Result<(), rusqlite::Error> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS skills (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                visibility TEXT NOT NULL DEFAULT 'global',
                enabled BOOLEAN DEFAULT 1,
                config TEXT NOT NULL,
                metadata TEXT,
                project_path TEXT,
                source TEXT DEFAULT 'local',
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Create indexes
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_skills_kind ON skills(kind)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_skills_visibility ON skills(visibility)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_skills_project ON skills(project_path)",
            [],
        )?;

        info!("Skills database table initialized");
        Ok(())
    }

    /// Load all skills from database into cache
    pub fn load_from_database(&self, conn: &Connection) -> Result<usize, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT id, kind, name, description, visibility, enabled, config, metadata, project_path, source, created_at, updated_at
             FROM skills WHERE enabled = 1"
        )?;

        let skills_iter = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let kind_str: String = row.get(1)?;
            let name: String = row.get(2)?;
            let description: Option<String> = row.get(3)?;
            let visibility_str: String = row.get(4)?;
            let enabled: bool = row.get(5)?;
            let config_str: String = row.get(6)?;
            let metadata_str: Option<String> = row.get(7)?;
            let project_path: Option<String> = row.get(8)?;
            let source: String = row.get(9)?;
            let created_at: String = row.get(10)?;
            let updated_at: String = row.get(11)?;

            let kind: SkillKind = serde_json::from_str(&format!("\"{}\"", kind_str))
                .unwrap_or(SkillKind::SlashCommand);
            let visibility: SkillVisibility = serde_json::from_str(&format!("\"{}\"", visibility_str))
                .unwrap_or(SkillVisibility::Global);
            let config: SkillConfig = serde_json::from_str(&config_str).unwrap_or_default();
            let metadata: SkillMetadata = metadata_str
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();

            Ok(Skill {
                id,
                kind,
                name,
                description: description.unwrap_or_default(),
                visibility,
                enabled,
                config,
                metadata,
                project_path,
                source,
                created_at,
                updated_at,
            })
        })?;

        let mut count = 0;
        for skill_result in skills_iter {
            if let Ok(skill) = skill_result {
                self.register_skill(skill);
                count += 1;
            }
        }

        info!("Loaded {} skills from database", count);
        Ok(count)
    }

    /// Register a skill in the cache and index
    pub fn register_skill(&self, skill: Skill) {
        let skill_id = skill.id.clone();

        // Index by kind
        match &skill.kind {
            SkillKind::SlashCommand => {
                if let Some(ref cmd) = skill.config.slash_command {
                    self.slash_commands.insert(cmd.name.clone(), skill_id.clone());
                    debug!("Registered slash command: /{}", cmd.name);
                }
            }
            SkillKind::Hook => {
                if let Some(ref hook) = skill.config.hook {
                    let trigger_key = format!("{:?}", hook.trigger).to_lowercase();
                    self.hooks
                        .entry(trigger_key)
                        .or_insert_with(Vec::new)
                        .push(skill_id.clone());
                    debug!("Registered hook: {:?}", hook.trigger);
                }
            }
            _ => {}
        }

        self.skills.insert(skill_id, skill);
    }

    /// Unregister a skill from cache and index
    pub fn unregister_skill(&self, skill_id: &str) {
        if let Some((_, skill)) = self.skills.remove(skill_id) {
            // Remove from indexes
            match &skill.kind {
                SkillKind::SlashCommand => {
                    if let Some(ref cmd) = skill.config.slash_command {
                        self.slash_commands.remove(&cmd.name);
                    }
                }
                SkillKind::Hook => {
                    if let Some(ref hook) = skill.config.hook {
                        let trigger_key = format!("{:?}", hook.trigger).to_lowercase();
                        if let Some(mut hooks) = self.hooks.get_mut(&trigger_key) {
                            hooks.retain(|id| id != skill_id);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Save a skill to the database
    pub fn save_skill(&self, conn: &Connection, skill: &Skill) -> Result<(), rusqlite::Error> {
        let kind_str = serde_json::to_string(&skill.kind)
            .unwrap_or_else(|_| "\"slash_command\"".to_string())
            .trim_matches('"')
            .to_string();
        let visibility_str = serde_json::to_string(&skill.visibility)
            .unwrap_or_else(|_| "\"global\"".to_string())
            .trim_matches('"')
            .to_string();
        let config_str = serde_json::to_string(&skill.config).unwrap_or_else(|_| "{}".to_string());
        let metadata_str = serde_json::to_string(&skill.metadata).ok();

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
                chrono::Utc::now().to_rfc3339()
            ],
        )?;

        // Update cache
        self.register_skill(skill.clone());

        info!("Saved skill: {} ({})", skill.name, skill.id);
        Ok(())
    }

    /// Delete a skill from database and cache
    pub fn delete_skill(&self, conn: &Connection, skill_id: &str) -> Result<(), rusqlite::Error> {
        conn.execute("DELETE FROM skills WHERE id = ?1", params![skill_id])?;
        self.unregister_skill(skill_id);
        info!("Deleted skill: {}", skill_id);
        Ok(())
    }

    /// Get a skill by ID
    pub fn get_skill(&self, skill_id: &str) -> Option<Skill> {
        self.skills.get(skill_id).map(|s| s.clone())
    }

    /// Get a slash command skill by command name
    pub fn get_slash_command(&self, command_name: &str) -> Option<Skill> {
        self.slash_commands
            .get(command_name)
            .and_then(|id| self.skills.get(id.value()).map(|s| s.clone()))
    }

    /// Get all hooks for a trigger type
    pub fn get_hooks_for_trigger(&self, trigger: &str) -> Vec<Skill> {
        self.hooks
            .get(trigger)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.skills.get(id).map(|s| s.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// List all skills of a specific kind
    pub fn list_skills_by_kind(&self, kind: SkillKind) -> Vec<Skill> {
        self.skills
            .iter()
            .filter(|s| s.kind == kind)
            .map(|s| s.clone())
            .collect()
    }

    /// List all skills
    pub fn list_all_skills(&self) -> Vec<Skill> {
        self.skills.iter().map(|s| s.clone()).collect()
    }

    /// List skills for a specific project
    pub fn list_project_skills(&self, project_path: &str) -> Vec<Skill> {
        self.skills
            .iter()
            .filter(|s| {
                s.visibility == SkillVisibility::Global
                    || s.project_path.as_deref() == Some(project_path)
            })
            .map(|s| s.clone())
            .collect()
    }

    /// List all slash commands
    pub fn list_slash_commands(&self) -> Vec<(String, Skill)> {
        self.slash_commands
            .iter()
            .filter_map(|entry| {
                let cmd_name = entry.key().clone();
                let skill_id = entry.value().clone();
                self.skills.get(&skill_id).map(|s| (cmd_name, s.clone()))
            })
            .collect()
    }

    /// Count skills by kind
    pub fn count_by_kind(&self) -> std::collections::HashMap<SkillKind, usize> {
        let mut counts = std::collections::HashMap::new();
        for skill in self.skills.iter() {
            *counts.entry(skill.kind.clone()).or_insert(0) += 1;
        }
        counts
    }

    /// Check if a slash command exists
    pub fn has_slash_command(&self, command_name: &str) -> bool {
        self.slash_commands.contains_key(command_name)
    }

    /// Clear all skills from cache (not database)
    pub fn clear_cache(&self) {
        self.skills.clear();
        self.slash_commands.clear();
        self.hooks.clear();
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = SkillRegistry::new();
        assert_eq!(registry.list_all_skills().len(), 0);
    }

    #[test]
    fn test_register_slash_command() {
        let registry = SkillRegistry::new();

        let config = SkillConfig {
            slash_command: Some(super::super::types::SlashCommandConfig {
                name: "test".to_string(),
                description: "Test command".to_string(),
                help: None,
                prompt: "Test prompt".to_string(),
                requires_args: false,
                args: None,
                examples: vec![],
            }),
            ..Default::default()
        };

        let skill = Skill {
            id: "test-skill".to_string(),
            kind: SkillKind::SlashCommand,
            name: "Test Skill".to_string(),
            description: "A test skill".to_string(),
            visibility: SkillVisibility::Global,
            enabled: true,
            config,
            metadata: SkillMetadata::default(),
            project_path: None,
            source: "local".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };

        registry.register_skill(skill);

        assert!(registry.has_slash_command("test"));
        assert!(registry.get_slash_command("test").is_some());
    }
}
