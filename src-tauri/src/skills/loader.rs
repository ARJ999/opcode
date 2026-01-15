//! Skill Loader
//!
//! Load skills from various sources: local files, GitHub, registry.

use log::{debug, error, info, warn};
use std::path::{Path, PathBuf};
use tokio::fs;

use super::types::{Skill, SkillConfig, SkillKind, SkillMetadata, SkillVisibility};

/// Skill loader for loading skills from various sources
pub struct SkillLoader {
    /// Base directory for local skills
    skills_dir: PathBuf,
    /// GitHub API token (optional, for private repos)
    github_token: Option<String>,
}

impl SkillLoader {
    /// Create a new skill loader
    pub fn new(skills_dir: impl Into<PathBuf>) -> Self {
        Self {
            skills_dir: skills_dir.into(),
            github_token: None,
        }
    }

    /// Set GitHub token for private repository access
    pub fn with_github_token(mut self, token: impl Into<String>) -> Self {
        self.github_token = Some(token.into());
        self
    }

    /// Load skills from the local skills directory
    pub async fn load_local_skills(&self) -> Result<Vec<Skill>, LoaderError> {
        let mut skills = Vec::new();

        if !self.skills_dir.exists() {
            // Create directory if it doesn't exist
            fs::create_dir_all(&self.skills_dir).await
                .map_err(|e| LoaderError::IoError(e.to_string()))?;
            return Ok(skills);
        }

        let mut entries = fs::read_dir(&self.skills_dir).await
            .map_err(|e| LoaderError::IoError(e.to_string()))?;

        while let Some(entry) = entries.next_entry().await
            .map_err(|e| LoaderError::IoError(e.to_string()))? {
            let path = entry.path();

            if path.extension().map_or(false, |ext| ext == "json" || ext == "yaml" || ext == "yml") {
                match self.load_skill_file(&path).await {
                    Ok(skill) => {
                        info!("Loaded skill from {:?}: {}", path, skill.name);
                        skills.push(skill);
                    }
                    Err(e) => {
                        warn!("Failed to load skill from {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(skills)
    }

    /// Load a single skill from a file
    pub async fn load_skill_file(&self, path: &Path) -> Result<Skill, LoaderError> {
        let content = fs::read_to_string(path).await
            .map_err(|e| LoaderError::IoError(e.to_string()))?;

        let extension = path.extension().and_then(|e| e.to_str());

        let mut skill: Skill = match extension {
            Some("json") => serde_json::from_str(&content)
                .map_err(|e| LoaderError::ParseError(format!("JSON parse error: {}", e)))?,
            Some("yaml") | Some("yml") => serde_yaml::from_str(&content)
                .map_err(|e| LoaderError::ParseError(format!("YAML parse error: {}", e)))?,
            _ => return Err(LoaderError::ParseError("Unknown file extension".to_string())),
        };

        // Set source to local file path
        skill.source = format!("file://{}", path.display());

        // Generate ID if not provided
        if skill.id.is_empty() {
            skill.id = uuid::Uuid::new_v4().to_string();
        }

        // Set timestamps
        if skill.created_at.is_empty() {
            skill.created_at = chrono::Utc::now().to_rfc3339();
        }
        skill.updated_at = chrono::Utc::now().to_rfc3339();

        Ok(skill)
    }

    /// Save a skill to a local file
    pub async fn save_skill_file(&self, skill: &Skill, filename: &str) -> Result<PathBuf, LoaderError> {
        let path = self.skills_dir.join(filename);

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await
                .map_err(|e| LoaderError::IoError(e.to_string()))?;
        }

        let content = if filename.ends_with(".yaml") || filename.ends_with(".yml") {
            serde_yaml::to_string(skill)
                .map_err(|e| LoaderError::SerializeError(e.to_string()))?
        } else {
            serde_json::to_string_pretty(skill)
                .map_err(|e| LoaderError::SerializeError(e.to_string()))?
        };

        fs::write(&path, content).await
            .map_err(|e| LoaderError::IoError(e.to_string()))?;

        info!("Saved skill to {:?}", path);
        Ok(path)
    }

    /// Load a skill from a GitHub repository
    pub async fn load_from_github(&self, repo: &str, path: &str) -> Result<Skill, LoaderError> {
        let url = format!(
            "https://raw.githubusercontent.com/{}/main/{}",
            repo, path
        );

        let client = reqwest::Client::new();
        let mut request = client.get(&url);

        if let Some(ref token) = self.github_token {
            request = request.header("Authorization", format!("token {}", token));
        }

        let response = request.send().await
            .map_err(|e| LoaderError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(LoaderError::NetworkError(format!(
                "GitHub returned status {}",
                response.status()
            )));
        }

        let content = response.text().await
            .map_err(|e| LoaderError::NetworkError(e.to_string()))?;

        let mut skill: Skill = if path.ends_with(".yaml") || path.ends_with(".yml") {
            serde_yaml::from_str(&content)
                .map_err(|e| LoaderError::ParseError(format!("YAML parse error: {}", e)))?
        } else {
            serde_json::from_str(&content)
                .map_err(|e| LoaderError::ParseError(format!("JSON parse error: {}", e)))?
        };

        // Set source
        skill.source = format!("github://{}/{}", repo, path);

        // Generate ID if not provided
        if skill.id.is_empty() {
            skill.id = uuid::Uuid::new_v4().to_string();
        }

        skill.updated_at = chrono::Utc::now().to_rfc3339();

        Ok(skill)
    }

    /// Load skills from a GitHub repository directory
    pub async fn load_from_github_dir(&self, repo: &str, dir: &str) -> Result<Vec<Skill>, LoaderError> {
        let api_url = format!(
            "https://api.github.com/repos/{}/contents/{}",
            repo, dir
        );

        let client = reqwest::Client::new();
        let mut request = client.get(&api_url)
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "opcode/2.0");

        if let Some(ref token) = self.github_token {
            request = request.header("Authorization", format!("token {}", token));
        }

        let response = request.send().await
            .map_err(|e| LoaderError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(LoaderError::NetworkError(format!(
                "GitHub API returned status {}",
                response.status()
            )));
        }

        let entries: Vec<GitHubContent> = response.json().await
            .map_err(|e| LoaderError::ParseError(e.to_string()))?;

        let mut skills = Vec::new();

        for entry in entries {
            if entry.content_type == "file" {
                let ext = entry.name.split('.').last().unwrap_or("");
                if ext == "json" || ext == "yaml" || ext == "yml" {
                    match self.load_from_github(repo, &entry.path).await {
                        Ok(skill) => skills.push(skill),
                        Err(e) => warn!("Failed to load skill {}: {}", entry.name, e),
                    }
                }
            }
        }

        Ok(skills)
    }

    /// Parse a skill from TOML content (Claude Code format)
    pub fn parse_claude_code_skill(&self, content: &str, name: &str) -> Result<Skill, LoaderError> {
        // Claude Code uses TOML for .claude/settings.toml
        // This parses slash commands from that format
        let toml_value: toml::Value = toml::from_str(content)
            .map_err(|e| LoaderError::ParseError(format!("TOML parse error: {}", e)))?;

        // Look for slash_commands section
        let slash_commands = toml_value
            .get("slash_commands")
            .and_then(|v| v.as_table())
            .ok_or_else(|| LoaderError::ParseError("No slash_commands section".to_string()))?;

        let cmd = slash_commands
            .get(name)
            .ok_or_else(|| LoaderError::ParseError(format!("Slash command '{}' not found", name)))?;

        let prompt = cmd.get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| LoaderError::ParseError("Missing 'prompt' field".to_string()))?;

        let description = cmd.get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("No description");

        let skill = Skill {
            id: uuid::Uuid::new_v4().to_string(),
            kind: SkillKind::SlashCommand,
            name: format!("/{}", name),
            description: description.to_string(),
            visibility: SkillVisibility::Project,
            enabled: true,
            config: SkillConfig {
                slash_command: Some(super::types::SlashCommandConfig {
                    name: name.to_string(),
                    description: description.to_string(),
                    help: None,
                    prompt: prompt.to_string(),
                    requires_args: prompt.contains("$ARGUMENTS"),
                    args: None,
                    examples: vec![],
                }),
                ..Default::default()
            },
            metadata: SkillMetadata::default(),
            project_path: None,
            source: "claude-code".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };

        Ok(skill)
    }

    /// Import all slash commands from a Claude Code settings file
    pub async fn import_claude_code_settings(&self, settings_path: &Path) -> Result<Vec<Skill>, LoaderError> {
        let content = fs::read_to_string(settings_path).await
            .map_err(|e| LoaderError::IoError(e.to_string()))?;

        let toml_value: toml::Value = toml::from_str(&content)
            .map_err(|e| LoaderError::ParseError(format!("TOML parse error: {}", e)))?;

        let mut skills = Vec::new();

        if let Some(slash_commands) = toml_value.get("slash_commands").and_then(|v| v.as_table()) {
            for (name, _) in slash_commands {
                match self.parse_claude_code_skill(&content, name) {
                    Ok(skill) => skills.push(skill),
                    Err(e) => warn!("Failed to parse slash command '{}': {}", name, e),
                }
            }
        }

        Ok(skills)
    }
}

/// GitHub content API response
#[derive(Debug, serde::Deserialize)]
struct GitHubContent {
    name: String,
    path: String,
    #[serde(rename = "type")]
    content_type: String,
}

/// Loader errors
#[derive(Debug, thiserror::Error)]
pub enum LoaderError {
    #[error("IO error: {0}")]
    IoError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Serialize error: {0}")]
    SerializeError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loader_creation() {
        let loader = SkillLoader::new("/tmp/skills");
        assert!(loader.github_token.is_none());
    }

    #[test]
    fn test_parse_claude_code_skill() {
        let content = r#"
[slash_commands.test]
prompt = "Test the code with $ARGUMENTS"
description = "Run tests"
"#;
        let loader = SkillLoader::new("/tmp/skills");
        let skill = loader.parse_claude_code_skill(content, "test").unwrap();
        assert_eq!(skill.name, "/test");
        assert!(skill.config.slash_command.is_some());
    }
}
