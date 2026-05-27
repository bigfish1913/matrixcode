//! Data structures for CodeGraph integration.

use serde::{Deserialize, Serialize};

/// Code symbol node from CodeGraph index.
#[derive(Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub qualified_name: String,
    pub file_path: String,
    pub language: String,
    pub start_line: u32,
    pub end_line: u32,
    pub start_column: u32,
    pub end_column: u32,
    pub signature: Option<String>,
    pub docstring: Option<String>,
    pub visibility: Option<String>,
    pub is_exported: bool,
    pub is_async: bool,
}

/// Edge representing relationship between nodes.
#[derive(Debug, Serialize, Deserialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub kind: String,
    pub line: Option<u32>,
}

/// Index status information.
#[derive(Debug, Serialize, Deserialize)]
pub struct IndexStatus {
    pub initialized: bool,
    pub file_count: u32,
    pub node_count: u32,
    pub edge_count: u32,
    pub languages: Vec<String>,
    pub pending_changes: PendingChanges,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PendingChanges {
    pub added: u32,
    pub modified: u32,
    pub removed: u32,
}

/// Environment type for CodeGraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeGraphEnv {
    Git,
    NonGit,
}

/// File information for indexing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: String,
    pub language: String,
    pub size: u64,
    pub modified: u64,
    pub node_count: Option<u32>,
}