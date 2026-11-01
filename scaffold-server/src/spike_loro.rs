use loro::{ExportMode, LoroDoc, VersionVector};

pub fn test_loro_round_trip() {
    // --- Doc A: create and modify ---
    let doc_a = LoroDoc::new();
    doc_a.get_map("root").insert("key", "value").unwrap();
    let updates_a = doc_a
        .export(ExportMode::updates(&VersionVector::default()))
        .unwrap();

    // --- Doc B: start fresh, import A's updates ---
    let doc_b = LoroDoc::new();
    doc_b.import(&updates_a).unwrap();

    // --- Doc C: create independently, import same updates ---
    let doc_c = LoroDoc::new();
    doc_c.import(&updates_a).unwrap();

    // --- Verify: B and C should have the same data ---
    let val_a = doc_a.get_map("root").get("key");
    let val_b = doc_b.get_map("root").get("key");
    let val_c = doc_c.get_map("root").get("key");

    assert_eq!(format!("{val_a:?}"), format!("{val_b:?}"));
    assert_eq!(format!("{val_a:?}"), format!("{val_c:?}"));
}
