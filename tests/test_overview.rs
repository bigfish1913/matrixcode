use std::fs;

use matrixcode::overview::{
    detect_project_type, should_ignore, truncate_content, OVERVIEW_FILENAME,
    ProjectOverview, PROJECT_TYPE_CONFIGS, SRC_DIR,
};
use tempfile::TempDir;

#[test]
fn should_ignore_patterns() {
    assert!(should_ignore(".git"));
    assert!(should_ignore("node_modules"));
    assert!(should_ignore("target"));
    assert!(should_ignore("target-test"));
    assert!(should_ignore("__pycache__"));
    assert!(should_ignore("Cargo.lock"));
    assert!(!should_ignore(SRC_DIR));
    assert!(!should_ignore("main.rs"));
}

#[test]
fn overview_load_returns_none_when_missing() {
    let tmp = TempDir::new().unwrap();
    let result = ProjectOverview::load(tmp.path()).unwrap();
    assert!(result.is_none());
}

#[test]
fn overview_clear_removes_file() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join(OVERVIEW_FILENAME);
    fs::write(&path, "test content").unwrap();

    assert!(ProjectOverview::exists(tmp.path()));
    ProjectOverview::clear(tmp.path()).unwrap();
    assert!(!ProjectOverview::exists(tmp.path()));
}

#[test]
fn detect_project_type_rust() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
    let expected = PROJECT_TYPE_CONFIGS
        .iter()
        .find(|c| c.type_name == "Rust")
        .map(|c| c.type_name)
        .unwrap();
    assert_eq!(detect_project_type(tmp.path()), expected);
}

#[test]
fn detect_project_type_go() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("go.mod"), "").unwrap();
    let expected = PROJECT_TYPE_CONFIGS
        .iter()
        .find(|c| c.type_name == "Go")
        .map(|c| c.type_name)
        .unwrap();
    assert_eq!(detect_project_type(tmp.path()), expected);
}

#[test]
fn detect_project_type_nodejs() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("package.json"), "").unwrap();
    let expected = PROJECT_TYPE_CONFIGS
        .iter()
        .find(|c| c.type_name == "Node.js/TypeScript")
        .map(|c| c.type_name)
        .unwrap();
    assert_eq!(detect_project_type(tmp.path()), expected);
}

#[test]
fn detect_project_type_python_pyproject() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("pyproject.toml"), "").unwrap();
    let expected = PROJECT_TYPE_CONFIGS
        .iter()
        .find(|c| c.type_name == "Python")
        .map(|c| c.type_name)
        .unwrap();
    assert_eq!(detect_project_type(tmp.path()), expected);
}

#[test]
fn detect_project_type_python_requirements() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("requirements.txt"), "").unwrap();
    let expected = PROJECT_TYPE_CONFIGS
        .iter()
        .find(|c| c.type_name == "Python")
        .map(|c| c.type_name)
        .unwrap();
    assert_eq!(detect_project_type(tmp.path()), expected);
}

#[test]
fn truncate_content_works() {
    let long = "a".repeat(100);
    let truncated = truncate_content(&long, 50);
    assert!(truncated.len() <= 70); // 50 + "... (truncated)"
    assert!(truncated.contains("truncated"));

    let short = "short";
    let not_truncated = truncate_content(short, 50);
    assert_eq!(not_truncated, short);
}