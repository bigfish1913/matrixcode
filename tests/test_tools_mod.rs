use code_agent::tools;

#[test]
fn test_all_tools_returns_five() {
    let all = tools::all_tools();
    assert_eq!(all.len(), 5);
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
        assert!(!def.description.is_empty(), "tool {} has empty description", def.name);
    }
}

#[test]
fn test_all_tools_have_valid_parameters() {
    let all = tools::all_tools();
    for tool in &all {
        let def = tool.definition();
        assert_eq!(def.parameters["type"], "object", "tool {} parameters should be object", def.name);
        assert!(def.parameters["properties"].is_object(), "tool {} should have properties", def.name);
    }
}

#[test]
fn test_expected_tool_names() {
    let all = tools::all_tools();
    let names: Vec<String> = all.iter().map(|t| t.definition().name).collect();
    assert!(names.contains(&"read".to_string()));
    assert!(names.contains(&"write".to_string()));
    assert!(names.contains(&"edit".to_string()));
    assert!(names.contains(&"search".to_string()));
    assert!(names.contains(&"webfetch".to_string()));
}
