//! Verification suggestion system for code changes.
//!
//! This module provides automatic detection of project types and
//! suggests relevant verification commands (tests, builds, type checks)
//! after file modifications.

use std::path::{Path, PathBuf};

/// Supported project types for verification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum ProjectType {
    /// Rust project (Cargo.toml)
    Rust,
    /// Node.js project (package.json)
    NodeJs,
    /// Python project (pyproject.toml or requirements.txt)
    Python,
    /// Go project (go.mod)
    Go,
    /// Java/Kotlin project (pom.xml or build.gradle)
    Java,
    /// Unknown or unsupported project type
    #[default]
    Unknown,
}


impl ProjectType {
    /// Returns the test command for this project type
    pub fn test_command(&self) -> Option<&'static str> {
        match self {
            ProjectType::Rust => Some("cargo test"),
            ProjectType::NodeJs => Some("npm test"),
            ProjectType::Python => Some("pytest"),
            ProjectType::Go => Some("go test ./..."),
            ProjectType::Java => Some("mvn test"),
            ProjectType::Unknown => None,
        }
    }

    /// Returns the build command for this project type
    pub fn build_command(&self) -> Option<&'static str> {
        match self {
            ProjectType::Rust => Some("cargo build"),
            ProjectType::NodeJs => Some("npm run build"),
            ProjectType::Python => None, // Python doesn't have a standard build command
            ProjectType::Go => Some("go build"),
            ProjectType::Java => Some("mvn compile"),
            ProjectType::Unknown => None,
        }
    }

    /// Returns the type check command for this project type
    pub fn typecheck_command(&self) -> Option<&'static str> {
        match self {
            ProjectType::Rust => Some("cargo check"),
            ProjectType::NodeJs => Some("npx tsc --noEmit"),
            ProjectType::Python => Some("mypy ."),
            ProjectType::Go => Some("go vet ./..."),
            ProjectType::Java => None,
            ProjectType::Unknown => None,
        }
    }

    /// Returns the lint command for this project type
    pub fn lint_command(&self) -> Option<&'static str> {
        match self {
            ProjectType::Rust => Some("cargo clippy"),
            ProjectType::NodeJs => Some("npm run lint"),
            ProjectType::Python => Some("ruff check ."),
            ProjectType::Go => Some("golint ./..."),
            ProjectType::Java => Some("mvn checkstyle:check"),
            ProjectType::Unknown => None,
        }
    }
}

/// Verification suggestion generated after file modification
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifySuggestion {
    /// The modified file that triggered this suggestion
    pub modified_file: String,
    /// Detected project type
    pub project_type: ProjectType,
    /// Related test files that might be affected
    pub related_tests: Vec<String>,
    /// Suggested verification commands
    pub commands: Vec<VerifyCommand>,
}

/// A single verification command with its type
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyCommand {
    /// Type of verification
    pub kind: VerifyKind,
    /// The command to execute
    pub command: String,
    /// Optional description for the command
    pub description: Option<String>,
}

/// Types of verification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyKind {
    /// Run tests
    Test,
    /// Build the project
    Build,
    /// Type checking
    TypeCheck,
    /// Linting
    Lint,
}

/// Main verification tool for project detection and test inference
pub struct VerifyTool {
    /// Root directory of the project
    project_root: PathBuf,
    /// Detected project type
    project_type: ProjectType,
}

impl VerifyTool {
    /// Create a new VerifyTool with the given project root
    pub fn new(project_root: PathBuf) -> Self {
        let project_type = Self::detect_project_type(&project_root);
        Self {
            project_root,
            project_type,
        }
    }

    /// Detect project type by checking for config files
    pub fn detect_project_type(root: &Path) -> ProjectType {
        // Check in order of specificity
        if root.join("Cargo.toml").exists() {
            return ProjectType::Rust;
        }
        if root.join("package.json").exists() {
            return ProjectType::NodeJs;
        }
        if root.join("pyproject.toml").exists() || root.join("requirements.txt").exists() {
            return ProjectType::Python;
        }
        if root.join("go.mod").exists() {
            return ProjectType::Go;
        }
        if root.join("pom.xml").exists() || root.join("build.gradle").exists() {
            return ProjectType::Java;
        }
        ProjectType::Unknown
    }

    /// Get the detected project type
    pub fn project_type(&self) -> ProjectType {
        self.project_type
    }

    /// Infer related test files for a given modified file
    pub fn infer_related_tests(&self, modified_file: &str) -> Vec<String> {
        let path = PathBuf::from(modified_file);
        let mut related_tests = Vec::new();

        match self.project_type {
            ProjectType::Rust => {
                // Rust: src/xxx.rs -> tests/xxx_test.rs or src/xxx/test.rs
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    // Check for integration tests
                    let integration_test = format!("tests/{}_test.rs", stem);
                    let module_test = path.parent()
                        .map(|p| p.join(format!("{}_test.rs", stem)))
                        .map(|p| p.to_string_lossy().to_string());

                    if self.project_root.join(&integration_test).exists() {
                        related_tests.push(integration_test);
                    }
                    if let Some(test) = module_test
                        && self.project_root.join(&test).exists() {
                            related_tests.push(test);
                        }

                    // Also check for module tests directory
                    let module_test_dir = format!("src/{}/tests.rs", stem);
                    if self.project_root.join(&module_test_dir).exists() {
                        related_tests.push(module_test_dir);
                    }
                }
            }
            ProjectType::NodeJs => {
                // Node.js: lib/xxx.ts -> test/xxx.spec.ts or __tests__/xxx.test.ts
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    // Common test patterns
                    let test_patterns = vec![
                        format!("test/{}.spec.ts", stem),
                        format!("test/{}.test.ts", stem),
                        format!("tests/{}.spec.ts", stem),
                        format!("tests/{}.test.ts", stem),
                        format!("__tests__/{}.test.ts", stem),
                        format!("__tests__/{}.test.js", stem),
                        format!("{}.spec.ts", stem),
                        format!("{}.test.ts", stem),
                    ];

                    for test_path in test_patterns {
                        // Also check with .js extension
                        let test_path_js = test_path.replace(".ts", ".js");
                        if self.project_root.join(&test_path).exists() {
                            related_tests.push(test_path);
                        } else if self.project_root.join(&test_path_js).exists() {
                            related_tests.push(test_path_js);
                        }
                    }
                }
            }
            ProjectType::Python => {
                // Python: xxx.py -> test_xxx.py
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let test_file = format!("test_{}.py", stem);
                    let tests_dir_file = format!("tests/test_{}.py", stem);

                    if self.project_root.join(&test_file).exists() {
                        related_tests.push(test_file);
                    }
                    if self.project_root.join(&tests_dir_file).exists() {
                        related_tests.push(tests_dir_file);
                    }
                }
            }
            ProjectType::Go => {
                // Go: xxx.go -> xxx_test.go
                if let Some(ext) = path.extension().and_then(|e| e.to_str())
                    && ext == "go" {
                        let test_file = format!("{}_test.go",
                            path.with_extension("").to_string_lossy());
                        if self.project_root.join(&test_file).exists() {
                            related_tests.push(test_file);
                        }
                    }
            }
            ProjectType::Java => {
                // Java: Xxx.java -> src/test/java/XxxTest.java
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let test_file = format!("src/test/java/{}Test.java", stem);
                    if self.project_root.join(&test_file).exists() {
                        related_tests.push(test_file);
                    }
                }
            }
            ProjectType::Unknown => {}
        }

        related_tests
    }

    /// Generate verification suggestion for a modified file
    pub fn generate_suggestion(&self, modified_file: &str) -> VerifySuggestion {
        let related_tests = self.infer_related_tests(modified_file);
        let mut commands = Vec::new();

        // Add type check command (fastest, run first)
        if let Some(cmd) = self.project_type.typecheck_command() {
            commands.push(VerifyCommand {
                kind: VerifyKind::TypeCheck,
                command: cmd.to_string(),
                description: Some("Type check the project".to_string()),
            });
        }

        // Add lint command
        if let Some(cmd) = self.project_type.lint_command() {
            commands.push(VerifyCommand {
                kind: VerifyKind::Lint,
                command: cmd.to_string(),
                description: Some("Run linter".to_string()),
            });
        }

        // Add test command
        if !related_tests.is_empty() {
            // If we found specific tests, suggest running those
            if let Some(test_cmd) = self.project_type.test_command() {
                let specific_cmd = match self.project_type {
                    ProjectType::Rust => {
                        // For Rust, we can run specific test file
                        format!("cargo test --test {}",
                            related_tests[0].trim_end_matches(".rs"))
                    }
                    _ => test_cmd.to_string(),
                };
                commands.push(VerifyCommand {
                    kind: VerifyKind::Test,
                    command: specific_cmd,
                    description: Some(format!("Run related tests: {}",
                        related_tests.join(", "))),
                });
            }
        } else if let Some(cmd) = self.project_type.test_command() {
            // No specific tests found, suggest running all tests
            commands.push(VerifyCommand {
                kind: VerifyKind::Test,
                command: cmd.to_string(),
                description: Some("Run all tests".to_string()),
            });
        }

        // Add build command
        if let Some(cmd) = self.project_type.build_command() {
            commands.push(VerifyCommand {
                kind: VerifyKind::Build,
                command: cmd.to_string(),
                description: Some("Build the project".to_string()),
            });
        }

        VerifySuggestion {
            modified_file: modified_file.to_string(),
            project_type: self.project_type,
            related_tests,
            commands,
        }
    }

    /// Get all available verification commands for the project
    pub fn get_all_commands(&self) -> Vec<VerifyCommand> {
        let mut commands = Vec::new();

        if let Some(cmd) = self.project_type.typecheck_command() {
            commands.push(VerifyCommand {
                kind: VerifyKind::TypeCheck,
                command: cmd.to_string(),
                description: Some("Type check the project".to_string()),
            });
        }

        if let Some(cmd) = self.project_type.lint_command() {
            commands.push(VerifyCommand {
                kind: VerifyKind::Lint,
                command: cmd.to_string(),
                description: Some("Run linter".to_string()),
            });
        }

        if let Some(cmd) = self.project_type.test_command() {
            commands.push(VerifyCommand {
                kind: VerifyKind::Test,
                command: cmd.to_string(),
                description: Some("Run all tests".to_string()),
            });
        }

        if let Some(cmd) = self.project_type.build_command() {
            commands.push(VerifyCommand {
                kind: VerifyKind::Build,
                command: cmd.to_string(),
                description: Some("Build the project".to_string()),
            });
        }

        commands
    }
}

/// Quick detection function for external use
pub fn detect_project_type(root: &Path) -> ProjectType {
    VerifyTool::detect_project_type(root)
}

/// Quick test inference function for external use
pub fn infer_related_tests(root: &Path, modified_file: &str) -> Vec<String> {
    let tool = VerifyTool::new(root.to_path_buf());
    tool.infer_related_tests(modified_file)
}

/// Generate a verification suggestion for a file modification
pub fn generate_verify_suggestion(root: &Path, modified_file: &str) -> VerifySuggestion {
    let tool = VerifyTool::new(root.to_path_buf());
    tool.generate_suggestion(modified_file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_rust_project() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        assert_eq!(VerifyTool::detect_project_type(temp_dir.path()), ProjectType::Rust);
    }

    #[test]
    fn test_detect_nodejs_project() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("package.json"), "{}").unwrap();
        assert_eq!(VerifyTool::detect_project_type(temp_dir.path()), ProjectType::NodeJs);
    }

    #[test]
    fn test_detect_python_project() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("pyproject.toml"), "[project]\nname = \"test\"").unwrap();
        assert_eq!(VerifyTool::detect_project_type(temp_dir.path()), ProjectType::Python);
    }

    #[test]
    fn test_detect_unknown_project() {
        let temp_dir = TempDir::new().unwrap();
        assert_eq!(VerifyTool::detect_project_type(temp_dir.path()), ProjectType::Unknown);
    }

    #[test]
    fn test_rust_test_inference() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        fs::create_dir(temp_dir.path().join("tests")).unwrap();
        fs::write(temp_dir.path().join("tests/utils_test.rs"), "").unwrap();

        let tool = VerifyTool::new(temp_dir.path().to_path_buf());
        let tests = tool.infer_related_tests("src/utils.rs");
        assert!(tests.contains(&"tests/utils_test.rs".to_string()));
    }

    #[test]
    fn test_project_type_commands() {
        assert_eq!(ProjectType::Rust.test_command(), Some("cargo test"));
        assert_eq!(ProjectType::Rust.build_command(), Some("cargo build"));
        assert_eq!(ProjectType::Rust.typecheck_command(), Some("cargo check"));

        assert_eq!(ProjectType::NodeJs.test_command(), Some("npm test"));
        assert_eq!(ProjectType::NodeJs.build_command(), Some("npm run build"));

        assert_eq!(ProjectType::Python.test_command(), Some("pytest"));
        assert_eq!(ProjectType::Python.build_command(), None);
    }

    #[test]
    fn test_generate_suggestion() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        fs::create_dir(temp_dir.path().join("tests")).unwrap();

        let tool = VerifyTool::new(temp_dir.path().to_path_buf());
        let suggestion = tool.generate_suggestion("src/main.rs");

        assert_eq!(suggestion.project_type, ProjectType::Rust);
        assert_eq!(suggestion.modified_file, "src/main.rs");
        assert!(!suggestion.commands.is_empty());

        // Check that typecheck is first (fastest)
        assert_eq!(suggestion.commands[0].kind, VerifyKind::TypeCheck);
    }
}