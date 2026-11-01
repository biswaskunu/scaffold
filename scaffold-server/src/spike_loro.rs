use loro::{ExportMode, LoroDoc, VersionVector};

#[test]
fn test_loro_round_trip() {
    // --- Doc A: create and modify ---
    let doc_a = LoroDoc::new();
    doc_a.set_peer_id(1).unwrap();
    let text_a = doc_a.get_text("text");
    text_a.insert(0, "Hello, Scaffold!").unwrap();
    doc_a.commit();

    let vv_empty = VersionVector::default();
    let updates_a = doc_a.export(ExportMode::updates(&vv_empty)).unwrap();

    // --- Doc B: start fresh, import A's updates ---
    let doc_b = LoroDoc::new();
    doc_b.import(&updates_a).unwrap();

    // --- Verify: B should have the text ---
    let text_b = doc_b.get_text("text");
    assert_eq!(text_b.to_string(), "Hello, Scaffold!");
}
