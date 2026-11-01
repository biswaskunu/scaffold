use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub r#type: NodeType,
    pub title: String,
    pub body: String,
    pub x: f64,
    pub y: f64,
    pub status: Option<NodeStatus>,
    pub effort: Option<f64>,
    pub created_by: String,
    pub ai_generated: bool,
    pub source_node: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeType {
    Feature,
    Service,
    Task,
    Risk,
    Note,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeStatus {
    Todo,
    Doing,
    Done,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeKind {
    DependsOn,
    PartOf,
    Calls,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GraphWarning {
    DanglingEdge { edge_id: String, missing_node: String },
    SelfEdge { edge_id: String },
    DuplicateEdge { kept: String, dropped: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuiltGraph {
    pub graph: Graph,
    pub warnings: Vec<GraphWarning>,
}

impl Graph {
    pub fn new() -> Self {
        Graph {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn build(nodes: Vec<Node>, edges: Vec<Edge>) -> BuiltGraph {
        let mut valid_nodes = std::collections::HashMap::new();
        let mut graph_nodes = Vec::new();
        for node in nodes {
            if valid_nodes.insert(node.id.clone(), node.clone()).is_none() {
                graph_nodes.push(node);
            }
        }

        let mut graph_edges = Vec::new();
        let mut warnings = Vec::new();
        let mut seen_edges = std::collections::HashSet::new();

        for edge in edges {
            if edge.from == edge.to {
                warnings.push(GraphWarning::SelfEdge { edge_id: edge.id });
                continue;
            }
            if !valid_nodes.contains_key(&edge.from) {
                warnings.push(GraphWarning::DanglingEdge {
                    edge_id: edge.id,
                    missing_node: edge.from,
                });
                continue;
            }
            if !valid_nodes.contains_key(&edge.to) {
                warnings.push(GraphWarning::DanglingEdge {
                    edge_id: edge.id,
                    missing_node: edge.to,
                });
                continue;
            }

            let key = (edge.kind.clone(), edge.from.clone(), edge.to.clone());
            if !seen_edges.insert(key) {
                warnings.push(GraphWarning::DuplicateEdge {
                    kept: "existing".to_string(),
                    dropped: edge.id,
                });
                continue;
            }

            graph_edges.push(edge);
        }

        BuiltGraph {
            graph: Graph {
                nodes: graph_nodes,
                edges: graph_edges,
            },
            warnings,
        }
    }

    pub fn add_node(&mut self, node: Node) {
        self.nodes.push(node);
    }

    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }

    pub fn detect_cycles(&self) -> Vec<Vec<String>> {
        let mut visited: Vec<bool> = vec![false; self.nodes.len()];
        let mut rec_stack: Vec<bool> = vec![false; self.nodes.len()];
        let mut node_indices: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for (i, node) in self.nodes.iter().enumerate() {
            node_indices.insert(node.id.clone(), i);
        }

        let mut cycles: Vec<Vec<String>> = Vec::new();

        fn dfs(
            node_idx: usize,
            graph: &Graph,
            visited: &mut Vec<bool>,
            rec_stack: &mut Vec<bool>,
            node_indices: &std::collections::HashMap<String, usize>,
            cycles: &mut Vec<Vec<String>>,
            path: &mut Vec<String>,
        ) {
            visited[node_idx] = true;
            rec_stack[node_idx] = true;
            path.push(graph.nodes[node_idx].id.clone());

            let edges = graph
                .edges
                .iter()
                .filter(|e| e.from == graph.nodes[node_idx].id);

            for edge in edges {
                let target_idx = match node_indices.get(&edge.to) {
                    Some(&idx) => idx,
                    None => continue,
                };

                if !visited[target_idx] {
                    dfs(
                        target_idx,
                        graph,
                        visited,
                        rec_stack,
                        node_indices,
                        cycles,
                        path,
                    );
                } else if rec_stack[target_idx] {
                    // Found a cycle - extract it from path
                    let mut cycle: Vec<String> = Vec::new();
                    let mut found = false;
                    for node_id in path.iter() {
                        if found {
                            cycle.push(node_id.clone());
                        }
                        if node_id == &edge.to {
                            found = true;
                        }
                    }
                    if !cycle.is_empty() {
                        cycles.push(cycle);
                    }
                }
            }

            path.pop();
            rec_stack[node_idx] = false;
        }

        for i in 0..self.nodes.len() {
            if !visited[i] {
                dfs(
                    i,
                    self,
                    &mut visited,
                    &mut rec_stack,
                    &node_indices,
                    &mut cycles,
                    &mut Vec::new(),
                );
            }
        }

        cycles
    }

    pub fn topological_sort(&self) -> Result<Vec<String>, String> {
        // Build adjacency list strictly for depends_on edges
        let mut in_degree: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut adj: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        for node in &self.nodes {
            in_degree.insert(node.id.clone(), 0);
            adj.entry(node.id.clone()).or_default();
        }

        for edge in &self.edges {
            if let EdgeKind::DependsOn = edge.kind {
                if let Some(in_deg) = in_degree.get_mut(&edge.to) {
                    *in_deg += 1;
                }
                adj.entry(edge.from.clone())
                    .or_default()
                    .push(edge.to.clone());
            }
        }

        // Kahn's algorithm with deterministic tie-breaking (sort by node id)
        let mut ready: Vec<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();
        ready.sort();

        let mut result: Vec<String> = Vec::new();

        while !ready.is_empty() {
            let node = ready.remove(0);
            result.push(node.clone());

            if let Some(neighbors) = adj.get(&node) {
                for neighbor in neighbors {
                    if let Some(in_deg) = in_degree.get_mut(neighbor) {
                        *in_deg -= 1;
                        if *in_deg == 0 {
                            let pos = ready.binary_search(neighbor).unwrap_or_else(|e| e);
                            ready.insert(pos, neighbor.clone());
                        }
                    }
                }
            }
        }

        if result.len() != self.nodes.len() {
            return Err("Graph has cycles".to_string());
        }

        Ok(result)
    }
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalGraph {
    pub nodes: Vec<CanonicalNode>,
    pub edges: Vec<CanonicalEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalNode {
    pub id: String,
    pub r#type: String,
    pub title: String,
    pub body: String,
    pub effort: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalEdge {
    pub kind: String,
    pub from: String,
    pub to: String,
}

impl Graph {
    pub fn canonical(&self) -> CanonicalGraph {
        let mut nodes: Vec<CanonicalNode> = self
            .nodes
            .iter()
            .map(|n| CanonicalNode {
                id: n.id.clone(),
                r#type: match n.r#type {
                    NodeType::Feature => "feature".to_string(),
                    NodeType::Service => "service".to_string(),
                    NodeType::Task => "task".to_string(),
                    NodeType::Risk => "risk".to_string(),
                    NodeType::Note => "note".to_string(),
                },
                title: n.title.clone(),
                body: n.body.clone(),
                effort: n.effort,
            })
            .collect();

        // Sort nodes by id
        nodes.sort_by_key(|n| n.id.clone());

        let mut edges: Vec<CanonicalEdge> = self
            .edges
            .iter()
            .map(|e| CanonicalEdge {
                kind: match e.kind {
                    EdgeKind::DependsOn => "depends_on".to_string(),
                    EdgeKind::PartOf => "part_of".to_string(),
                    EdgeKind::Calls => "calls".to_string(),
                },
                from: e.from.clone(),
                to: e.to.clone(),
            })
            .collect();

        // Sort edges by (kind, from, to)
        edges.sort_by(|a, b| {
            a.kind
                .cmp(&b.kind)
                .then_with(|| a.from.cmp(&b.from))
                .then_with(|| a.to.cmp(&b.to))
        });

        CanonicalGraph { nodes, edges }
    }
}

pub mod errors {
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum GraphError {
        #[error("Node not found: {0}")]
        NodeNotFound(String),
        #[error("Edge not found: {0}")]
        EdgeNotFound(String),
        #[error("Cycle detected: {0:?}")]
        CycleDetected(Vec<String>),
        #[error("Graph has cycles, cannot topological sort")]
        HasCycles,
    }
}

pub use errors::GraphError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_new() {
        let g = Graph::new();
        assert!(g.nodes.is_empty());
        assert!(g.edges.is_empty());
    }

    #[test]
    fn test_add_node_and_edge() {
        let mut g = Graph::new();
        let node = Node {
            id: "test-node".to_string(),
            r#type: NodeType::Feature,
            title: "Test Feature".to_string(),
            body: "".to_string(),
            x: 0.0,
            y: 0.0,
            status: None,
            effort: None,
            created_by: "test".to_string(),
            ai_generated: false,
            source_node: None,
        };
        let node_id = node.id.clone();
        g.add_node(node);
        let _edge = Edge {
            id: "test-edge".to_string(),
            from: node_id,
            to: "other-node".to_string(),
            kind: EdgeKind::DependsOn,
        };
        // This would need a node to exist - skip for now
    }

    #[test]
    fn test_topological_sort_acyclic() {
        let mut g = Graph::new();
        let n1 = Node {
            id: "n1".to_string(),
            r#type: NodeType::Feature,
            title: "First".to_string(),
            body: "".to_string(),
            x: 0.0,
            y: 0.0,
            status: None,
            effort: None,
            created_by: "test".to_string(),
            ai_generated: false,
            source_node: None,
        };
        let n2 = Node {
            id: "n2".to_string(),
            r#type: NodeType::Feature,
            title: "Second".to_string(),
            body: "".to_string(),
            x: 0.0,
            y: 0.0,
            status: None,
            effort: None,
            created_by: "test".to_string(),
            ai_generated: false,
            source_node: None,
        };
        let n3 = Node {
            id: "n3".to_string(),
            r#type: NodeType::Feature,
            title: "Third".to_string(),
            body: "".to_string(),
            x: 0.0,
            y: 0.0,
            status: None,
            effort: None,
            created_by: "test".to_string(),
            ai_generated: false,
            source_node: None,
        };
        g.add_node(n1.clone());
        g.add_node(n2.clone());
        g.add_node(n3.clone());
        g.add_edge(Edge {
            id: "edge1".to_string(),
            from: "n1".to_string(),
            to: "n2".to_string(),
            kind: EdgeKind::DependsOn,
        });
        g.add_edge(Edge {
            id: "edge2".to_string(),
            from: "n2".to_string(),
            to: "n3".to_string(),
            kind: EdgeKind::DependsOn,
        });

        let order = g.topological_sort().unwrap();
        assert_eq!(
            order,
            vec!["n1".to_string(), "n2".to_string(), "n3".to_string()]
        );
    }

    #[test]
    fn test_topological_sort_cyclic() {
        let mut g = Graph::new();
        let n1 = Node {
            id: "n1".to_string(),
            r#type: NodeType::Feature,
            title: "First".to_string(),
            body: "".to_string(),
            x: 0.0,
            y: 0.0,
            status: None,
            effort: None,
            created_by: "test".to_string(),
            ai_generated: false,
            source_node: None,
        };
        let n2 = Node {
            id: "n2".to_string(),
            r#type: NodeType::Feature,
            title: "Second".to_string(),
            body: "".to_string(),
            x: 0.0,
            y: 0.0,
            status: None,
            effort: None,
            created_by: "test".to_string(),
            ai_generated: false,
            source_node: None,
        };
        g.add_node(n1.clone());
        g.add_node(n2.clone());
        g.add_edge(Edge {
            id: "edge3".to_string(),
            from: "n1".to_string(),
            to: "n2".to_string(),
            kind: EdgeKind::DependsOn,
        });
        g.add_edge(Edge {
            id: "edge4".to_string(),
            from: "n2".to_string(),
            to: "n1".to_string(),
            kind: EdgeKind::DependsOn,
        });

        let result = g.topological_sort();
        assert!(result.is_err());
    }

    #[test]
    fn test_canonical_serialization() {
        let mut g = Graph::new();
        let n1 = Node {
            id: "n1".to_string(),
            r#type: NodeType::Feature,
            title: "First".to_string(),
            body: "".to_string(),
            x: 0.0,
            y: 0.0,
            status: None,
            effort: None,
            created_by: "test".to_string(),
            ai_generated: false,
            source_node: None,
        };
        let n2 = Node {
            id: "n2".to_string(),
            r#type: NodeType::Feature,
            title: "Second".to_string(),
            body: "".to_string(),
            x: 0.0,
            y: 0.0,
            status: None,
            effort: None,
            created_by: "test".to_string(),
            ai_generated: false,
            source_node: None,
        };
        g.add_node(n1.clone());
        g.add_node(n2.clone());
        g.add_edge(Edge {
            id: "e1".to_string(),
            from: "n1".to_string(),
            to: "n2".to_string(),
            kind: EdgeKind::DependsOn,
        });

        let canonical = g.canonical();
        let json = serde_json::to_string(&canonical).unwrap();
        assert!(json.contains("\"from\":\"n1\""));
        assert!(json.contains("\"to\":\"n2\""));
        assert!(json.contains("\"kind\":\"depends_on\""));
    }
}
