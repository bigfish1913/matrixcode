//! Section-based prompt composition
//!
//! Each section can be:
//! - Static: constant string, cacheable
//! - Dynamic: computed at runtime, not cacheable

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
}
