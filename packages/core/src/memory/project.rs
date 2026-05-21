//! Project structure analysis for automatic memory creation.

use std::path::PathBuf;
use std::fs;

use super::storage::MemoryStorage;
use super::types::{MemoryCategory, MemoryEntry};

// ============================================================================
// Project Type Configuration
// ============================================================================

/// Project type detection configuration.
pub struct ProjectTypeConfig {
    pub type_name: &'static str,
    pub detect_files: &'static [&'static str],
    pub entry_files: &'static [&'static str],
    pub key_dirs: &'static [&'static str],
    pub tech_stack: &'static str,
}

pub const PROJECT_TYPE_CONFIGS: &[ProjectTypeConfig] = &[
    ProjectTypeConfig {
        type_name: "Rust",
        detect_files: &["Cargo.toml"],
        entry_files: &["src/main.rs", "src/lib.rs"],
        key_dirs: &["src", "tests", "examples"],
        tech_stack: "Rust",
    },
    ProjectTypeConfig {
        type_name: "Node.js",
        detect_files: &["package.json"],
        entry_files: &["index.js", "src/index.js", "app.js"],
        key_dirs: &["src", "lib", "components", "pages"],
        tech_stack: "Node.js",
    },
    ProjectTypeConfig {
        type_name: "TypeScript",
        detect_files: &["tsconfig.json", "package.json"],
        entry_files: &["src/index.ts", "src/main.ts"],
        key_dirs: &["src", "lib", "components"],
        tech_stack: "TypeScript",
    },
    ProjectTypeConfig {
        type_name: "React",
        detect_files: &["package.json"],
        entry_files: &["src/index.tsx", "src/App.tsx"],
        key_dirs: &["src/components", "src/pages", "src/hooks"],
        tech_stack: "React + TypeScript",
    },
    ProjectTypeConfig {
        type_name: "Vue",
        detect_files: &["vue.config.js", "vite.config.js", "package.json"],
        entry_files: &["src/main.ts", "src/App.vue"],
        key_dirs: &["src/components", "src/views", "src/stores"],
        tech_stack: "Vue.js",
    },
    ProjectTypeConfig {
        type_name: "Python",
        detect_files: &["requirements.txt", "setup.py", "pyproject.toml"],
        entry_files: &["main.py", "app.py", "__main__.py"],
        key_dirs: &["src", "lib", "tests", "app"],
        tech_stack: "Python",
    },
    ProjectTypeConfig {
        type_name: "Go",
        detect_files: &["go.mod"],
        entry_files: &["main.go", "cmd/main.go"],
        key_dirs: &["cmd", "pkg", "internal", "api"],
        tech_stack: "Go",
    },
    ProjectTypeConfig {
        type_name: "Java",
        detect_files: &["pom.xml", "build.gradle"],
        entry_files: &["src/main/java/Main.java"],
        key_dirs: &["src/main/java", "src/test/java"],
        tech_stack: "Java",
    },
];

pub const IGNORE_DIRS: &[&str] = &[
    ".git", ".github", ".matrix", ".idea", ".vscode",
    "node_modules", "target", "build", "dist", "out",
    "vendor", "__pycache__", ".venv", "venv", "env",
];

// ============================================================================
// Project Structure Analyzer
// ============================================================================

pub struct ProjectStructureAnalyzer {
    project_root: PathBuf,
}

impl ProjectStructureAnalyzer {
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    pub fn detect_project_type(&self) -> Option<&'static ProjectTypeConfig> {
        for config in PROJECT_TYPE_CONFIGS {
            for detect_file in config.detect_files {
                if detect_file.starts_with('*') {
                    let extension = detect_file.trim_start_matches('*');
                    if let Ok(entries) = fs::read_dir(&self.project_root) {
                        for entry in entries.flatten() {
                            if entry.file_name().to_string_lossy().ends_with(extension) {
                                return Some(config);
                            }
                        }
                    }
                } else {
                    let path = self.project_root.join(detect_file);
                    if path.exists() {
                        return Some(config);
                    }
                }
            }
        }
        None
    }

    pub fn find_entry_file(&self, config: &ProjectTypeConfig) -> Option<String> {
        for entry_file in config.entry_files {
            let path = self.project_root.join(entry_file);
            if path.exists() {
                return Some(entry_file.to_string());
            }
        }
        None
    }

    pub fn scan_key_directories(&self, config: &ProjectTypeConfig) -> Vec<(String, String)> {
        let mut dirs_info: Vec<(String, String)> = Vec::new();

        for key_dir in config.key_dirs {
            let path = self.project_root.join(key_dir);
            if path.exists() && path.is_dir() {
                let purpose = self.infer_directory_purpose(key_dir);
                dirs_info.push((key_dir.to_string(), purpose));
            }
        }

        dirs_info
    }

    fn infer_directory_purpose(&self, dir_name: &str) -> String {
        let dir_lower = dir_name.to_lowercase();

        if dir_lower.contains("component") { return "组件目录".to_string(); }
        if dir_lower.contains("page") || dir_lower == "views" { return "页面目录".to_string(); }
        if dir_lower.contains("hook") { return "Hook目录".to_string(); }
        if dir_lower.contains("util") || dir_lower == "lib" { return "工具目录".to_string(); }
        if dir_lower.contains("test") { return "测试目录".to_string(); }
        if dir_lower.contains("api") { return "API目录".to_string(); }
        if dir_lower.contains("model") { return "模型目录".to_string(); }
        if dir_lower.contains("service") { return "服务目录".to_string(); }

        "代码目录".to_string()
    }

    pub fn generate_memories(&self) -> Vec<MemoryEntry> {
        let mut entries: Vec<MemoryEntry> = Vec::new();

        let config = match self.detect_project_type() {
            Some(c) => c,
            None => return entries,
        };

        // Tech stack
        entries.push(MemoryEntry::new(
            MemoryCategory::Technical,
            format!("项目技术栈: {}", config.tech_stack),
            None,
        ));

        // Entry file
        if let Some(entry) = self.find_entry_file(config) {
            entries.push(MemoryEntry::new(
                MemoryCategory::Structure,
                format!("入口文件: {}", entry),
                None,
            ));
        }

        // Key directories
        for (dir, purpose) in self.scan_key_directories(config) {
            entries.push(MemoryEntry::new(
                MemoryCategory::Structure,
                format!("{} 是 {}", dir, purpose),
                None,
            ));
        }

        entries
    }
}

/// Generate project structure memories and save.
pub fn generate_project_structure_memories(project_root: &std::path::Path, storage: &mut MemoryStorage) -> usize {
    let analyzer = ProjectStructureAnalyzer::new(project_root.to_path_buf());
    let entries = analyzer.generate_memories();

    let count = entries.len();
    for entry in entries {
        if let Err(e) = storage.add_entry(entry, true) {
            log::warn!("Failed to save project memory: {}", e);
        }
    }

    count
}