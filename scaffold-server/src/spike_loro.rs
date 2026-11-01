use loro::{Document, Export, Update};

#[test]
fn test_loro_round_trip() {
    // --- Doc A: create and modify ---
    let mut doc_a = Document::new();
    doc_a.put("key", "value".into()).unwrap();
    let vv_a = doc_a.version_vector().clone();
    let updates_a: Export = doc_a.export_updates_since(&vv_a).unwrap();

    // --- Doc B: start fresh, import A's updates ---
    let mut doc_b = Document::new();
    doc_b.import_updates(&updates_a).unwrap();

    // --- Doc C: create independently, import same updates ---
    let mut doc_c = Document::new();
    doc_c.import_updates(&updates_a).unwrap();

    // --- Verify: B and C should have the same data ---
    let val_a = doc_a.get::<str>("key").map(|s| s.to_string()).unwrap_or_default();
    let val_b = doc_b.get::<str>("key").map(|s| s.to_string()).unwrap_or_default();
    let val_c = doc_c.get::<str>("key").map(|s| s.to_string()).unwrap_or_default();

    assert_eq!(val_a, val_b, "Doc B should have the same data as Doc A after import");
    assert_eq!(val_a, val_c, "Doc C should have the same data as Doc A after import");
    assert_eq!(val_b, val_c, "Docs B and C should converge after same import");
}