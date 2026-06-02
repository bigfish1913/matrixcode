//! Skill discovery and loading.
//!
//! A "skill" is a markdown file with a YAML frontmatter block, or a directory
//! containing skill files. Two formats are supported:
//!
//! **Format 1: Single-file skill (SKILL.md)**
//! ```text
//! skills/
//!   my-skill/
//!     SKILL.md            # required, with frontmatter
//!     helper.py           # optional support files
//!     templates/...       # optional subdirs
//! ```
//!
//! **Format 2: Multi-file skills (like Claude Code plugins)**
//! ```text
//! skills/
//!   om/
//!     debug.md            # each .md file is a separate skill
//!     feature.md
//!     plan.md
//!     ...
//! ```
//!
//! Skills are surfaced to the model in two stages:
//! 1. At startup every skill's name + description is injected into the
//!    system prompt so the model knows what's available.
//! 2. The `skill` tool loads the full skill body (and lists the
//!    skill's files) when the model decides to use one. This keeps
//!    baseline token cost low even with many skills installed.
//!
//! ## Skill Priority and Type
//!
//! Skills have priority levels that determine invocation order:
//! - **Process** (priority 1): brainstorming, debugging, planning
//! - **Implementation** (priority 2): frontend-design, mcp-builder, code-review
//!
//! Skills also have types that determine flexibility:
//! - **Rigid**: Must be followed exactly (TDD, debugging workflows)
//! - **Flexible**: Can be adapted to context (patterns, style guides)

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Skill type determines how strictly the skill should be followed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SkillType {
    /// Rigid skill - must be followed exactly (TDD, debugging)
    Rigid,
    /// Flexible skill - can be adapted to context (patterns)
    #[default]
    Flexible,
}

impl SkillType {
    /// Parse from string
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "rigid" => Self::Rigid,
            "flexible" => Self::Flexible,
            _ => Self::default(),
        }
    }

    /// Convert to string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rigid => "rigid",
            Self::Flexible => "flexible",
        }
    }
}

/// Skill priority determines invocation order.
/// Lower number = higher priority (processed first).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum SkillPriority {
    /// Process skills (brainstorming, debugging, planning) - highest priority
    Process = 1,
    /// Implementation skills (frontend, mcp-builder) - second priority
    #[default]
    Implementation = 2,
}

impl SkillPriority {
    /// Parse from string
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "process" => Self::Process,
            "implementation" => Self::Implementation,
            _ => Self::default(),
        }
    }

    /// Convert to string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Process => "process",
            Self::Implementation => "implementation",
        }
    }

    /// Get display label for prompt
    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Process => "Process",
            Self::Implementation => "Implementation",
        }
    }
}

/// A loaded skill ready to be advertised to the model.
#[derive(Debug, Clone, PartialEq)]
pub struct Skill {
    /// Canonical identifier, taken from frontmatter `name` or file name.
    /// Used as the argument to the `skill` tool.
    pub name: String,
    /// Short one-line description shown in the system prompt.
    pub description: String,
    /// Trigger conditions (optional). When to use this skill.
    /// Example: "代码审查请求"、"用户说 'review'、"修改多个文件后"
    pub trigger: Option<String>,
    /// Skill type: Rigid (must follow exactly) or Flexible (adapt to context)
    pub skill_type: SkillType,
    /// Skill priority: Process (first) or Implementation (second)
    pub priority: SkillPriority,
    /// Whether this skill is mandatory to invoke when triggered
    pub mandatory: bool,
    /// Absolute path to the skill directory.
    pub dir: PathBuf,
    /// Full markdown body (without frontmatter).
    pub body: String,
    /// The source .md file path (SKILL.md or other .md file).
    pub source_file: PathBuf,
}

impl Skill {
    /// Path to this skill's source file.
    pub fn skill_md(&self) -> PathBuf {
        self.source_file.clone()
    }
}

/// Walk the given roots and load skills from all supported formats:
/// - Format 1: directories with `SKILL.md` (backward compatible)
/// - Format 2: directories with multiple `.md` files (Claude Code style)
/// - Format 3: standalone `.md` files directly in the root directory
///
/// Missing roots are silently skipped so users can keep a personal
/// `~/.matrix/skills` directory without the project-local one (or
/// vice versa). Skills with duplicate names: first one wins, later ones
/// are dropped with a stderr warning so precedence is predictable.
pub fn discover_skills(roots: &[PathBuf]) -> Vec<Skill> {
    let mut out: Vec<Skill> = Vec::new();

    for root in roots {
        if !root.is_dir() {
            continue;
        }
        let entries = match std::fs::read_dir(root) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("[warn] could not read skills dir {}: {e}", root.display());
                continue;
            }
        };
        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_dir() {
                // Try Format 1: SKILL.md exists (package index)
                let skill_md = path.join("SKILL.md");
                if skill_md.is_file() {
                    match load_skill_from_file(&skill_md, &path) {
                        Ok(skill) => {
                            add_skill(&mut out, skill);
                        }
                        Err(e) => {
                            eprintln!("[warn] skipping skill at {}: {e}", path.display());
                        }
                    }
                    // Don't continue here - also load other .md files in the directory
                    // (SKILL.md acts as package index, other .md files are individual skills)
                }

                // Try Format 2: multiple .md files in directory (including alongside SKILL.md)
                load_multi_file_skills(&path, &mut out);
            } else if path.is_file() {
                // Format 3: standalone .md file directly in root
                let ext = path.extension().and_then(|e| e.to_str());
                if ext != Some("md") {
                    continue;
                }
                match load_skill_from_file(&path, root) {
                    Ok(skill) => {
                        add_skill(&mut out, skill);
                    }
                    Err(e) => {
                        let raw = std::fs::read_to_string(&path).unwrap_or_default();
                        if raw.trim_start().starts_with("---") {
                            eprintln!("[warn] skipping skill file {}: {e}", path.display());
                        }
                    }
                }
            }
        }
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Add a skill to the list, checking for duplicates.
fn add_skill(out: &mut Vec<Skill>, skill: Skill) {
    if out.iter().any(|s| s.name == skill.name) {
        eprintln!(
            "[warn] duplicate skill name '{}' at {} (ignored)",
            skill.name,
            skill.source_file.display()
        );
        return;
    }
    out.push(skill);
}

/// Load skills from a directory containing multiple .md files.
/// Each .md file with frontmatter becomes a separate skill.
/// Note: SKILL.md is skipped here (handled separately as package index),
/// but other .md files are loaded even when SKILL.md exists.
fn load_multi_file_skills(dir: &Path, out: &mut Vec<Skill>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("[warn] could not read skill dir {}: {e}", dir.display());
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        // Only process .md files
        let ext = path.extension().and_then(|e| e.to_str());
        if ext != Some("md") {
            continue;
        }

        // Skip SKILL.md (already handled in Format 1)
        if path.file_name().and_then(|n| n.to_str()) == Some("SKILL.md") {
            continue;
        }

        match load_skill_from_file(&path, dir) {
            Ok(skill) => {
                add_skill(out, skill);
            }
            Err(e) => {
                // Only warn if the file has frontmatter (indicating it's meant to be a skill)
                let raw = std::fs::read_to_string(&path).unwrap_or_default();
                if raw.trim_start().starts_with("---") {
                    eprintln!("[warn] skipping skill file {}: {e}", path.display());
                }
            }
        }
    }
}

/// Load a skill from a specific .md file.
/// Public so tests and the `skill` tool can reload a specific skill.
pub fn load_skill_from_file(md_path: &Path, dir: &Path) -> Result<Skill> {
    let raw = std::fs::read_to_string(md_path)
        .with_context(|| format!("reading {}", md_path.display()))?;
    let (front, body) = split_frontmatter(&raw)
        .with_context(|| format!("parsing frontmatter of {}", md_path.display()))?;

    // Name: frontmatter > filename (without .md) > directory name
    let name = front
        .get("name")
        .cloned()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            md_path
                .file_stem()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
        })
        .or_else(|| {
            dir.file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
        })
        .ok_or_else(|| anyhow::anyhow!("skill has no 'name' in frontmatter"))?;

    let description = front
        .get("description")
        .cloned()
        .unwrap_or_else(|| "(no description)".to_string());

    // Extract trigger field (optional)
    let trigger = front.get("trigger").cloned();

    // Extract skill_type (optional, default to Flexible)
    let skill_type = front
        .get("type")
        .map(|s| SkillType::from_str(s))
        .unwrap_or_default();

    // Extract priority (optional, default to Implementation)
    let priority = front
        .get("priority")
        .map(|s| SkillPriority::from_str(s))
        .unwrap_or_default();

    // Extract mandatory flag (optional, default to false)
    let mandatory = front
        .get("mandatory")
        .map(|s| s.trim().to_lowercase() == "true")
        .unwrap_or(false);

    Ok(Skill {
        name,
        description,
        trigger,
        skill_type,
        priority,
        mandatory,
        dir: dir.to_path_buf(),
        body: body.to_string(),
        source_file: md_path.to_path_buf(),
    })
}

/// Load a skill from a directory with SKILL.md (backward compatible).
pub fn load_skill(dir: &Path) -> Result<Skill> {
    let md_path = dir.join("SKILL.md");
    load_skill_from_file(&md_path, dir)
}

/// Minimal YAML-frontmatter parser: supports the shape
/// ```text
/// ---
/// name: foo
/// description: bar baz
/// ---
/// body...
/// ```
/// Only flat `key: value` pairs are recognised. Values may be wrapped
/// in matching single or double quotes. Missing frontmatter is treated
/// as empty (body == whole file), which keeps bare markdown files from
/// being rejected outright — though they'll fall back to the directory
/// name and generic description.
fn split_frontmatter(raw: &str) -> Result<(std::collections::BTreeMap<String, String>, &str)> {
    let mut front = std::collections::BTreeMap::new();

    let trimmed = raw.trim_start_matches('\u{feff}'); // strip BOM if any
    let Some(rest) = trimmed.strip_prefix("---") else {
        return Ok((front, trimmed));
    };
    // Require newline right after opening ---
    let rest = rest
        .strip_prefix('\n')
        .or_else(|| rest.strip_prefix("\r\n"));
    let Some(rest) = rest else {
        return Ok((front, trimmed));
    };

    // Find closing --- on its own line.
    let mut end_idx: Option<usize> = None;
    let mut cursor = 0usize;
    for line in rest.split_inclusive('\n') {
        let trimmed_line = line.trim_end_matches(['\n', '\r']);
        if trimmed_line == "---" {
            end_idx = Some(cursor + line.len());
            break;
        }
        cursor += line.len();
    }
    let Some(end) = end_idx else {
        // No closing delimiter — treat whole file as body, no frontmatter.
        return Ok((front, trimmed));
    };

    let front_block = &rest[..cursor];
    let body = rest[end..].trim_start_matches(['\n', '\r']);

    for line in front_block.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let key = k.trim().to_string();
        let val = unquote(v.trim());
        if !key.is_empty() {
            front.insert(key, val);
        }
    }

    Ok((front, body))
}

fn unquote(s: &str) -> String {
    let bytes = s.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        return s[1..s.len() - 1].to_string();
    }
    s.to_string()
}

/// Render the skills catalogue body for insertion into a prompt section.
/// Returns `None` if there are no skills, so callers can skip injection
/// entirely rather than bolting on an empty section.
///
/// Skills are sorted by priority (Process first, then Implementation).
/// Mandatory skills are marked with ⚠️ indicator.
pub fn format_catalogue(skills: &[Skill]) -> Option<String> {
    if skills.is_empty() {
        return None;
    }

    // Sort by priority (Process first)
    let mut sorted_skills = skills.to_vec();
    sorted_skills.sort_by_key(|s| s.priority);

    let mut s = String::from(
        "Use the `skill` tool with the skill's name to load its full instructions.\n\n",
    );

    // Group by priority
    let process_skills: Vec<_> = sorted_skills.iter().filter(|s| s.priority == SkillPriority::Process).collect();
    let impl_skills: Vec<_> = sorted_skills.iter().filter(|s| s.priority == SkillPriority::Implementation).collect();

    if !process_skills.is_empty() {
        s.push_str("**Process Skills** (invoke first for workflow guidance):\n");
        for sk in process_skills {
            let mandatory_marker = if sk.mandatory { "⚠️ " } else { "" };
            let type_marker = if sk.skill_type == SkillType::Rigid { "[rigid] " } else { "" };
            s.push_str(&format!(
                "- {}{}{}: {}\n",
                mandatory_marker, type_marker, sk.name, sk.description
            ));
        }
        s.push_str("\n");
    }

    if !impl_skills.is_empty() {
        s.push_str("**Implementation Skills** (invoke after process skills):\n");
        for sk in impl_skills {
            let mandatory_marker = if sk.mandatory { "⚠️ " } else { "" };
            let type_marker = if sk.skill_type == SkillType::Rigid { "[rigid] " } else { "" };
            s.push_str(&format!(
                "- {}{}{}: {}\n",
                mandatory_marker, type_marker, sk.name, sk.description
            ));
        }
    }

    Some(s)
}

/// List files inside a skill directory (recursive), relative to the
/// skill root. Used by the `skill` tool so the model knows what
/// supporting files it can `read` next.
pub fn list_skill_files(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

fn walk(root: &Path, cur: &Path, out: &mut Vec<String>) {
    let entries = match std::fs::read_dir(cur) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if file_type.is_dir() {
            walk(root, &p, out);
        } else if file_type.is_file()
            && let Ok(rel) = p.strip_prefix(root)
        {
            out.push(rel.display().to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_file(path: &Path, body: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    #[test]
    fn parses_basic_frontmatter() {
        let (front, body) =
            split_frontmatter("---\nname: foo\ndescription: hi there\n---\nbody text\n").unwrap();
        assert_eq!(front.get("name").unwrap(), "foo");
        assert_eq!(front.get("description").unwrap(), "hi there");
        assert_eq!(body, "body text\n");
    }

    #[test]
    fn quoted_values_are_unwrapped() {
        let (front, _) =
            split_frontmatter("---\nname: 'foo bar'\ndescription: \"baz\"\n---\nx").unwrap();
        assert_eq!(front.get("name").unwrap(), "foo bar");
        assert_eq!(front.get("description").unwrap(), "baz");
    }

    #[test]
    fn missing_frontmatter_returns_whole_body() {
        let (front, body) = split_frontmatter("just markdown\nno front").unwrap();
        assert!(front.is_empty());
        assert_eq!(body, "just markdown\nno front");
    }

    #[test]
    fn unclosed_frontmatter_falls_back_to_body() {
        let (front, body) = split_frontmatter("---\nname: foo\nbody without close").unwrap();
        assert!(front.is_empty());
        assert!(body.starts_with("---"));
    }

    #[test]
    fn discover_loads_skill_directory() {
        let tmp = tempdir().unwrap();
        let root = tmp.path().join("skills");
        write_file(
            &root.join("greet/SKILL.md"),
            "---\nname: greet\ndescription: say hi\n---\nSay hello to the user.\n",
        );
        write_file(&root.join("greet/extra.txt"), "support file");

        let skills = discover_skills(&[root]);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "greet");
        assert_eq!(skills[0].description, "say hi");
        assert!(skills[0].body.contains("Say hello"));
        let files = list_skill_files(&skills[0].dir);
        assert!(files.iter().any(|f| f == "SKILL.md"));
        assert!(files.iter().any(|f| f == "extra.txt"));
    }

    #[test]
    fn discover_loads_multi_file_skills() {
        let tmp = tempdir().unwrap();
        let root = tmp.path().join("skills");
        write_file(
            &root.join("om/debug.md"),
            "---\nname: debug\ndescription: debug issues\n---\nDebug workflow.\n",
        );
        write_file(
            &root.join("om/feature.md"),
            "---\nname: feature\ndescription: build features\n---\nFeature workflow.\n",
        );

        let skills = discover_skills(&[root]);
        assert_eq!(skills.len(), 2);

        let debug_skill = skills.iter().find(|s| s.name == "debug").unwrap();
        assert_eq!(debug_skill.description, "debug issues");
        assert!(debug_skill.body.contains("Debug workflow"));

        let feature_skill = skills.iter().find(|s| s.name == "feature").unwrap();
        assert_eq!(feature_skill.description, "build features");
        assert!(feature_skill.body.contains("Feature workflow"));
    }

    #[test]
    fn multi_file_skill_name_from_filename() {
        let tmp = tempdir().unwrap();
        let root = tmp.path().join("skills");
        // No 'name' in frontmatter - should use filename
        write_file(
            &root.join("utils/helper.md"),
            "---\ndescription: a helper\n---\nHelper content.\n",
        );

        let skills = discover_skills(&[root]);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "helper");
    }

    #[test]
    fn duplicate_names_are_dropped() {
        let tmp = tempdir().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        write_file(
            &a.join("x/SKILL.md"),
            "---\nname: x\ndescription: first\n---\nA\n",
        );
        write_file(
            &b.join("x/SKILL.md"),
            "---\nname: x\ndescription: second\n---\nB\n",
        );
        let skills = discover_skills(&[a, b]);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].description, "first");
    }

    #[test]
    fn missing_root_is_skipped() {
        let skills = discover_skills(&[PathBuf::from("/definitely/not/here")]);
        assert!(skills.is_empty());
    }

    #[test]
    fn discover_loads_standalone_md_files() {
        let tmp = tempdir().unwrap();
        let root = tmp.path().join("skills");
        // Standalone .md file directly in root
        write_file(
            &root.join("om.md"),
            "---\nname: om\ndescription: main entry\n---\nOpenMatrix entry point.\n",
        );
        write_file(
            &root.join("openmatrix.md"),
            "---\nname: openmatrix\ndescription: detect dev tasks\n---\nDetect development tasks.\n",
        );

        let skills = discover_skills(&[root]);
        assert_eq!(skills.len(), 2);

        let om = skills.iter().find(|s| s.name == "om").unwrap();
        assert_eq!(om.description, "main entry");
        assert!(om.body.contains("OpenMatrix entry point"));

        let openmatrix = skills.iter().find(|s| s.name == "openmatrix").unwrap();
        assert_eq!(openmatrix.description, "detect dev tasks");
    }

    #[test]
    fn discover_mixed_formats() {
        let tmp = tempdir().unwrap();
        let root = tmp.path().join("skills");
        // Format 1: directory with SKILL.md
        write_file(
            &root.join("debug/SKILL.md"),
            "---\nname: debug\ndescription: debug tool\n---\nDebug.\n",
        );
        // Format 2: directory with multiple .md files
        write_file(
            &root.join("om/feature.md"),
            "---\nname: om:feature\ndescription: build features\n---\nFeature.\n",
        );
        // Format 3: standalone .md file
        write_file(
            &root.join("openmatrix.md"),
            "---\nname: openmatrix\ndescription: detect tasks\n---\nDetect.\n",
        );

        let skills = discover_skills(&[root]);
        assert_eq!(skills.len(), 3);
        assert!(skills.iter().any(|s| s.name == "debug"));
        assert!(skills.iter().any(|s| s.name == "om:feature"));
        assert!(skills.iter().any(|s| s.name == "openmatrix"));
    }

    #[test]
    fn catalogue_renders_or_skips() {
        assert!(format_catalogue(&[]).is_none());
        let s = Skill {
            name: "demo".into(),
            description: "does stuff".into(),
            trigger: None,
            skill_type: SkillType::Flexible,
            priority: SkillPriority::Implementation,
            mandatory: false,
            dir: PathBuf::from("/tmp"),
            body: String::new(),
            source_file: PathBuf::from("/tmp/demo.md"),
        };
        let cat = format_catalogue(&[s]).unwrap();
        assert!(cat.contains("Use the `skill` tool"));
        assert!(cat.contains("demo: does stuff"));
        assert!(cat.contains("Implementation Skills"));
    }

    #[test]
    fn catalogue_groups_by_priority() {
        let process_skill = Skill {
            name: "brainstorm".into(),
            description: "brainstorm ideas".into(),
            trigger: None,
            skill_type: SkillType::Flexible,
            priority: SkillPriority::Process,
            mandatory: false,
            dir: PathBuf::from("/tmp"),
            body: String::new(),
            source_file: PathBuf::from("/tmp/brainstorm.md"),
        };
        let impl_skill = Skill {
            name: "frontend".into(),
            description: "frontend design".into(),
            trigger: None,
            skill_type: SkillType::Rigid,
            priority: SkillPriority::Implementation,
            mandatory: true,
            dir: PathBuf::from("/tmp"),
            body: String::new(),
            source_file: PathBuf::from("/tmp/frontend.md"),
        };
        let cat = format_catalogue(&[impl_skill, process_skill]).unwrap();
        // Process skills should appear first
        let process_idx = cat.find("Process Skills").unwrap();
        let impl_idx = cat.find("Implementation Skills").unwrap();
        assert!(process_idx < impl_idx, "Process skills should come before Implementation");
        // Mandatory marker should appear
        assert!(cat.contains("⚠️"), "Mandatory skill should have marker");
        // Rigid marker should appear
        assert!(cat.contains("[rigid]"), "Rigid skill should have marker");
    }

    #[test]
    fn skill_type_parsing() {
        assert_eq!(SkillType::from_str("rigid"), SkillType::Rigid);
        assert_eq!(SkillType::from_str("RIGID"), SkillType::Rigid);
        assert_eq!(SkillType::from_str("flexible"), SkillType::Flexible);
        assert_eq!(SkillType::from_str("unknown"), SkillType::Flexible);
    }

    #[test]
    fn skill_priority_parsing() {
        assert_eq!(SkillPriority::from_str("process"), SkillPriority::Process);
        assert_eq!(SkillPriority::from_str("PROCESS"), SkillPriority::Process);
        assert_eq!(SkillPriority::from_str("implementation"), SkillPriority::Implementation);
        assert_eq!(SkillPriority::from_str("unknown"), SkillPriority::Implementation);
    }

    #[test]
    fn skill_priority_ordering() {
        assert!(SkillPriority::Process < SkillPriority::Implementation);
    }
}
