//! `skill` tool: loads a named skill's full instructions into the
//! conversation on demand. Only skills that were discovered at startup
//! are accessible. The tool response contains the skill body plus a
//! listing of the skill's directory so the model knows what files it
//! can `read` next.

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolDefinition};
use crate::skills::{Skill, list_skill_files};

pub struct SkillTool {
    skills: Arc<Vec<Skill>>,
}

impl SkillTool {
    pub fn new(skills: Arc<Vec<Skill>>) -> Self {
        Self { skills }
    }
}

#[async_trait]
impl Tool for SkillTool {
    fn definition(&self) -> ToolDefinition {
        // Build an enum of valid names so the model gets a typed hint
        // instead of a free-form string. Empty enum would be invalid
        // JSON-Schema, so we only include it when at least one skill
        // is loaded.
        let mut props = json!({
            "name": {
                "type": "string",
                "description": "Name of the skill to load (must match one listed in the system prompt)."
            }
        });
        if !self.skills.is_empty() {
            let names: Vec<Value> = self
                .skills
                .iter()
                .map(|s| Value::String(s.name.clone()))
                .collect();
            props["name"]["enum"] = Value::Array(names);
        }

        ToolDefinition {
            name: "skill".to_string(),
            description:
                "Load the full instructions for a named skill. Call this when a skill \
                 listed in the system prompt looks relevant to the user's request. \
                 The response includes the skill's body and a list of files in its \
                 directory that you can read with the `read` tool."
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": props,
                "required": ["name"]
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let name = params["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'name'"))?;

        // Show spinner while loading skill - RAII guard ensures cleanup on error
        // let mut spinner = ToolSpinner::new(&format!("loading skill '{}'", name));

        let skill = self
            .skills
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| {
                let available: Vec<&str> = self.skills.iter().map(|s| s.name.as_str()).collect();
                // spinner.finish_error("skill not found");
                anyhow::anyhow!(
                    "unknown skill '{}'. Available: {}",
                    name,
                    if available.is_empty() {
                        "(none loaded)".to_string()
                    } else {
                        available.join(", ")
                    }
                )
            })?;

        let files = list_skill_files(&skill.dir);
        let files_section = if files.is_empty() {
            String::new()
        } else {
            let mut s = String::from("\n\n---\nFiles in this skill (read with the `read` tool, paths are relative to the skill directory):\n");
            s.push_str(&format!("skill_dir: {}\n", skill.dir.display()));
            for f in files {
                s.push_str(&format!("- {}\n", f));
            }
            s
        };

        let result = format!(
            "# Skill: {}\n\n{}{}",
            skill.name,
            skill.body.trim_end(),
            files_section
        );

        // spinner.finish_success(&format!("skill '{}' loaded", name));
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fake_skill(name: &str, body: &str) -> Skill {
        Skill {
            name: name.to_string(),
            description: "desc".to_string(),
            dir: PathBuf::from("/nonexistent"),
            body: body.to_string(),
            source_file: PathBuf::from("/nonexistent/SKILL.md"),
        }
    }

    #[tokio::test]
    async fn returns_skill_body() {
        let skills = Arc::new(vec![fake_skill("demo", "do the thing")]);
        let tool = SkillTool::new(skills);
        let out = tool.execute(json!({"name": "demo"})).await.unwrap();
        assert!(out.contains("# Skill: demo"));
        assert!(out.contains("do the thing"));
    }

    #[tokio::test]
    async fn unknown_skill_errors() {
        let skills = Arc::new(vec![fake_skill("demo", "x")]);
        let tool = SkillTool::new(skills);
        let err = tool
            .execute(json!({"name": "missing"}))
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("unknown skill"));
        assert!(err.contains("demo"));
    }

    #[tokio::test]
    async fn missing_name_errors() {
        let skills = Arc::new(vec![]);
        let tool = SkillTool::new(skills);
        let err = tool.execute(json!({})).await.unwrap_err().to_string();
        assert!(err.contains("missing 'name'"));
    }

    #[test]
    fn definition_enum_only_when_skills_present() {
        let empty = SkillTool::new(Arc::new(vec![]));
        let def = empty.definition();
        assert!(def.parameters["properties"]["name"].get("enum").is_none());

        let full = SkillTool::new(Arc::new(vec![fake_skill("a", ""), fake_skill("b", "")]));
        let def = full.definition();
        let names = def.parameters["properties"]["name"]["enum"]
            .as_array()
            .unwrap();
        assert_eq!(names.len(), 2);
    }
}