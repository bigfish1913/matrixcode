//! Workflow Layout Algorithm
//!
//! Computes DAG layout positions for rendering

use matrixcode_core::workflow::WorkflowDef;
use std::collections::HashMap;

/// Layout computation result
pub struct LayoutResult {
    /// Node positions (row, col)
    pub positions: HashMap<String, (usize, usize)>,
    /// Layers (nodes grouped by depth)
    pub layers: Vec<Vec<String>>,
    /// Grid dimensions
    pub width: usize,
    pub height: usize,
}

/// Compute layout from workflow definition
pub fn compute_layout(def: &WorkflowDef) -> LayoutResult {
    let mut positions = HashMap::new();
    let mut layers: Vec<Vec<String>> = Vec::new();
    let mut visited: HashMap<String, bool> = HashMap::new();

    // Find start node
    let start_node = def
        .nodes
        .iter()
        .find(|n| n.node_type == matrixcode_core::workflow::NodeType::Start)
        .map(|n| n.id.clone());

    if let Some(start) = start_node {
        // BFS layer assignment
        let mut current_layer = vec![start.clone()];
        visited.insert(start.clone(), true);

        while !current_layer.is_empty() {
            // Sort layer nodes for consistent ordering
            let mut sorted_layer = current_layer.clone();
            sorted_layer.sort();
            layers.push(sorted_layer.clone());

            // Record positions
            for (col, node_id) in sorted_layer.iter().enumerate() {
                positions.insert(node_id.clone(), (layers.len() - 1, col));
            }

            // Find next layer
            let mut next_layer = Vec::new();
            for node_id in &current_layer {
                for edge in &def.edges {
                    if &edge.from == node_id && !visited.contains_key(&edge.to) {
                        // Check if target node exists
                        if def.nodes.iter().any(|n| n.id == edge.to) {
                            visited.insert(edge.to.clone(), true);
                            next_layer.push(edge.to.clone());
                        }
                    }
                }
            }

            current_layer = next_layer;
        }

        // Add any unvisited nodes to last layer (fallback)
        for node in &def.nodes {
            if !visited.contains_key(&node.id) {
                if let Some(last_layer) = layers.last_mut() {
                    last_layer.push(node.id.clone());
                    let col = last_layer.len() - 1;
                    positions.insert(node.id.clone(), (layers.len() - 1, col));
                } else {
                    layers.push(vec![node.id.clone()]);
                    positions.insert(node.id.clone(), (0, 0));
                }
            }
        }
    }

    let height = layers.len();
    let width = layers.iter().map(|l| l.len()).max().unwrap_or(0);

    LayoutResult {
        positions,
        layers,
        width,
        height,
    }
}

/// Calculate edge path points
pub fn calculate_edge_path(
    from_pos: (usize, usize),
    to_pos: (usize, usize),
    node_width: usize,
    node_height: usize,
    spacing_x: usize,
    spacing_y: usize,
) -> Vec<(usize, usize)> {
    let from_x = from_pos.1 * (node_width + spacing_x) + node_width / 2;
    let from_y = from_pos.0 * (node_height + spacing_y) + node_height;
    let to_x = to_pos.1 * (node_width + spacing_x) + node_width / 2;
    let to_y = to_pos.0 * (node_height + spacing_y);

    // Simple vertical path
    let mut points = Vec::new();
    points.push((from_x, from_y));

    // If same column, direct vertical line
    if from_x == to_x {
        for y in from_y + 1..to_y {
            points.push((from_x, y));
        }
    } else {
        // Different columns: need horizontal segment
        let mid_y = (from_y + to_y) / 2;
        for y in from_y + 1..mid_y {
            points.push((from_x, y));
        }
        for x in from_x..to_x {
            points.push((x, mid_y));
        }
        for y in mid_y..to_y {
            points.push((to_x, y));
        }
    }

    points.push((to_x, to_y));
    points
}
