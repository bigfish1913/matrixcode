//! Workflow Visualization Types
//!
//! Defines visual state for workflow DAG rendering

use matrixcode_core::workflow::{
    NodeStatus, NodeType, WorkflowContext, WorkflowDef, WorkflowPersistence, WorkflowRegistry,
};
use std::collections::HashMap;
use std::path::PathBuf;

/// Workflow visualization state
pub struct WorkflowViewState {
    /// Current workflow definition (for DAG structure)
    pub workflow_def: Option<WorkflowDef>,
    /// Execution context (for status)
    pub context: Option<WorkflowContext>,
    /// Visual layout cache
    pub layout: DagLayout,
    /// Panel visibility
    pub visible: bool,
    /// Selected node (for inspection)
    pub selected_node: Option<String>,
    /// View mode
    pub view_mode: WorkflowViewMode,
    /// Spinner frame for running nodes
    pub spinner_frame: usize,
}

impl Default for WorkflowViewState {
    fn default() -> Self {
        Self {
            workflow_def: None,
            context: None,
            layout: DagLayout::default(),
            visible: false,
            selected_node: None,
            view_mode: WorkflowViewMode::Dag,
            spinner_frame: 0,
        }
    }
}

/// DAG layout cache (computed from WorkflowDef)
#[derive(Default)]
pub struct DagLayout {
    /// Node positions (row, col) in grid coordinates
    pub node_positions: HashMap<String, (usize, usize)>,
    /// Edge connections (from, to)
    pub edges: Vec<EdgeInfo>,
    /// Layer nodes (nodes grouped by layer)
    pub layers: Vec<Vec<String>>,
    /// Layout dimensions (rows, cols)
    pub height: usize,
    pub width: usize,
}

/// Edge connection info
pub struct EdgeInfo {
    pub from: String,
    pub to: String,
    pub condition: Option<String>,
}

/// View modes
pub enum WorkflowViewMode {
    /// Full DAG visualization
    Dag,
    /// Compact progress bar + node list
    Progress,
    /// Node detail panel
    Detail,
}

/// Visual node representation
pub struct VisualNode {
    pub id: String,
    pub name: String,
    pub node_type: NodeType,
    pub status: NodeVisualStatus,
    pub position: (usize, usize),
    pub elapsed_ms: Option<u64>,
}

/// Visual node status
pub enum NodeVisualStatus {
    Pending,
    Running,
    Completed,
    Failed { error: String },
    Skipped,
}

impl NodeVisualStatus {
    /// Get status icon
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Pending => "○",
            Self::Running => "⟳",
            Self::Completed => "✓",
            Self::Failed { .. } => "✗",
            Self::Skipped => "→",
        }
    }

    /// Get status color name (for ratatui styling)
    pub fn color(&self) -> &'static str {
        match self {
            Self::Pending => "gray",
            Self::Running => "yellow",
            Self::Completed => "green",
            Self::Failed { .. } => "red",
            Self::Skipped => "blue",
        }
    }
}

/// Get node type icon
pub fn node_type_icon(node_type: &NodeType) -> &'static str {
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

/// Convert WorkflowContext node status to visual status
pub fn to_visual_status(status: &NodeStatus, error: Option<&String>) -> NodeVisualStatus {
    match status {
        NodeStatus::Pending => NodeVisualStatus::Pending,
        NodeStatus::Running => NodeVisualStatus::Running,
        NodeStatus::Completed => NodeVisualStatus::Completed,
        NodeStatus::Failed => NodeVisualStatus::Failed {
            error: error.cloned().unwrap_or_default(),
        },
        NodeStatus::Skipped => NodeVisualStatus::Skipped,
    }
}

impl WorkflowViewState {
    /// Update from workflow definition
    pub fn set_workflow(&mut self, def: WorkflowDef) {
        self.workflow_def = Some(def.clone());
        self.layout = compute_layout(&def);
        self.visible = true;
    }

    /// Update from execution context
    pub fn update_context(&mut self, context: WorkflowContext) {
        self.context = Some(context);
    }

    /// Load workflow instances from persistence
    /// Returns a list of recent workflow contexts (most recent first)
    pub fn load_recent_instances(project_dir: Option<&PathBuf>) -> Vec<WorkflowContext> {
        let persistence = WorkflowPersistence::new(project_dir);
        if let Ok(contexts) = persistence.list() {
            // Sort by updated_at descending (most recent first)
            let mut sorted = contexts;
            sorted.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            // Return only running/paused workflows, or the 5 most recent
            let running: Vec<_> = sorted
                .iter()
                .filter(|c| {
                    matches!(
                        c.status,
                        matrixcode_core::workflow::WorkflowStatus::Running
                            | matrixcode_core::workflow::WorkflowStatus::Paused
                    )
                })
                .cloned()
                .collect();
            if running.is_empty() {
                sorted.into_iter().take(5).collect()
            } else {
                running
            }
        } else {
            Vec::new()
        }
    }

    /// Load workflow definition from registry
    pub fn load_workflow_def(
        project_dir: Option<&PathBuf>,
        workflow_id: &str,
    ) -> Option<WorkflowDef> {
        let registry = WorkflowRegistry::new(project_dir);
        if let Some(info) = registry.get(workflow_id) {
            matrixcode_core::workflow::parse_workflow_from_file(&info.path).ok()
        } else {
            None
        }
    }

    /// Load and display the most recent workflow instance
    pub fn load_most_recent(&mut self, project_dir: Option<&PathBuf>) {
        let instances = Self::load_recent_instances(project_dir);
        if let Some(ctx) = instances.first() {
            // Load the workflow definition
            if let Some(def) = Self::load_workflow_def(project_dir, &ctx.workflow_id) {
                self.set_workflow(def);
                self.update_context(ctx.clone());
            }
        }
    }

    /// Get node visual status
    pub fn get_node_status(&self, node_id: &str) -> NodeVisualStatus {
        if let Some(ctx) = &self.context
            && let Some(exec) = ctx.node_executions.get(node_id)
        {
            return to_visual_status(&exec.status, exec.error.as_ref());
        }
        NodeVisualStatus::Pending
    }

    /// Get progress percentage
    pub fn progress(&self) -> (usize, usize) {
        if let Some(ctx) = &self.context {
            let total = self.layout.node_positions.len();
            let completed = ctx.execution_path.len();
            return (completed, total);
        }
        (0, self.layout.node_positions.len())
    }

    /// Advance spinner frame
    pub fn advance_spinner(&mut self) {
        self.spinner_frame = (self.spinner_frame + 1) % 10;
    }

    /// Get spinner char for current frame
    pub fn spinner_char(&self) -> char {
        const SPINNER: &[char] = &['⠁', '⠃', '⠇', '⡇', '⣇', '⣧', '⣷', '⣿', '⢟', '⠟'];
        SPINNER[self.spinner_frame % SPINNER.len()]
    }
}

/// Compute DAG layout from workflow definition
fn compute_layout(def: &WorkflowDef) -> DagLayout {
    let mut layout = DagLayout::default();

    // Build layers using topological sort
    let mut visited: HashMap<String, bool> = HashMap::new();
    let mut layers: Vec<Vec<String>> = Vec::new();

    // Find start node
    let start_node = def
        .nodes
        .iter()
        .find(|n| n.node_type == NodeType::Start)
        .map(|n| n.id.clone());

    if let Some(start) = start_node {
        // BFS to assign layers
        let mut current_layer = vec![start.clone()];
        visited.insert(start.clone(), true);

        while !current_layer.is_empty() {
            layers.push(current_layer.clone());
            let mut next_layer = Vec::new();

            for node_id in &current_layer {
                // Find edges from this node
                for edge in &def.edges {
                    if &edge.from == node_id && !visited.contains_key(&edge.to) {
                        visited.insert(edge.to.clone(), true);
                        next_layer.push(edge.to.clone());
                    }
                }
            }

            current_layer = next_layer;
        }
    }

    // Assign positions based on layers
    for (row, layer) in layers.iter().enumerate() {
        for (col, node_id) in layer.iter().enumerate() {
            layout.node_positions.insert(node_id.clone(), (row, col));
        }
    }

    layout.layers = layers.clone();
    layout.height = layers.len();
    layout.width = layers.iter().map(|l| l.len()).max().unwrap_or(0);

    // Build edge info
    for edge in &def.edges {
        layout.edges.push(EdgeInfo {
            from: edge.from.clone(),
            to: edge.to.clone(),
            condition: edge.condition.clone(),
        });
    }

    layout
}
