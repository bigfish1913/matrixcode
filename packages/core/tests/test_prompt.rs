use matrixcode_core::prompt::{
    PromptProfile, PromptSection, SectionBuilder, PromptBuilder, PromptOrchestrator,
    CACHE_BOUNDARY, build_system_prompt,
};
use matrixcode_core::skills::Skill;

#[test]
fn prompt_profile_from_str_works() {
    assert_eq!(PromptProfile::from_str("default"), PromptProfile::Default);
    assert_eq!(PromptProfile::from_str("safe"), PromptProfile::Safe);
    assert_eq!(PromptProfile::from_str("fast"), PromptProfile::Fast);
    assert_eq!(PromptProfile::from_str("review"), PromptProfile::Review);
    // Unknown values default to Default
    assert_eq!(PromptProfile::from_str("unknown"), PromptProfile::Default);
}

#[test]
fn prompt_profile_default_name_is_stable() {
    assert_eq!(PromptProfile::default().as_str(), "default");
}

#[test]
fn prompt_profile_as_str_works() {
    assert_eq!(PromptProfile::Default.as_str(), "default");
    assert_eq!(PromptProfile::Safe.as_str(), "safe");
    assert_eq!(PromptProfile::Fast.as_str(), "fast");
    assert_eq!(PromptProfile::Review.as_str(), "review");
}

#[test]
fn prompt_section_static_renders_with_named_header() {
    let section = PromptSection::static_section("TASK CONTEXT", "- current task: review");
    assert_eq!(section.render(), "[TASK CONTEXT]\n- current task: review");
}

#[test]
fn prompt_section_handles_empty_content() {
    let section = PromptSection::static_section("EMPTY", "");
    assert!(section.render().is_empty());
}

#[test]
fn prompt_section_dynamic_computes_content() {
    let section = PromptSection::dynamic_section("TIME", || {
        "current time".to_string()
    });
    assert_eq!(section.compute_content(), "current time");
}

#[test]
fn section_builder_orders_sections() {
    let sections = SectionBuilder::new()
        .add_section(PromptSection::static_section("last", "c").with_order(10))
        .add_section(PromptSection::static_section("first", "a").with_order(1))
        .add_section(PromptSection::static_section("middle", "b").with_order(5))
        .build();

    assert_eq!(sections[0].name, "first");
    assert_eq!(sections[1].name, "middle");
    assert_eq!(sections[2].name, "last");
}

#[test]
fn prompt_orchestrator_assembles_sections() {
    let mut orchestrator = PromptOrchestrator::new(std::env::current_dir().unwrap())
        .with_context_injection(false);
    orchestrator.add_section(PromptSection::static_section("identity", "You are an AI assistant."));

    let assembled = orchestrator.assemble();
    assert!(!assembled.prompt.is_empty());
    assert!(assembled.prompt.contains("identity"));
    assert!(assembled.cached_sections >= 1);
}

#[test]
fn prompt_builder_creates_orchestrator() {
    let mut orchestrator = PromptBuilder::new(std::env::current_dir().unwrap())
        .profile(PromptProfile::Default)
        .add_static("identity", "You are an AI assistant.")
        .no_context()
        .build();

    let assembled = orchestrator.assemble();
    assert!(assembled.prompt.contains("identity"));
}

#[test]
fn cache_boundary_separates_cached_and_dynamic() {
    let mut orchestrator = PromptOrchestrator::new(std::env::current_dir().unwrap())
        .with_boundary(true)
        .with_context_injection(false);

    orchestrator.add_section(PromptSection::static_section("cached", "cached content"));
    orchestrator.add_section(PromptSection::dynamic_section("dynamic", || {
        "dynamic content".to_string()
    }));

    let assembled = orchestrator.assemble();
    assert!(assembled.prompt.contains(CACHE_BOUNDARY));

    let (cached, dynamic) = assembled.split_at_boundary();
    assert!(cached.is_some());
    assert!(dynamic.is_some());
}

#[test]
fn build_system_prompt_creates_valid_prompt() {
    let skills: Vec<Skill> = vec![];
    let prompt = build_system_prompt(&PromptProfile::Default, &skills, None, None);

    // Should contain some expected sections
    assert!(!prompt.is_empty());
}

#[test]
fn prompt_section_estimates_tokens() {
    let section = PromptSection::static_section("test", "Hello world this is a test");
    let tokens = section.estimated_tokens();
    assert!(tokens > 0);
}