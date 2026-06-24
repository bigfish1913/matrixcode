//! Context collector for AI review.
//!
//! Collects LSP diagnostics, CodeGraph symbols, and other context information
//! to provide comprehensive review context.

use anyhow::Result;
use std::path::Path;

use super::{ReviewContext, SymbolInfo, SymbolKind};

/// Collect review context for a file.
pub fn collect_context(file_path: &Path, project_path: Option<&Path>) -> ReviewContext {
    let mut context = ReviewContext::default();
    
    // Collect CodeGraph information if project path is available
    if let Some(project) = project_path {
        if let Ok(codegraph_ctx) = collect_codegraph_context(file_path, project) {
            context.symbols = codegraph_ctx.symbols;
            context.callers = codegraph_ctx.callers;
            context.callees = codegraph_ctx.callees;
            context.related_files = codegraph_ctx.related_files;
        }
    }
    
    // TODO: Collect LSP diagnostics when LSP integration is ready
    // context.lsp_diagnostics = collect_lsp_diagnostics(file_path);
    
    context
}

/// CodeGraph context collection result
struct CodegraphContext {
    symbols: Vec<SymbolInfo>,
    callers: Vec<String>,
    callees: Vec<String>,
    related_files: Vec<String>,
}

/// Collect CodeGraph context for a file.
fn collect_codegraph_context(file_path: &Path, project_path: &Path) -> Result<CodegraphContext> {
    use crate::tools::codegraph::{CodeGraphManager, Node};
    
    let manager = CodeGraphManager::new(project_path);
    
    // Get file-relative path
    let relative_path = file_path.strip_prefix(project_path)
        .unwrap_or(file_path)
        .to_string_lossy()
        .to_string();
    
    // Search for symbols in this file
    let symbols = collect_file_symbols(&manager, &relative_path)?;
    
    // Get callers and callees for the symbols
    let mut callers = Vec::new();
    let mut callees = Vec::new();
    let mut related_files = Vec::new();
    
    for sym in &symbols {
        // Find callers (who uses this symbol)
        if let Ok(callers_nodes) = manager.callers(&sym.name, 5) {
            for node in callers_nodes {
                let caller_info = format!("{} ({})", node.name, node.file_path);
                if !callers.contains(&caller_info) {
                    callers.push(caller_info);
                }
                // Add related files
                if !related_files.contains(&node.file_path) {
                    related_files.push(node.file_path.clone());
                }
            }
        }
        
        // Find callees (what this symbol depends on)
        if let Ok(callees_nodes) = manager.callees(&sym.name, 5) {
            for node in callees_nodes {
                let callee_info = format!("{} ({})", node.name, node.file_path);
                if !callees.contains(&callee_info) {
                    callees.push(callee_info);
                }
            }
        }
    }
    
    Ok(CodegraphContext {
        symbols,
        callers,
        callees,
        related_files,
    })
}

/// Collect symbols defined in a specific file.
fn collect_file_symbols(manager: &crate::tools::codegraph::CodeGraphManager, file_path: &str) -> Result<Vec<SymbolInfo>> {
    use crate::tools::codegraph::Node;
    use rusqlite::Row;
    
    // Query symbols in this file
    let conn = manager.connect()?;
    let mut stmt = conn.prepare(
        "SELECT id, kind, name, qualified_name, file_path, language,
                start_line, end_line, start_column, end_column,
                signature, docstring, visibility, is_exported, is_async
         FROM nodes
         WHERE file_path LIKE ?
         ORDER BY start_line
         LIMIT 20"
    )?;
    
    let pattern = format!("%{}%", file_path);
    let nodes = stmt
        .query_map(rusqlite::params![&pattern], |row: &Row| {
            Ok(Node {
                id: row.get(0)?,
                kind: row.get(1)?,
                name: row.get(2)?,
                qualified_name: row.get(3)?,
                file_path: row.get(4)?,
                language: row.get(5)?,
                start_line: row.get(6)?,
                end_line: row.get(7)?,
                start_column: row.get(8)?,
                end_column: row.get(9)?,
                signature: row.get(10)?,
                docstring: row.get(11)?,
                visibility: row.get(12)?,
                is_exported: row.get::<_, i32>(13)? != 0,
                is_async: row.get::<_, i32>(14)? != 0,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    
    // Convert to SymbolInfo
    let symbols = nodes
        .into_iter()
        .filter_map(|node| {
            let kind = parse_symbol_kind(&node.kind);
            Some(SymbolInfo {
                name: node.name,
                kind,
                signature: node.signature,
                doc: node.docstring,
            })
        })
        .collect();
    
    Ok(symbols)
}

/// Parse symbol kind from string.
fn parse_symbol_kind(kind_str: &str) -> SymbolKind {
    match kind_str.to_lowercase().as_str() {
        "function" | "func" => SymbolKind::Function,
        "class" => SymbolKind::Class,
        "method" => SymbolKind::Method,
        "variable" | "var" | "field" => SymbolKind::Variable,
        "constant" | "const" => SymbolKind::Constant,
        "module" | "namespace" => SymbolKind::Module,
        "interface" | "trait" => SymbolKind::Interface,
        "type" | "typedef" | "enum" => SymbolKind::Type,
        _ => SymbolKind::Function, // Default
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_symbol_kind() {
        assert_eq!(parse_symbol_kind("function"), SymbolKind::Function);
        assert_eq!(parse_symbol_kind("class"), SymbolKind::Class);
        assert_eq!(parse_symbol_kind("method"), SymbolKind::Method);
        assert_eq!(parse_symbol_kind("variable"), SymbolKind::Variable);
    }
}