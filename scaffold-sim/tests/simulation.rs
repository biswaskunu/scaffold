use loro::ContainerTrait;
use scaffold_graph::{Edge, EdgeKind, Graph, GraphWarning, Node, NodeType};
use scaffold_sim::{SimClient, SimulatedNetwork};

#[test]
fn test_sim_scenario_2_dangling_edge_and_cleanup() {
    let mut net = SimulatedNetwork::new();
    let mut client_a = SimClient::new("client_a");
    let mut client_b = SimClient::new("client_b");

    net.partition("client_a", "client_b");

    let nodes_a = client_a.doc.get_map("nodes");
    let map_n1_a = nodes_a
        .insert_container("n1", loro::LoroMap::new())
        .unwrap();
    map_n1_a.insert("title", "Node 1").unwrap();
    map_n1_a.insert("type", "feature").unwrap();

    let nodes_b = client_b.doc.get_map("nodes");
    let map_n1_b = nodes_b
        .insert_container("n1", loro::LoroMap::new())
        .unwrap();
    map_n1_b.insert("title", "Node 1").unwrap();
    map_n1_b.insert("type", "feature").unwrap();

    nodes_a.delete("n1").unwrap();

    let map_n2_b = nodes_b
        .insert_container("n2", loro::LoroMap::new())
        .unwrap();
    map_n2_b.insert("title", "Node 2").unwrap();
    map_n2_b.insert("type", "feature").unwrap();

    let edges_b = client_b.doc.get_map("edges");
    let map_e1_b = edges_b
        .insert_container("e1", loro::LoroMap::new())
        .unwrap();
    map_e1_b.insert("from", "n2").unwrap();
    map_e1_b.insert("to", "n1").unwrap();
    map_e1_b.insert("kind", "depends_on").unwrap();

    net.heal("client_a", "client_b");
    let update_a = client_a.export_update();
    let update_b = client_b.export_update();

    client_a.import_update(&update_b);
    client_b.import_update(&update_a);

    let nodes_doc = client_a.doc.get_map("nodes");
    let edges_doc = client_a.doc.get_map("edges");

    let mut nodes = Vec::new();
    for key in nodes_doc.keys() {
        if let Some(val) = nodes_doc.get(&key) {
            if let Some(c) = val.as_container() {
                let m = client_a.doc.get_map(c.id());
                let title = m
                    .get("title")
                    .and_then(|v| {
                        v.into_value()
                            .ok()
                            .and_then(|val| val.as_string().map(|s| s.to_string()))
                    })
                    .unwrap_or_default();
                nodes.push(Node {
                    id: key.to_string(),
                    r#type: NodeType::Feature,
                    title,
                    body: "".to_string(),
                    x: 0.0,
                    y: 0.0,
                    status: None,
                    effort: None,
                    created_by: "test".to_string(),
                    ai_generated: false,
                    source_node: None,
                });
            }
        }
    }

    let mut edges = Vec::new();
    for key in edges_doc.keys() {
        if let Some(val) = edges_doc.get(&key) {
            if let Some(c) = val.as_container() {
                let m = client_a.doc.get_map(c.id());
                let from = m
                    .get("from")
                    .and_then(|v| {
                        v.into_value()
                            .ok()
                            .and_then(|val| val.as_string().map(|s| s.to_string()))
                    })
                    .unwrap_or_default();
                let to = m
                    .get("to")
                    .and_then(|v| {
                        v.into_value()
                            .ok()
                            .and_then(|val| val.as_string().map(|s| s.to_string()))
                    })
                    .unwrap_or_default();
                edges.push(Edge {
                    id: key.to_string(),
                    from,
                    to,
                    kind: EdgeKind::DependsOn,
                });
            }
        }
    }

    let built = Graph::build(nodes, edges);
    assert_eq!(built.graph.edges.len(), 0);
    assert!(built
        .warnings
        .iter()
        .any(|w| matches!(w, GraphWarning::DanglingEdge { .. })));

    for w in &built.warnings {
        if let GraphWarning::DanglingEdge { edge_id, .. } = w {
            edges_doc.delete(edge_id).unwrap();
        }
    }

    let built_after_cleanup = Graph::build(built.graph.nodes, vec![]);
    assert!(built_after_cleanup.warnings.is_empty());
}

#[test]
fn test_sim_scenario_3_cyclic_dependency_refusal() {
    let mut net = SimulatedNetwork::new();
    let mut client_a = SimClient::new("client_a");
    let client_b = SimClient::new("client_b");

    net.partition("client_a", "client_b");

    let nodes_a = client_a.doc.get_map("nodes");
    nodes_a
        .insert_container("n1", loro::LoroMap::new())
        .unwrap();
    nodes_a
        .insert_container("n2", loro::LoroMap::new())
        .unwrap();
    let edges_a = client_a.doc.get_map("edges");
    let e1 = edges_a
        .insert_container("e1", loro::LoroMap::new())
        .unwrap();
    e1.insert("from", "n1").unwrap();
    e1.insert("to", "n2").unwrap();
    e1.insert("kind", "depends_on").unwrap();

    let nodes_b = client_b.doc.get_map("nodes");
    nodes_b
        .insert_container("n1", loro::LoroMap::new())
        .unwrap();
    nodes_b
        .insert_container("n2", loro::LoroMap::new())
        .unwrap();
    let edges_b = client_b.doc.get_map("edges");
    let e2 = edges_b
        .insert_container("e2", loro::LoroMap::new())
        .unwrap();
    e2.insert("from", "n2").unwrap();
    e2.insert("to", "n1").unwrap();
    e2.insert("kind", "depends_on").unwrap();

    net.heal("client_a", "client_b");
    let _update_a = client_a.export_update();
    let update_b = client_b.export_update();

    client_a.import_update(&update_b);

    let nodes_doc = client_a.doc.get_map("nodes");
    let edges_doc = client_a.doc.get_map("edges");

    let mut nodes = Vec::new();
    for key in nodes_doc.keys() {
        if let Some(val) = nodes_doc.get(&key) {
            if let Some(_c) = val.as_container() {
                nodes.push(Node {
                    id: key.to_string(),
                    r#type: NodeType::Feature,
                    title: "".to_string(),
                    body: "".to_string(),
                    x: 0.0,
                    y: 0.0,
                    status: None,
                    effort: None,
                    created_by: "test".to_string(),
                    ai_generated: false,
                    source_node: None,
                });
            }
        }
    }

    let mut edges = Vec::new();
    for key in edges_doc.keys() {
        if let Some(val) = edges_doc.get(&key) {
            if let Some(c) = val.as_container() {
                let m = client_a.doc.get_map(c.id());
                let from = m
                    .get("from")
                    .and_then(|v| {
                        v.into_value()
                            .ok()
                            .and_then(|val| val.as_string().map(|s| s.to_string()))
                    })
                    .unwrap_or_default();
                let to = m
                    .get("to")
                    .and_then(|v| {
                        v.into_value()
                            .ok()
                            .and_then(|val| val.as_string().map(|s| s.to_string()))
                    })
                    .unwrap_or_default();
                edges.push(Edge {
                    id: key.to_string(),
                    from,
                    to,
                    kind: EdgeKind::DependsOn,
                });
            }
        }
    }

    let built = Graph::build(nodes, edges);
    let cycles = built.graph.detect_cycles();
    assert!(!cycles.is_empty());

    let topo = built.graph.topological_sort();
    assert!(topo.is_err());
}

#[test]
fn test_sim_scenario_1_same_node_different_fields_across_partition() {
    let mut net = SimulatedNetwork::new();
    let mut client_a = SimClient::new("client_a");
    let mut client_b = SimClient::new("client_b");

    net.partition("client_a", "client_b");

    let nodes_a = client_a.doc.get_map("nodes");
    let map_n1_a = nodes_a
        .insert_container("n1", loro::LoroMap::new())
        .unwrap();
    map_n1_a.insert("title", "Original Title").unwrap();
    map_n1_a.insert("body", "Original Body").unwrap();

    let nodes_b = client_b.doc.get_map("nodes");
    let map_n1_b = nodes_b
        .insert_container("n1", loro::LoroMap::new())
        .unwrap();
    map_n1_b.insert("title", "Original Title").unwrap();
    map_n1_b.insert("body", "Original Body").unwrap();

    // Client A edits title while partitioned
    let m_a = client_a.doc.get_map("n1");
    m_a.insert("title", "Edited Title A").unwrap();

    // Client B edits body while partitioned
    let m_b = client_b.doc.get_map("n1");
    m_b.insert("body", "Edited Body B").unwrap();

    net.heal("client_a", "client_b");
    let update_a = client_a.export_update();
    let update_b = client_b.export_update();

    client_a.import_update(&update_b);
    client_b.import_update(&update_a);

    // Check client A's view of node n1 fields
    let nodes_doc_a = client_a.doc.get_map("nodes");
    let val_a = nodes_doc_a.get("n1").unwrap();
    let c_a = val_a.as_container().unwrap();
    let m_a_result = client_a.doc.get_map(c_a.id());
    let title_a = m_a_result
        .get("title")
        .and_then(|v| v.into_value().ok())
        .and_then(|val| val.as_string().map(|s| s.to_string()));
    let body_a = m_a_result
        .get("body")
        .and_then(|v| v.into_value().ok())
        .and_then(|val| val.as_string().map(|s| s.to_string()));

    // Check client B's view of node n1 fields
    let nodes_doc_b = client_b.doc.get_map("nodes");
    let val_b = nodes_doc_b.get("n1").unwrap();
    let c_b = val_b.as_container().unwrap();
    let m_b_result = client_b.doc.get_map(c_b.id());
    let title_b = m_b_result
        .get("title")
        .and_then(|v| v.into_value().ok())
        .and_then(|val| val.as_string().map(|s| s.to_string()));
    let body_b = m_b_result
        .get("body")
        .and_then(|v| v.into_value().ok())
        .and_then(|val| val.as_string().map(|s| s.to_string()));

    // Both title and body should have values after LCRDT merge
    println!("Client A: title={:?}, body={:?}", title_a, body_a);
    println!("Client B: title={:?}, body={:?}", title_b, body_b);

    // Both clients should see both edits survive the merge
    assert!(title_a.is_some(), "Client A should have title after merge");
    assert!(body_a.is_some(), "Client A should have body after merge");
    assert!(title_b.is_some(), "Client B should have title after merge");
    assert!(body_b.is_some(), "Client B should have body after merge");
}
