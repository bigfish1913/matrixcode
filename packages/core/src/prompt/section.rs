//! Section-based prompt composition
//!
//! Each section can be:
//! - Static: constant string, cacheable
//! - Dynamic: computed at runtime, not cacheable
//!
//! Predefined sections are available for common prompt components:
//! - Red flags warning table
//! - Skill priority rules
//! - Tool usage guidelines

use std::sync::Arc;

/// Section content type
#[derive(Clone)]
pub enum SectionContent {
    /// Static content that can be cached
    Static(&'static str),
    /// Dynamic content computed at runtime
    Dynamic(Arc<dyn Fn() -> String + Send + Sync>),
    /// Cached result (computed once and stored)
    Cached(String),
}

impl SectionContent {
    /// Create static content
    pub fn static_content(s: &'static str) -> Self {
        Self::Static(s)
    }

    /// Create dynamic content with a computation function
    pub fn dynamic<F>(f: F) -> Self
    where
        F: Fn() -> String + Send + Sync + 'static,
    {
        Self::Dynamic(Arc::new(f))
    }

    /// Compute the content (for dynamic sections)
    pub fn compute(&self) -> String {
        match self {
            Self::Static(s) => s.to_string(),
            Self::Dynamic(f) => f(),
            Self::Cached(s) => s.clone(),
        }
    }

    /// Check if this content can be cached
    pub fn is_cacheable(&self) -> bool {
        matches!(self, Self::Static(_) | Self::Cached(_))
    }

    /// Mark content as cached (after computing)
    pub fn cache(self, content: String) -> Self {
        Self::Cached(content)
    }
}

impl std::fmt::Debug for SectionContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Static(s) => f.debug_tuple("Static").field(&s.len()).finish(),
            Self::Dynamic(_) => f.write_str("Dynamic(<function>)"),
            Self::Cached(s) => f.debug_tuple("Cached").field(&s.len()).finish(),
        }
    }
}

/// A prompt section with name, content, and cacheability
#[derive(Clone, Debug)]
pub struct PromptSection {
    /// Unique section identifier
    pub name: String,
    /// Section content
    pub content: SectionContent,
    /// Whether this section should be cached (default: true for static)
    pub cacheable: bool,
    /// Section priority/order (lower = earlier in prompt)
    pub order: usize,
}

// ============================================================================
// Predefined Sections - Common prompt components
// ============================================================================

/// Red flags warning section - prevents common AI reasoning errors
///
/// This section should be included early in the prompt to help the AI
/// avoid common pitfalls like bypassing skill checks or acting before thinking.
pub fn red_flags_section() -> PromptSection {
    PromptSection::static_section("red_flags", RED_FLAGS_CONTENT).with_order(10)
}

/// Skill priority rules section - defines skill invocation order
///
/// Process skills (brainstorming, debugging) should be invoked before
/// Implementation skills (frontend, code-review).
pub fn skill_priority_section() -> PromptSection {
    PromptSection::static_section("skill_priority", SKILL_PRIORITY_CONTENT).with_order(15)
}

/// Skill invocation rules section - mandatory skill check before action
///
/// This enforces the "1% rule": if there's even a 1% chance a skill applies,
/// it MUST be invoked before any other response.
pub fn skill_rules_section() -> PromptSection {
    PromptSection::static_section("skill_rules", SKILL_RULES_CONTENT).with_order(5)
}

/// Tool usage guidelines section - how to use tools effectively
pub fn tool_guidelines_section() -> PromptSection {
    PromptSection::static_section("tool_guidelines", TOOL_GUIDELINES_CONTENT).with_order(20)
}

/// Predefined red flags content
const RED_FLAGS_CONTENT: &'static str = "
## Red Flags - STOP and reconsider

These thoughts mean STOP — you're rationalizing:

| Thought | Reality |
|---------|---------|
| \"This is just a simple question\" | Questions are tasks. Check for skills. |
| \"I need more context first\" | Skill check comes BEFORE clarifying questions. |
| \"Let me explore the codebase first\" | Skills tell you HOW to explore. Check first. |
| \"I can check git/files quickly\" | Files lack conversation context. Check for skills. |
| \"Let me gather information first\" | Skills tell you HOW to gather information. |
| \"This doesn't need a formal skill\" | If a skill exists, use it. |
| \"I remember this skill\" | Skills evolve. Read current version. |
| \"This doesn't count as a task\" | Action = task. Check for skills. |
| \"The skill is overkill\" | Simple things become complex. Use it. |
| \"I'll just do this one thing first\" | Check BEFORE doing anything. |
| \"This feels productive\" | Undisciplined action wastes time. Skills prevent this. |
| \"I know what that means\" | Knowing the concept ≠ using the skill. Invoke it. |

**When you catch yourself thinking these, STOP and invoke the relevant skill.**
";

/// Predefined skill priority content
const SKILL_PRIORITY_CONTENT: &'static str = "
## Skill Priority

When multiple skills could apply, use this order:

1. **Process skills first** (brainstorming, debugging, planning) - these determine HOW to approach the task
2. **Implementation skills second** (frontend-design, mcp-builder, code-review) - these guide execution

Examples:
- \"Let's build X\" → brainstorming first, then implementation skills
- \"Fix this bug\" → debugging first, then domain-specific skills
- \"Review this code\" → code-review skill directly (domain-specific)

## Skill Types

- **Rigid skills** (TDD, debugging): Follow exactly. Don't adapt away discipline.
- **Flexible skills** (patterns): Adapt principles to context.

The skill itself tells you which type it is.
";

/// Predefined skill rules content
const SKILL_RULES_CONTENT: &'static str = "
<EXTREMELY-IMPORTANT>
If you think there is even a **1% chance** a skill might apply to what you are doing,
you **ABSOLUTELY MUST** invoke the skill.

IF A SKILL APPLIES TO YOUR TASK, YOU DO NOT HAVE A CHOICE. YOU MUST USE IT.

This is not negotiable. This is not optional. You cannot rationalize your way out of this.
</EXTREMELY-IMPORTANT>

## The Rule

**Invoke relevant or requested skills BEFORE any response or action.**
Even a 1% chance a skill might apply means that you should invoke the skill to check.
If an invoked skill turns out to be wrong for the situation, you don't need to use it.
";

/// Predefined tool guidelines content
const TOOL_GUIDELINES_CONTENT: &'static str = "
## Tool Usage Guidelines

### When to use tools

| Task | Preferred Tool |
|------|----------------|
| Find files by pattern | `Glob` (not `find` command) |
| Search file contents | `Grep` (not `grep` command) |
| Read a specific file | `Read` (not `cat` command) |
| Search code symbols | `code_search` (not `grep`) |
| Find function callers | `code_callers` (not manual search) |
| Find function callees | `code_callees` (not manual search) |

### CodeGraph vs Native Search

Use **CodeGraph** (`code_*` tools) for structural questions:
- \"Where is X defined?\" → `code_search`
- \"What calls Y?\" → `code_callers`
- \"What does Y call?\" → `code_callees`

Use **native grep/read** for literal text:
- String contents, comments, log messages
- After you already have a specific file open

### Rules of thumb

- **Don't grep first** when looking up a symbol by name
- **Trust CodeGraph results** — they come from full AST parse
- **Don't re-verify** CodeGraph results with grep (slower, less accurate)
";

impl PromptSection {
    /// Create a new static section
    pub fn static_section(name: impl Into<String>, content: &'static str) -> Self {
        Self {
            name: name.into(),
            content: SectionContent::static_content(content),
            cacheable: true,
            order: 0,
        }
    }

    /// Create a new dynamic section
    pub fn dynamic_section<F>(name: impl Into<String>, compute: F) -> Self
    where
        F: Fn() -> String + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            content: SectionContent::dynamic(compute),
            cacheable: false,
            order: 0,
        }
    }

    /// Create a cached section (after computation)
    pub fn cached_section(name: impl Into<String>, content: String) -> Self {
        Self {
            name: name.into(),
            content: SectionContent::Cached(content),
            cacheable: true,
            order: 0,
        }
    }

    /// Set section order
    pub fn with_order(self, order: usize) -> Self {
        Self { order, ..self }
    }

    /// Set cacheability
    pub fn with_cacheable(self, cacheable: bool) -> Self {
        Self { cacheable, ..self }
    }

    /// Compute and render the section content
    pub fn render(&self) -> String {
        let content = self.content.compute();
        if content.is_empty() {
            String::new()
        } else {
            format!("[{}]\n{}", self.name, content)
        }
    }

    /// Compute raw content (without section header)
    pub fn compute_content(&self) -> String {
        self.content.compute()
    }

    /// Get estimated token count (approximate)
    pub fn estimated_tokens(&self) -> usize {
        // Rough estimate: ~4 chars per token for Chinese, ~1 token per word for English
        let content = self.compute_content();
        let chinese_chars = content
            .chars()
            .filter(|c| c.is_alphabetic() && c.len_utf8() > 1)
            .count();
        let english_words = content.split_whitespace().count();
        chinese_chars / 3 + english_words + (content.len() - chinese_chars) / 4
    }
}

/// Builder for creating prompt sections
pub struct SectionBuilder {
    sections: Vec<PromptSection>,
}

impl SectionBuilder {
    pub fn new() -> Self {
        Self {
            sections: Vec::new(),
        }
    }

    /// Add a static section
    pub fn add_static(self, name: impl Into<String>, content: &'static str) -> Self {
        self.add_section(PromptSection::static_section(name, content))
    }

    /// Add a dynamic section
    pub fn add_dynamic<F>(self, name: impl Into<String>, compute: F) -> Self
    where
        F: Fn() -> String + Send + Sync + 'static,
    {
        self.add_section(PromptSection::dynamic_section(name, compute))
    }

    /// Add a section
    pub fn add_section(mut self, section: PromptSection) -> Self {
        self.sections.push(section);
        self
    }

    /// Build sections sorted by order
    pub fn build(self) -> Vec<PromptSection> {
        let mut sections = self.sections;
        sections.sort_by_key(|s| s.order);
        sections
    }
}

impl Default for SectionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_section() {
        let section = PromptSection::static_section("identity", "You are an AI assistant.");
        assert!(section.cacheable);
        assert_eq!(section.compute_content(), "You are an AI assistant.");
    }

    #[test]
    fn test_dynamic_section() {
        let section = PromptSection::dynamic_section("date", || {
            format!("Current date: {}", chrono::Local::now().format("%Y-%m-%d"))
        });
        assert!(!section.cacheable);
        let content = section.compute_content();
        assert!(content.starts_with("Current date:"));
    }

    #[test]
    fn test_render_with_header() {
        let section = PromptSection::static_section("test", "Hello");
        let rendered = section.render();
        assert_eq!(rendered, "[test]\nHello");
    }

    #[test]
    fn test_section_builder() {
        let sections = SectionBuilder::new()
            .add_static("a", "content a")
            .add_static("b", "content b")
            .build();
        assert_eq!(sections.len(), 2);
    }

    #[test]
    fn test_order_sorting() {
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
    fn test_predefined_red_flags_section() {
        let section = red_flags_section();
        assert_eq!(section.name, "red_flags");
        assert!(section.cacheable);
        assert!(section.order == 10);
        let content = section.compute_content();
        assert!(content.contains("Red Flags"));
        assert!(content.contains("STOP"));
        assert!(content.contains("This is just a simple question"));
    }

    #[test]
    fn test_predefined_skill_priority_section() {
        let section = skill_priority_section();
        assert_eq!(section.name, "skill_priority");
        assert!(section.cacheable);
        assert!(section.order == 15);
        let content = section.compute_content();
        assert!(content.contains("Skill Priority"));
        assert!(content.contains("Process skills first"));
    }

    #[test]
    fn test_predefined_skill_rules_section() {
        let section = skill_rules_section();
        assert_eq!(section.name, "skill_rules");
        assert!(section.cacheable);
        assert!(section.order == 5); // Should be earliest
        let content = section.compute_content();
        assert!(content.contains("1%"));
        assert!(content.contains("MUST"));
    }

    #[test]
    fn test_predefined_tool_guidelines_section() {
        let section = tool_guidelines_section();
        assert_eq!(section.name, "tool_guidelines");
        assert!(section.cacheable);
        let content = section.compute_content();
        assert!(content.contains("Glob"));
        assert!(content.contains("Grep"));
        assert!(content.contains("CodeGraph"));
    }

    #[test]
    fn test_predefined_sections_order() {
        // Verify predefined sections have correct order
        let rules = skill_rules_section();
        let flags = red_flags_section();
        let priority = skill_priority_section();
        let tools = tool_guidelines_section();

        // Rules should come first (lowest order)
        assert!(rules.order < flags.order);
        assert!(flags.order < priority.order);
        assert!(priority.order < tools.order);
    }
}
