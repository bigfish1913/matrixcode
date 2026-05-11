//! Skill discovery and loading.
//!
//! A "skill" is a directory containing a `SKILL.md` file with a YAML
//! frontmatter block. The frontmatter declares at least a `name` and a
//! `description`; the body contains instructions the model should follow
//! when the skill is active. Additional files in the skill directory
//! (scripts, templates, reference docs) can be read on demand by the
//! normal `read` tool.
//!
//! Skills are surfaced to the model in two stages:
//! 1. At startup every skill's name + description is injected into the
//!    system prompt so the model knows what's available.
//! 2. The `skill` tool loads the full `SKILL.md` body (and lists the
//!    skill's files) when the model decides to use one. This keeps
//!    baseline token cost low even with many skills installed.
//!
//! Directory layout:
//! ```text
//! skills/
//!   my-skill/
//!     SKILL.md            # required, with frontmatter
//!     helper.py           # optional support files
//!     templates/...       # optional subdirs
//! ```

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// A loaded skill ready to be advertised to the model.
#[derive(Debug, Clone)]
pub struct Skill {
    /// Canonical identifier, taken from frontmatter `name`. Used as the
    /// argument to the `skill` tool.
    pub name: String,
    /// Short one-line description shown in the system prompt.
    pub description: String,
    /// Absolute path to the skill directory.
    pub dir: PathBuf,
    /// Full markdown body (without frontmatter) of `SKILL.md`.
    pub body: String,
}

impl Skill {
    /// Path to this skill's `SKILL.md`.
    pub fn skill_md(&self) -> PathBuf {
        self.dir.join("SKILL.md")
    }
}

/// Walk the given roots and load every `SKILL.md` found one level deep.
/// Missing roots are silently skipped so users can keep a personal
/// `~/.code-agent/skills` directory without the project-local one (or
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
            if !path.is_dir() {
                continue;
            }
            let md = path.join("SKILL.md");
            if !md.is_file() {
                continue;
            }
            match load_skill(&path) {
                Ok(skill) => {
                    if out.iter().any(|s| s.name == skill.name) {
                        eprintln!(
                            "[warn] duplicate skill name '{}' at {} (ignored)",
                            skill.name,
                            path.display()
                        );
                        continue;
                    }
                    out.push(skill);
                }
                Err(e) => {
                    eprintln!("[warn] skipping skill at {}: {e}", path.display());
                }
            }
        }
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Load a single skill directory. Public so tests and the `skill` tool
/// can reload a specific skill without rescanning everything.
pub fn load_skill(dir: &Path) -> Result<Skill> {
    let md_path = dir.join("SKILL.md");
    let raw = std::fs::read_to_string(&md_path)
        .with_context(|| format!("reading {}", md_path.display()))?;
    let (front, body) = split_frontmatter(&raw)
        .with_context(|| format!("parsing frontmatter of {}", md_path.display()))?;

    let name = front
        .get("name")
        .cloned()
        .filter(|s| !s.is_empty())
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

    Ok(Skill {
        name,
        description,
        dir: dir.to_path_buf(),
        body: body.to_string(),
    })
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
    let rest = rest.strip_prefix('\n').or_else(|| rest.strip_prefix("\r\n"));
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

/// Render the skills catalogue that gets appended to the system prompt.
/// Returns `None` if there are no skills, so callers can skip injection
/// entirely rather than bolting on an empty section.
pub fn format_catalogue(skills: &[Skill]) -> Option<String> {
    if skills.is_empty() {
        return None;
    }
    let mut s = String::from(
        "\n\nAvailable skills (use the `skill` tool with the skill's name to load its full instructions):\n",
    );
    for sk in skills {
        s.push_str(&format!("- {}: {}\n", sk.name, sk.description));
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
        } else if file_type.is_file() {
            if let Ok(rel) = p.strip_prefix(root) {
                out.push(rel.display().to_string());
            }
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
    fn catalogue_renders_or_skips() {
        assert!(format_catalogue(&[]).is_none());
        let s = Skill {
            name: "demo".into(),
            description: "does stuff".into(),
            dir: PathBuf::from("/tmp"),
            body: String::new(),
        };
        let cat = format_catalogue(&[s]).unwrap();
        assert!(cat.contains("demo: does stuff"));
    }
}
