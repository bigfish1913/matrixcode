//! Ignore pattern matching for CodeGraph watcher.

use std::path::Path;

/// Default ignore patterns for file watching.
pub const DEFAULT_IGNORE_PATTERNS: &[&str] = &[
    "target",
    "dist",
    "build",
    "out",
    "bin",
    "obj",
    ".output",
    "node_modules",
    "vendor",
    "Pods",
    ".venv",
    "venv",
    "__pycache__",
    ".cache",
    ".tmp",
    ".temp",
    "tmp",
    "temp",
    ".idea",
    ".vscode",
    ".eclipse",
    ".project",
    ".classpath",
    ".generated",
    "generated",
    ".codegraph",
    "package-lock.json",
    "yarn.lock",
    "Cargo.lock",
    "pnpm-lock.yaml",
    "coverage",
    ".nyc_output",
    "test-results",
    "logs",
];

/// Extensions to watch (source files only).
pub const WATCH_EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "mjs", "py", "go", "java", "kt", "kts", "c", "cpp", "cc", "h",
    "hpp", "rb", "php", "swift", "cs", "scala", "lua", "sh",
];

/// Gitignore patterns loaded from file.
pub struct IgnoreMatcher {
    patterns: Vec<String>,
    negation_patterns: Vec<String>,
}

impl IgnoreMatcher {
    /// Load ignore patterns from .gitignore and defaults.
    pub fn load(project_path: &Path) -> Self {
        let mut patterns = Vec::new();
        let mut negation_patterns = Vec::new();

        // Add default patterns
        for p in DEFAULT_IGNORE_PATTERNS {
            patterns.push(p.to_string());
        }

        // Load .gitignore
        let gitignore_path = project_path.join(".gitignore");
        if gitignore_path.exists()
            && let Ok(content) = std::fs::read_to_string(&gitignore_path) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if let Some(stripped) = line.strip_prefix('!') {
                        negation_patterns.push(stripped.to_string());
                    } else {
                        patterns.push(line.to_string());
                    }
                }
            }

        Self {
            patterns,
            negation_patterns,
        }
    }

    /// Check if a path should be ignored.
    pub fn should_ignore(&self, path: &Path, project_path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        let relative_path = path
            .strip_prefix(project_path)
            .unwrap_or(path)
            .to_string_lossy();

        // Check negation patterns first (explicit inclusion)
        for pattern in &self.negation_patterns {
            if Self::matches_pattern(&relative_path, pattern) {
                return false;
            }
        }

        // Check ignore patterns
        for pattern in &self.patterns {
            if Self::matches_pattern(&relative_path, pattern) || path_str.contains(pattern) {
                return true;
            }
        }

        // Check hidden files (but allow .codegraph)
        for component in path.components() {
            if let std::path::Component::Normal(name) = component {
                let name_str = name.to_string_lossy();
                if name_str.starts_with('.')
                    && name_str != ".codegraph"
                    && !WATCH_EXTENSIONS.contains(&name_str.split('.').next_back().unwrap_or(""))
                {
                    return true;
                }
            }
        }

        false
    }

    /// Check if path matches a gitignore pattern.
    fn matches_pattern(path: &str, pattern: &str) -> bool {
        let pattern = pattern.trim_start_matches('/');

        // Directory match (pattern ends with /)
        if let Some(dir_pattern) = pattern.strip_suffix('/') {
            return path.contains(dir_pattern) || path.starts_with(dir_pattern);
        }

        // Wildcard match
        if pattern.contains('*') {
            let parts = pattern.split('*').collect::<Vec<_>>();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];
                return (prefix.is_empty() || path.starts_with(prefix))
                    && (suffix.is_empty() || path.ends_with(suffix));
            }
        }

        // Exact match or contains
        path == pattern || path.contains(pattern) || path.starts_with(&format!("{}/", pattern))
    }
}
