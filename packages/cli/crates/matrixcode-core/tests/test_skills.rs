use std::path::PathBuf;

use matrixcode_core::skills;
use matrixcode_core::tools;
use serde_json::json;
use std::sync::Arc;
use tempfile::tempdir;

fn write_file(path: &std::path::Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

#[test]
fn discover_finds_nested_skills() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().join("skills");
    write_file(
        &root.join("alpha/SKILL.md"),
        "---\nname: alpha\ndescription: first\n---\nAlpha body\n",
    );
    write_file(
        &root.join("beta/SKILL.md"),
        "---\nname: beta\ndescription: second\n---\nBeta body\n",
    );
    // A directory without SKILL.md must be ignored.
    write_file(&root.join("ignored/notes.txt"), "nope");

    let found = skills::discover_skills(&[root]);
    let names: Vec<&str> = found.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["alpha", "beta"]);
}

#[test]
fn discover_returns_empty_when_no_roots_exist() {
    let found = skills::discover_skills(&[PathBuf::from("/no/such/path/here")]);
    assert!(found.is_empty());
}

#[test]
fn format_catalogue_includes_each_skill() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().join("skills");
    write_file(
        &root.join("one/SKILL.md"),
        "---\nname: one\ndescription: does one thing\n---\nbody\n",
    );
    let found = skills::discover_skills(&[root]);
    let cat = skills::format_catalogue(&found).unwrap();
    assert!(cat.contains("one: does one thing"));
    assert!(cat.contains("skill"));
}

#[tokio::test]
async fn skill_tool_loads_body_and_lists_files() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().join("skills");
    write_file(
        &root.join("demo/SKILL.md"),
        "---\nname: demo\ndescription: example\n---\nDo the thing.\n",
    );
    write_file(&root.join("demo/helper.sh"), "#!/bin/sh\necho hi\n");
    write_file(&root.join("demo/templates/note.md"), "note template");

    let found = skills::discover_skills(&[root]);
    let all = tools::all_tools_with_skills(Arc::new(found));
    let skill_tool = all
        .iter()
        .find(|t| t.definition().name == "skill")
        .expect("skill tool missing");

    let out = skill_tool
        .execute(json!({ "name": "demo" }))
        .await
        .unwrap();

    assert!(out.contains("# Skill: demo"));
    assert!(out.contains("Do the thing."));
    assert!(out.contains("SKILL.md"));
    assert!(out.contains("helper.sh"));
    // Accept both Unix and Windows path separators
    assert!(out.contains("templates/note.md") || out.contains("templates\\note.md"));
}

#[tokio::test]
async fn skill_tool_rejects_unknown_name() {
    let all = tools::all_tools_with_skills(Arc::new(Vec::new()));
    let skill_tool = all
        .iter()
        .find(|t| t.definition().name == "skill")
        .unwrap();

    let err = skill_tool
        .execute(json!({ "name": "ghost" }))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("unknown skill"));
}

#[test]
fn tools_include_skill_tool() {
    let all = tools::all_tools();
    let names: Vec<String> = all.iter().map(|t| t.definition().name).collect();
    assert!(names.contains(&"skill".to_string()));
}
