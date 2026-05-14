use code_agent::prompt::{
    build_static_system_prompt, PromptContext, PromptProfile, PromptSection,
    SystemPromptBuilder, SECTION_AVAILABLE_SKILLS,
};

#[test]
fn prompt_profile_parses_known_values() {
    assert_eq!(
        "default".parse::<PromptProfile>().unwrap(),
        PromptProfile::Default
    );
    assert_eq!("safe".parse::<PromptProfile>().unwrap(), PromptProfile::Safe);
    assert_eq!("fast".parse::<PromptProfile>().unwrap(), PromptProfile::Fast);
    assert_eq!(
        "review".parse::<PromptProfile>().unwrap(),
        PromptProfile::Review
    );
}

#[test]
fn prompt_profile_rejects_unknown_value() {
    let err = "unknown".parse::<PromptProfile>().unwrap_err();
    assert!(err.contains("unknown prompt profile"));
}

#[test]
fn prompt_profile_default_name_is_stable() {
    assert_eq!(PromptProfile::default().as_str(), "default");
}

#[test]
fn safe_profile_omits_execution_policy() {
    let prompt = build_static_system_prompt(PromptProfile::Safe);
    assert!(!prompt.contains("执行策略："));
    assert!(prompt.contains("行为约束："));
    assert!(prompt.contains("编辑规则："));
}

#[test]
fn fast_profile_omits_behavior_and_editing_rules() {
    let prompt = build_static_system_prompt(PromptProfile::Fast);
    assert!(prompt.contains("执行策略："));
    assert!(!prompt.contains("行为约束："));
    assert!(!prompt.contains("编辑规则："));
}

#[test]
fn review_profile_omits_editing_and_execution_rules() {
    let prompt = build_static_system_prompt(PromptProfile::Review);
    assert!(prompt.contains("行为约束："));
    assert!(!prompt.contains("编辑规则："));
    assert!(!prompt.contains("执行策略："));
}

#[test]
fn prompt_section_renders_with_named_header() {
    let section = PromptSection::new("TASK CONTEXT", "- current task: review").unwrap();
    assert_eq!(section.render(), "[TASK CONTEXT]\n- current task: review");
}

#[test]
fn prompt_section_skips_blank_title_or_body() {
    assert!(PromptSection::new("", "body").is_none());
    assert!(PromptSection::new("TITLE", "   ").is_none());
}

#[test]
fn prompt_context_renders_multiple_sections_in_order() {
    let context = PromptContext::new()
        .with_section("PROJECT CONTEXT", "- language: Rust")
        .with_section("TASK CONTEXT", "- mode: explain");
    assert_eq!(
        context.render_sections(),
        vec![
            "[PROJECT CONTEXT]\n- language: Rust".to_string(),
            "[TASK CONTEXT]\n- mode: explain".to_string()
        ]
    );
}

#[test]
fn builder_appends_named_sections_after_static_prompt() {
    let prompt = SystemPromptBuilder::new(PromptProfile::Default)
        .with_section("DYNAMIC", "- foo")
        .build();
    assert!(prompt.contains("完成要求："));
    assert!(prompt.ends_with("[DYNAMIC]\n- foo"));
}

#[test]
fn builder_accepts_structured_context() {
    let context = PromptContext::new().with_available_skills("- demo: does stuff");
    let prompt = SystemPromptBuilder::new(PromptProfile::Default)
        .with_context(context)
        .build();
    assert!(prompt.contains(&format!("[{}]\n- demo: does stuff", SECTION_AVAILABLE_SKILLS)));
}

#[test]
fn builder_renders_named_skills_section_after_static_prompt() {
    let prompt = SystemPromptBuilder::new(PromptProfile::Default)
        .with_available_skills(
            "Use the `skill` tool with the skill's name to load its full instructions:\n- demo: does stuff",
        )
        .build();
    assert!(prompt.contains("完成要求："));
    assert!(prompt.contains(&format!("[{}]", SECTION_AVAILABLE_SKILLS)));
    assert!(prompt.contains("- demo: does stuff"));
}