use matrixcode_core::tools;

#[test]
fn test_all_tools_count() {
    // The tool count has grown as new tools were added.
    // This test just ensures the count is stable and >= expected minimum.
    let all = tools::all_tools();
    // Current count: 22 tools (basic tools + task tools + plan_mode tools + monitor)
    assert!(
        all.len() >= 22,
        "Expected at least 22 tools, got {}",
        all.len()
    );
}

#[test]
fn test_all_tools_have_unique_names() {
    let all = tools::all_tools();
    let names: Vec<String> = all.iter().map(|t| t.definition().name).collect();
    let mut deduped = names.clone();
    deduped.sort();
    deduped.dedup();
    assert_eq!(names.len(), deduped.len());
}

#[test]
fn test_all_tools_have_descriptions() {
    let all = tools::all_tools();
    for tool in &all {
        let def = tool.definition();
        assert!(
            !def.description.is_empty(),
            "tool {} has empty description",
            def.name
        );
    }
}

#[test]
fn test_all_tools_have_valid_parameters() {
    let all = tools::all_tools();
    for tool in &all {
        let def = tool.definition();
        assert_eq!(
            def.parameters["type"], "object",
            "tool {} parameters should be object",
            def.name
        );
        assert!(
            def.parameters["properties"].is_object(),
            "tool {} should have properties",
            def.name
        );
    }
}

#[test]
fn test_expected_tool_names() {
    let all = tools::all_tools();
    let names: Vec<String> = all.iter().map(|t| t.definition().name).collect();
    assert!(names.contains(&"ask".to_string()));
    assert!(names.contains(&"read".to_string()));
    assert!(names.contains(&"write".to_string()));
    assert!(names.contains(&"edit".to_string()));
    assert!(names.contains(&"search".to_string()));
    assert!(names.contains(&"glob".to_string()));
    assert!(names.contains(&"ls".to_string()));
    assert!(names.contains(&"bash".to_string()));
    assert!(names.contains(&"multi_edit".to_string()));
    assert!(names.contains(&"todo_write".to_string()));
    assert!(names.contains(&"websearch".to_string()));
    assert!(names.contains(&"webfetch".to_string()));
    assert!(names.contains(&"skill".to_string()));
}
