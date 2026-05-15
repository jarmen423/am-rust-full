use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceGraphNode {
    pub node_id: String,
    pub node_type: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceGraphEdge {
    pub edge_id: String,
    pub from_node_id: String,
    pub to_node_id: String,
    pub relation_type: String,
    pub label: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceGraphSeed {
    pub seed_type: String,
    pub seed_id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceGraphPayload {
    pub status: String,
    pub seed: WorkspaceGraphSeed,
    pub nodes: Vec<WorkspaceGraphNode>,
    pub edges: Vec<WorkspaceGraphEdge>,
}

/// Response wrapper for graph explore/note/board/entity routes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphResponse {
    pub status: String,
    pub seed: WorkspaceGraphSeed,
    pub nodes: Vec<WorkspaceGraphNode>,
    pub edges: Vec<WorkspaceGraphEdge>,
}
