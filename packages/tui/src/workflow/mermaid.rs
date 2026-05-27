//! Mermaid Export
//!
//! Export workflow DAG to Mermaid diagram syntax

use matrixcode_core::workflow::{WorkflowDef, WorkflowContext, NodeType};

/// Export workflow to Mermaid flowchart syntax
pub fn export_mermaid(def: &WorkflowDef, ctx: Option<&WorkflowContext>) -> String {
    let mut output = String::new();

    // Header
    output.push_str("```mermaid\n");
    output.push_str("flowchart TD\n");

    // Node definitions with labels
    for node in &def.nodes {
        let label = &node.name;
        let icon = node_type_icon_mermaid(&node.node_type);

        // Determine status for styling
        let status_class = if let Some(context) = ctx {
            if let Some(exec) = context.node_executions.get(&node.id) {
                match exec.status {
                    matrixcode_core::workflow::NodeStatus::Pending => "pending",
                    matrixcode_core::workflow::NodeStatus::Running => "running",
                    matrixcode_core::workflow::NodeStatus::Completed => "completed",
                    matrixcode_core::workflow::NodeStatus::Failed => "failed",
                    matrixcode_core::workflow::NodeStatus::Skipped => "skipped",
                }
            } else {
                "pending"
            }
        } else {
            ""
        };

        // Sanitize node ID for mermaid (replace special chars)
        let safe_id = sanitize_id(&node.id);

        // Node definition
        output.push_str(&format!("    {}[\"{} {}\"]\n", safe_id, icon, label));

        // Apply class for styling
        if !status_class.is_empty() {
            output.push_str(&format!("    class {} {}\n", safe_id, status_class));
        }
    }

    // Edge definitions
    for edge in &def.edges {
        let from_safe = sanitize_id(&edge.from);
        let to_safe = sanitize_id(&edge.to);

        if let Some(ref condition) = edge.condition {
            output.push_str(&format!("    {} -- {} --> {}\n", from_safe, condition, to_safe));
        } else {
            output.push_str(&format!("    {} --> {}\n", from_safe, to_safe));
        }
    }

    // Style definitions
    output.push('\n');
    output.push_str("    classDef pending fill:#f9f9f9,stroke:#999,stroke-width:1px\n");
    output.push_str("    classDef running fill:#fff3cd,stroke:#ffc107,stroke-width:3px\n");
    output.push_str("    classDef completed fill:#d4edda,stroke:#28a745,stroke-width:2px\n");
    output.push_str("    classDef failed fill:#f8d7da,stroke:#dc3545,stroke-width:2px\n");
    output.push_str("    classDef skipped fill:#e2e3e5,stroke:#6c757d,stroke-width:1px\n");

    // Footer
    output.push_str("```\n");

    output
}

/// Export workflow to Mermaid with execution summary
pub fn export_mermaid_with_summary(def: &WorkflowDef, ctx: &WorkflowContext) -> String {
    let mut output = export_mermaid(def, Some(ctx));

    // Add execution summary
    output.push_str("\n## Execution Summary\n\n");

    let status = match ctx.status {
        matrixcode_core::workflow::WorkflowStatus::Pending => "Pending",
        matrixcode_core::workflow::WorkflowStatus::Running => "Running",
        matrixcode_core::workflow::WorkflowStatus::Completed => "✅ Completed",
        matrixcode_core::workflow::WorkflowStatus::Failed => "❌ Failed",
        matrixcode_core::workflow::WorkflowStatus::Paused => "⏸️ Paused",
        matrixcode_core::workflow::WorkflowStatus::Cancelled => "🚫 Cancelled",
    };

    output.push_str(&format!("**Status**: {}\n\n", status));
    output.push_str(&format!("**Instance ID**: {}\n\n", ctx.instance_id));
    output.push_str(&format!("**Nodes executed**: {}\n\n", ctx.execution_path.len()));

    if let Some(ref error) = ctx.error {
        output.push_str(&format!("**Error**: {}\n\n", error));
    }

    // Node execution details
    output.push_str("### Node Details\n\n");
    output.push_str("| Node | Status | Duration |\n");
    output.push_str("|------|--------|----------|\n");

    for node in &def.nodes {
        let exec = ctx.node_executions.get(&node.id);
        let (status_icon, duration) = if let Some(e) = exec {
            let icon = match e.status {
                matrixcode_core::workflow::NodeStatus::Pending => "○",
                matrixcode_core::workflow::NodeStatus::Running => "⟳",
                matrixcode_core::workflow::NodeStatus::Completed => "✓",
                matrixcode_core::workflow::NodeStatus::Failed => "✗",
                matrixcode_core::workflow::NodeStatus::Skipped => "→",
            };
            let duration = if let (Some(start), Some(end)) = (e.started_at, e.finished_at) {
                let ms = (end - start).num_milliseconds();
                format!("{}ms", ms)
            } else {
                "-".to_string()
            };
            (icon, duration)
        } else {
            ("○", "-".to_string())
        };

        output.push_str(&format!("| {} | {} {} | {} |\n", node.name, status_icon, node.id, duration));
    }

    output
}

/// Get node type icon for mermaid
fn node_type_icon_mermaid(node_type: &NodeType) -> &'static str {
    match node_type {
        NodeType::Start => "▶",
        NodeType::End => "■",
        NodeType::Task => "⚙",
        NodeType::Condition => "◇",
        NodeType::Parallel => "║",
        NodeType::Approval => "?",
        NodeType::Wait => "⏳",
        NodeType::SubWorkflow => "↳",
    }
}

/// Sanitize node ID for mermaid syntax
/// Mermaid has reserved keywords like 'end', 'start', 'subgraph' that cannot be used as node IDs
fn sanitize_id(id: &str) -> String {
    // Mermaid reserved keywords that cause parse errors
    const RESERVED: &[&str] = &["end", "start", "subgraph", "direction", "style", "class", "linkstyle"];

    // First sanitize characters
    let sanitized: String = id.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    // Prefix reserved keywords with 'n_' to avoid conflicts
    if RESERVED.contains(&sanitized.as_str()) {
        format!("n_{}", sanitized)
    } else {
        sanitized
    }
}

/// Export workflow instance to file
pub fn export_workflow_mermaid(
    def: &WorkflowDef,
    ctx: Option<&WorkflowContext>,
    output_path: &std::path::Path,
) -> std::io::Result<()> {
    let content = if let Some(context) = ctx {
        export_mermaid_with_summary(def, context)
    } else {
        export_mermaid(def, None)
    };

    std::fs::write(output_path, content)
}