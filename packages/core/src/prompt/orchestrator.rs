//! Prompt Orchestrator
//!
//! Core prompt assembly system that:
//! - Composes sections in order
//! - Inserts cache boundary markers
//! - Manages caching for static sections
//! - Injects runtime context

use crate::prompt::{CacheKey, PromptSection, SectionCache};
use crate::prompt::{ContextInjector, SystemContext, UserContext};
use std::sync::Arc;

/// Cache boundary marker for API caching
///
/// This marker indicates where cached content ends and dynamic content begins.
/// APIs like Claude can use this for prompt prefix caching.
pub const CACHE_BOUNDARY: &str = "\n<!-- CACHE_BOUNDARY -->\n";

/// Prompt profile type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromptProfile {
    /// Default profile (full capabilities)
    Default,
    /// Safe profile (restricted operations)
    Safe,
    /// Fast profile (minimal prompt)
    Fast,
    /// Review profile (code review focus)
    Review,
}

impl PromptProfile {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Safe => "safe",
            Self::Fast => "fast",
            Self::Review => "review",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "safe" => Self::Safe,
            "fast" => Self::Fast,
            "review" => Self::Review,
            _ => Self::Default,
        }
    }
}

impl std::fmt::Display for PromptProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Default for PromptProfile {
    fn default() -> Self {
        Self::Default
    }
}

/// Prompt orchestrator - manages prompt assembly
pub struct PromptOrchestrator {
    /// Cache for static sections
    cache: Arc<SectionCache>,
    /// Context injector
    context_injector: ContextInjector,
    /// Current profile
    profile: PromptProfile,
    /// Sections to include
    sections: Vec<PromptSection>,
    /// Whether to include cache boundary
    include_boundary: bool,
    /// Whether to inject context
    inject_context: bool,
}

impl PromptOrchestrator {
    /// Create a new orchestrator
    pub fn new<P: Into<std::path::PathBuf>>(working_dir: P) -> Self {
        Self {
            cache: Arc::new(SectionCache::new()),
            context_injector: ContextInjector::new(working_dir.into()),
            profile: PromptProfile::Default,
            sections: Vec::new(),
            include_boundary: true,
            inject_context: true,
        }
    }

    /// Create with shared cache
    pub fn with_cache(mut self, cache: Arc<SectionCache>) -> Self {
        self.cache = cache;
        self
    }

    /// Set profile
    pub fn with_profile(mut self, profile: PromptProfile) -> Self {
        self.profile = profile;
        self
    }

    /// Set whether to include cache boundary
    pub fn with_boundary(mut self, include: bool) -> Self {
        self.include_boundary = include;
        self
    }

    /// Set whether to inject context
    pub fn with_context_injection(mut self, inject: bool) -> Self {
        self.inject_context = inject;
        self
    }

    /// Add a section
    pub fn add_section(&mut self, section: PromptSection) -> &mut Self {
        self.sections.push(section);
        self
    }

    /// Add multiple sections
    pub fn add_sections(&mut self, sections: Vec<PromptSection>) -> &mut Self {
        self.sections.extend(sections);
        self
    }

    /// Clear all sections
    pub fn clear_sections(&mut self) -> &mut Self {
        self.sections.clear();
        self
    }

    /// Invalidate cache (e.g., when context changes)
    pub fn invalidate_cache(&mut self) {
        self.cache.clear();
        self.context_injector.invalidate();
    }

    /// Render a section with caching
    fn render_section(&self, section: &PromptSection) -> String {
        if section.cacheable {
            let key = CacheKey::new(&section.name, self.profile.as_str());
            self.cache
                .get_or_compute(&key, || section.compute_content())
        } else {
            section.compute_content()
        }
    }

    /// Assemble the full prompt
    pub fn assemble(&mut self) -> AssembledPrompt {
        let mut cached_parts = Vec::new();
        let mut dynamic_parts = Vec::new();
        let mut cached_tokens = 0;
        let mut dynamic_tokens = 0;

        // Sort sections by order
        let mut sections = self.sections.clone();
        sections.sort_by_key(|s| s.order);

        // Process each section
        for section in &sections {
            let content = self.render_section(section);

            if section.cacheable {
                cached_parts.push((section.name.clone(), content.clone()));
                cached_tokens += self.estimate_tokens(&content);
            } else {
                dynamic_parts.push((section.name.clone(), content.clone()));
                dynamic_tokens += self.estimate_tokens(&content);
            }
        }

        // Inject runtime context if enabled
        let context_section = if self.inject_context {
            let ctx = self.context_injector.render_full_context();
            dynamic_tokens += self.estimate_tokens(&ctx);
            Some(ctx)
        } else {
            None
        };

        // Assemble final prompt
        let mut final_parts = Vec::new();

        // Add cached parts first
        for (name, content) in &cached_parts {
            if !content.is_empty() {
                final_parts.push(format!("[{}]\n{}", name, content));
            }
        }

        // Add cache boundary if there are both cached and dynamic parts
        if self.include_boundary
            && !cached_parts.is_empty()
            && (!dynamic_parts.is_empty() || context_section.is_some())
        {
            final_parts.push(CACHE_BOUNDARY.to_string());
        }

        // Add dynamic parts
        for (name, content) in &dynamic_parts {
            if !content.is_empty() {
                final_parts.push(format!("[{}]\n{}", name, content));
            }
        }

        // Add context section
        if let Some(ctx) = context_section {
            final_parts.push(ctx);
        }

        let full_prompt = final_parts.join("\n\n");

        // Get cache stats
        let stats = self.cache.stats();

        AssembledPrompt {
            prompt: full_prompt,
            cached_sections: cached_parts.len(),
            dynamic_sections: dynamic_parts.len(),
            cached_tokens,
            dynamic_tokens,
            total_tokens: cached_tokens + dynamic_tokens,
            cache_hit_rate: stats.hit_rate(),
            profile: self.profile,
        }
    }

    /// Assemble prompt for specific profile
    pub fn assemble_for_profile(&mut self, profile: PromptProfile) -> AssembledPrompt {
        self.profile = profile;
        self.assemble()
    }

    /// Estimate token count
    fn estimate_tokens(&self, content: &str) -> usize {
        crate::prompt::cache::estimate_tokens(content)
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> crate::prompt::cache::CacheStats {
        self.cache.stats()
    }

    /// Get user context
    pub fn get_user_context(&mut self) -> &UserContext {
        self.context_injector.get_user_context()
    }

    /// Get system context
    pub fn get_system_context(&mut self) -> &SystemContext {
        self.context_injector.get_system_context()
    }
}

/// Assembled prompt with metadata
#[derive(Debug, Clone)]
pub struct AssembledPrompt {
    /// The full prompt text
    pub prompt: String,
    /// Number of cached sections
    pub cached_sections: usize,
    /// Number of dynamic sections
    pub dynamic_sections: usize,
    /// Estimated cached tokens
    pub cached_tokens: usize,
    /// Estimated dynamic tokens
    pub dynamic_tokens: usize,
    /// Total estimated tokens
    pub total_tokens: usize,
    /// Cache hit rate
    pub cache_hit_rate: f64,
    /// Profile used
    pub profile: PromptProfile,
}

impl AssembledPrompt {
    /// Check if prompt is empty
    pub fn is_empty(&self) -> bool {
        self.prompt.is_empty()
    }

    /// Get prompt length in characters
    pub fn len(&self) -> usize {
        self.prompt.len()
    }

    /// Get cache efficiency percentage
    pub fn cache_efficiency(&self) -> f64 {
        if self.total_tokens == 0 {
            0.0
        } else {
            (self.cached_tokens as f64 / self.total_tokens as f64) * 100.0
        }
    }

    /// Split prompt at cache boundary
    pub fn split_at_boundary(&self) -> (Option<&str>, Option<&str>) {
        if let Some(idx) = self.prompt.find(CACHE_BOUNDARY) {
            let cached = &self.prompt[..idx];
            let dynamic = &self.prompt[idx + CACHE_BOUNDARY.len()..];
            (
                if cached.is_empty() {
                    None
                } else {
                    Some(cached)
                },
                if dynamic.is_empty() {
                    None
                } else {
                    Some(dynamic)
                },
            )
        } else {
            if self.prompt.is_empty() {
                (None, None)
            } else {
                (Some(&self.prompt), None)
            }
        }
    }
}

/// Builder for creating prompt orchestrators
pub struct PromptBuilder {
    working_dir: std::path::PathBuf,
    profile: PromptProfile,
    sections: Vec<PromptSection>,
    include_boundary: bool,
    inject_context: bool,
}

impl PromptBuilder {
    pub fn new<P: Into<std::path::PathBuf>>(working_dir: P) -> Self {
        Self {
            working_dir: working_dir.into(),
            profile: PromptProfile::Default,
            sections: Vec::new(),
            include_boundary: true,
            inject_context: true,
        }
    }

    pub fn profile(mut self, profile: PromptProfile) -> Self {
        self.profile = profile;
        self
    }

    pub fn add_section(mut self, section: PromptSection) -> Self {
        self.sections.push(section);
        self
    }

    pub fn add_static(self, name: impl Into<String>, content: &'static str) -> Self {
        self.add_section(PromptSection::static_section(name, content))
    }

    pub fn add_dynamic<F>(self, name: impl Into<String>, compute: F) -> Self
    where
        F: Fn() -> String + Send + Sync + 'static,
    {
        self.add_section(PromptSection::dynamic_section(name, compute))
    }

    pub fn no_boundary(mut self) -> Self {
        self.include_boundary = false;
        self
    }

    pub fn no_context(mut self) -> Self {
        self.inject_context = false;
        self
    }

    pub fn build(self) -> PromptOrchestrator {
        let mut orchestrator = PromptOrchestrator::new(self.working_dir)
            .with_profile(self.profile)
            .with_boundary(self.include_boundary)
            .with_context_injection(self.inject_context);
        orchestrator.add_sections(self.sections);
        orchestrator
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assemble_simple() {
        let mut orchestrator = PromptOrchestrator::new(std::env::current_dir().unwrap());
        orchestrator.add_section(PromptSection::static_section(
            "identity",
            "You are an AI assistant.",
        ));

        let assembled = orchestrator.assemble();
        assert!(!assembled.prompt.is_empty());
        assert!(assembled.prompt.contains("identity"));
        assert!(assembled.cached_sections >= 1);
    }

    #[test]
    fn test_assemble_with_dynamic() {
        let mut orchestrator = PromptOrchestrator::new(std::env::current_dir().unwrap());
        orchestrator.add_section(PromptSection::static_section("identity", "You are an AI."));
        orchestrator.add_section(PromptSection::dynamic_section("date", || {
            format!("Current date: {}", chrono::Local::now().format("%Y-%m-%d"))
        }));

        let assembled = orchestrator.assemble();
        assert!(assembled.dynamic_sections >= 1);
        assert!(assembled.cached_sections >= 1);
    }

    #[test]
    fn test_cache_boundary() {
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
    fn test_profile() {
        let orchestrator = PromptOrchestrator::new(std::env::current_dir().unwrap())
            .with_profile(PromptProfile::Fast);

        assert_eq!(orchestrator.profile, PromptProfile::Fast);
    }

    #[test]
    fn test_builder() {
        let mut orchestrator = PromptBuilder::new(std::env::current_dir().unwrap())
            .profile(PromptProfile::Review)
            .add_static("identity", "You are a code reviewer.")
            .add_dynamic("date", || "Today".to_string())
            .build();

        let assembled = orchestrator.assemble();
        assert!(assembled.prompt.contains("identity"));
    }

    #[test]
    fn test_cache_efficiency() {
        let mut orchestrator =
            PromptOrchestrator::new(std::env::current_dir().unwrap()).with_context_injection(false);

        // Add static content with words (to have proper token estimate)
        let static_content = "static content that should be cached properly test test test";
        orchestrator.add_section(PromptSection::static_section("big", static_content));
        orchestrator.add_section(PromptSection::dynamic_section("small", || {
            "dynamic".to_string()
        }));

        let assembled = orchestrator.assemble();
        let efficiency = assembled.cache_efficiency();

        // Static content should be cached
        assert!(efficiency >= 50.0, "Cache efficiency: {}", efficiency);
    }

    #[test]
    fn test_invalidate_cache() {
        let mut orchestrator = PromptOrchestrator::new(std::env::current_dir().unwrap());
        orchestrator.add_section(PromptSection::static_section("test", "test content"));

        // First assembly
        let _ = orchestrator.assemble();

        // Invalidate
        orchestrator.invalidate_cache();

        // Should recalculate
        let assembled = orchestrator.assemble();
        assert!(assembled.prompt.contains("test"));
    }
}
