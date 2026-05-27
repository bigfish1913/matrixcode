//! Project root detection for CodeGraph.

use std::path::{Path, PathBuf};

/// Project root marker files (used to detect project root).
const PROJECT_ROOT_MARKERS: &[&str] = &[
    // Git
    ".git",
    // Rust
    "Cargo.toml",
    // Node.js
    "package.json",
    // TypeScript
    "tsconfig.json",
    // Go
    "go.mod",
    // Python
    "pyproject.toml",
    "setup.py",
    "requirements.txt",
    // Java
    "pom.xml",
    "build.gradle",
    // PHP
    "composer.json",
    // Ruby
    "Gemfile",
];

/// Create a command with hidden window on Windows.
#[cfg(windows)]
fn create_command(program: &str) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let mut cmd = std::process::Command::new(program);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[cfg(not(windows))]
fn create_command(program: &str) -> std::process::Command {
    std::process::Command::new(program)
}

/// Find project root directory from a given starting path.
pub fn find_project_root(start_path: &Path) -> PathBuf {
    // Try to find by Git first
    if let Some(git_root) = find_git_root(start_path) {
        return git_root;
    }

    // Try to find by project markers
    if let Some(marker_root) = find_by_markers(start_path) {
        return marker_root;
    }

    // Fallback to current directory
    start_path.to_path_buf()
}

/// Find Git repository root.
fn find_git_root(start_path: &Path) -> Option<PathBuf> {
    let output = create_command("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(start_path)
        .output()
        .ok()?;

    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Some(PathBuf::from(path))
    } else {
        None
    }
}

/// Find project root by looking for marker files.
fn find_by_markers(start_path: &Path) -> Option<PathBuf> {
    let mut current = start_path.to_path_buf();

    loop {
        // Check for any marker file
        for marker in PROJECT_ROOT_MARKERS {
            let marker_path = current.join(marker);
            if marker_path.exists() {
                return Some(current);
            }
        }

        // Move up one directory
        if !current.pop() {
            break;
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_project_root_with_git() {
        let start = std::env::current_dir().unwrap();
        let root = find_project_root(&start);
        assert!(root.exists());
    }
}