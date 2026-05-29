//! Prompt Export and Observability
//!
//! Provides:
//! - JSONL dump for prompt inspection
//! - Debug output for development
//! - Prompt analysis tools

use std::path::{Path, PathBuf};
use std::io::Write;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use crate::prompt::AssembledPrompt;

/// A single dump entry for JSONL export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpEntry {
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Profile used
    pub profile: String,
    /// Full prompt content
    pub prompt: String,
    /// Cached sections count
    pub cached_sections: usize,
    /// Dynamic sections count
    pub dynamic_sections: usize,
    /// Cached tokens estimate
    pub cached_tokens: usize,
    /// Dynamic tokens estimate
    pub dynamic_tokens: usize,
    /// Total tokens estimate
    pub total_tokens: usize,
    /// Cache efficiency percentage
    pub cache_efficiency: f64,
    /// Session ID (optional)
    pub session_id: Option<String>,
    /// Conversation ID (optional)
    pub conversation_id: Option<String>,
}

impl DumpEntry {
    /// Create from assembled prompt
    pub fn from_prompt(prompt: &AssembledPrompt, session_id: Option<String>) -> Self {
        Self {
            timestamp: Utc::now(),
            profile: prompt.profile.to_string(),
            prompt: prompt.prompt.clone(),
            cached_sections: prompt.cached_sections,
            dynamic_sections: prompt.dynamic_sections,
            cached_tokens: prompt.cached_tokens,
            dynamic_tokens: prompt.dynamic_tokens,
            total_tokens: prompt.total_tokens,
            cache_efficiency: prompt.cache_efficiency(),
            session_id,
            conversation_id: None,
        }
    }

    /// Create with additional conversation context
    pub fn with_conversation(mut self, conversation_id: String) -> Self {
        self.conversation_id = Some(conversation_id);
        self
    }
}

/// Prompt dumper for observability
pub struct PromptDumper {
    /// Dump file path
    dump_path: Option<PathBuf>,
    /// Whether to dump to file
    dump_enabled: bool,
    /// Whether to print to stdout
    print_enabled: bool,
    /// Session ID
    session_id: Option<String>,
    /// Entries buffer
    entries: Vec<DumpEntry>,
    /// Maximum entries to buffer before flush
    buffer_size: usize,
}

impl PromptDumper {
    /// Create a new dumper
    pub fn new() -> Self {
        Self {
            dump_path: None,
            dump_enabled: false,
            print_enabled: false,
            session_id: None,
            entries: Vec::new(),
            buffer_size: 100,
        }
    }

    /// Enable file dumping
    pub fn enable_file_dump<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.dump_path = Some(path.into());
        self.dump_enabled = true;
        self
    }

    /// Enable stdout printing
    pub fn enable_print(mut self) -> Self {
        self.print_enabled = true;
        self
    }

    /// Set session ID
    pub fn with_session(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Set buffer size
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// Dump an assembled prompt
    pub fn dump(&mut self, prompt: &AssembledPrompt) {
        let entry = DumpEntry::from_prompt(prompt, self.session_id.clone());
        
        // Print if enabled
        if self.print_enabled {
            self.print_entry(&entry);
        }
        
        // Buffer entry
        self.entries.push(entry);
        
        // Flush if buffer is full
        if self.entries.len() >= self.buffer_size {
            self.flush();
        }
    }

    /// Dump with conversation ID
    pub fn dump_with_conversation(&mut self, prompt: &AssembledPrompt, conversation_id: String) {
        let entry = DumpEntry::from_prompt(prompt, self.session_id.clone())
            .with_conversation(conversation_id);
        
        if self.print_enabled {
            self.print_entry(&entry);
        }
        
        self.entries.push(entry);
        
        if self.entries.len() >= self.buffer_size {
            self.flush();
        }
    }

    /// Print entry to stdout
    fn print_entry(&self, entry: &DumpEntry) {
        println!("=== Prompt Dump ===");
        println!("Timestamp: {}", entry.timestamp);
        println!("Profile: {}", entry.profile);
        println!("Sections: {} cached, {} dynamic", entry.cached_sections, entry.dynamic_sections);
        println!("Tokens: {} cached, {} dynamic, {} total", 
                 entry.cached_tokens, entry.dynamic_tokens, entry.total_tokens);
        println!("Cache efficiency: {:.1}%", entry.cache_efficiency);
        println!("--- Prompt Content ---");
        
        // Limit output for readability
        if entry.prompt.len() > 2000 {
            println!("{}... (truncated, {} chars total)", 
                     &entry.prompt[..2000], entry.prompt.len());
        } else {
            println!("{}", entry.prompt);
        }
        
        println!("=== End Dump ===");
    }

    /// Flush buffer to file
    pub fn flush(&mut self) {
        if !self.dump_enabled || self.dump_path.is_none() || self.entries.is_empty() {
            return;
        }
        
        let path = self.dump_path.as_ref().unwrap();
        
        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).ok();
            }
        }
        
        // Open file for append
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok();
        
        if let Some(ref mut file) = file {
            for entry in &self.entries {
                let json = serde_json::to_string(entry).unwrap();
                writeln!(file, "{}", json).ok();
            }
        }
        
        self.entries.clear();
    }

    /// Get all entries
    pub fn entries(&self) -> &[DumpEntry] {
        &self.entries
    }

    /// Clear entries buffer
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Analyze prompt for debugging
    pub fn analyze_prompt(prompt: &str) -> PromptAnalysis {
        let mut analysis = PromptAnalysis::default();
        
        // Count sections
        let section_pattern = regex::Regex::new(r"\[([^\]]+)\]").unwrap();
        for cap in section_pattern.captures_iter(prompt) {
            analysis.sections.push(cap[1].to_string());
        }
        
        // Count XML tags
        let tag_pattern = regex::Regex::new(r"<([a-zA-Z_][a-zA-Z0-9_]*)>").unwrap();
        for cap in tag_pattern.captures_iter(prompt) {
            let tag = cap[1].to_string();
            analysis.xml_tags.push(tag.clone());
            analysis.xml_tag_counts.entry(tag).and_modify(|c| *c += 1).or_insert(1);
        }
        
        // Find cache boundary
        analysis.has_cache_boundary = prompt.contains(crate::prompt::CACHE_BOUNDARY);
        
        // Estimate tokens
        analysis.estimated_tokens = crate::prompt::cache::estimate_tokens(prompt);
        
        // Character count
        analysis.char_count = prompt.len();
        
        // Line count
        analysis.line_count = prompt.lines().count();
        
        analysis
    }
}

impl Default for PromptDumper {
    fn default() -> Self {
        Self::new()
    }
}

/// Analysis result for a prompt
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PromptAnalysis {
    /// Detected sections
    pub sections: Vec<String>,
    /// Detected XML tags
    pub xml_tags: Vec<String>,
    /// XML tag counts
    pub xml_tag_counts: std::collections::HashMap<String, usize>,
    /// Has cache boundary
    pub has_cache_boundary: bool,
    /// Estimated tokens
    pub estimated_tokens: usize,
    /// Character count
    pub char_count: usize,
    /// Line count
    pub line_count: usize,
}

impl PromptAnalysis {
    /// Print analysis summary
    pub fn print_summary(&self) {
        println!("Prompt Analysis Summary:");
        println!("  Sections: {:?}", self.sections);
        println!("  XML tags: {} unique, {:?} counts", self.xml_tags.len(), self.xml_tag_counts);
        println!("  Cache boundary: {}", self.has_cache_boundary);
        println!("  Tokens estimate: {}", self.estimated_tokens);
        println!("  Characters: {}", self.char_count);
        println!("  Lines: {}", self.line_count);
    }
}

/// Read dump entries from a JSONL file
pub fn read_dump_file<P: AsRef<Path>>(path: P) -> Vec<DumpEntry> {
    let path = path.as_ref();
    if !path.exists() {
        return Vec::new();
    }
    
    let content = std::fs::read_to_string(path).unwrap_or_default();
    content.lines()
        .filter_map(|line| serde_json::from_str::<DumpEntry>(line).ok())
        .collect()
}

/// Analyze a dump file for patterns
pub fn analyze_dump_file<P: AsRef<Path>>(path: P) -> DumpFileAnalysis {
    let entries = read_dump_file(path);
    
    let mut analysis = DumpFileAnalysis::default();
    analysis.total_entries = entries.len();
    
    for entry in &entries {
        analysis.total_tokens += entry.total_tokens;
        analysis.avg_tokens += entry.total_tokens;
        analysis.profile_counts.entry(entry.profile.clone()).and_modify(|c| *c += 1).or_insert(1);
        
        if entry.cache_efficiency > analysis.max_cache_efficiency {
            analysis.max_cache_efficiency = entry.cache_efficiency;
        }
        if entry.cache_efficiency < analysis.min_cache_efficiency || analysis.min_cache_efficiency == 0.0 {
            analysis.min_cache_efficiency = entry.cache_efficiency;
        }
    }
    
    if analysis.total_entries > 0 {
        analysis.avg_tokens /= analysis.total_entries;
        analysis.avg_cache_efficiency = entries.iter().map(|e| e.cache_efficiency).sum::<f64>() / analysis.total_entries as f64;
    }
    
    analysis
}

/// Analysis of a dump file
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DumpFileAnalysis {
    /// Total entries
    pub total_entries: usize,
    /// Total tokens across all entries
    pub total_tokens: usize,
    /// Average tokens per entry
    pub avg_tokens: usize,
    /// Profile counts
    pub profile_counts: std::collections::HashMap<String, usize>,
    /// Max cache efficiency
    pub max_cache_efficiency: f64,
    /// Min cache efficiency
    pub min_cache_efficiency: f64,
    /// Average cache efficiency
    pub avg_cache_efficiency: f64,
}

impl DumpFileAnalysis {
    pub fn print_summary(&self) {
        println!("Dump File Analysis:");
        println!("  Total entries: {}", self.total_entries);
        println!("  Total tokens: {}", self.total_tokens);
        println!("  Average tokens: {}", self.avg_tokens);
        println!("  Profile distribution: {:?}", self.profile_counts);
        println!("  Cache efficiency: min {:.1}%, max {:.1}%, avg {:.1}%",
                 self.min_cache_efficiency, self.max_cache_efficiency, self.avg_cache_efficiency);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dump_entry_creation() {
        let mut orchestrator = crate::prompt::PromptOrchestrator::new(std::env::current_dir().unwrap());
        orchestrator.add_section(crate::prompt::PromptSection::static_section("test", "test content"));
        
        let assembled = orchestrator.assemble();
        let entry = DumpEntry::from_prompt(&assembled, Some("session-1".to_string()));
        
        assert_eq!(entry.profile, "default");
        assert!(entry.prompt.contains("test"));
        assert_eq!(entry.session_id, Some("session-1".to_string()));
    }

    #[test]
    fn test_dumper_basic() {
        let mut dumper = PromptDumper::new().enable_print();
        
        let mut orchestrator = crate::prompt::PromptOrchestrator::new(std::env::current_dir().unwrap());
        orchestrator.add_section(crate::prompt::PromptSection::static_section("identity", "You are AI"));
        
        let assembled = orchestrator.assemble();
        dumper.dump(&assembled);
        
        assert_eq!(dumper.entries().len(), 1);
    }

    #[test]
    fn test_analyze_prompt() {
        let prompt = "[identity]\nYou are AI\n\n<context>\nSome context\n</context>";
        let analysis = PromptDumper::analyze_prompt(prompt);
        
        assert!(analysis.sections.contains(&"identity".to_string()));
        assert!(analysis.xml_tags.contains(&"context".to_string()));
        assert!(!analysis.has_cache_boundary);
        assert!(analysis.estimated_tokens > 0);
    }

    #[test]
    fn test_prompt_analysis_summary() {
        let prompt = "[test]\nContent";
        let analysis = PromptDumper::analyze_prompt(prompt);
        analysis.print_summary();
    }

    #[test]
    fn test_dump_file_analysis() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let path = temp_file.path();
        
        // Write some entries
        let mut dumper = PromptDumper::new()
            .enable_file_dump(path)
            .with_session("test-session".to_string());
        
        let mut orchestrator = crate::prompt::PromptOrchestrator::new(std::env::current_dir().unwrap());
        orchestrator.add_section(crate::prompt::PromptSection::static_section("test", "content"));
        
        for _ in 0..5 {
            let assembled = orchestrator.assemble();
            dumper.dump(&assembled);
        }
        dumper.flush();
        
        // Analyze
        let analysis = analyze_dump_file(path);
        assert_eq!(analysis.total_entries, 5);
        assert!(analysis.avg_tokens > 0);
        analysis.print_summary();
    }
}