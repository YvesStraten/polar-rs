#[test]
fn generated_types_are_up_to_date() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec_path = manifest.join("../spec/openapi.json");
    let committed_path = manifest.join("../crates/polar-api/src/types.rs");

    let spec = std::fs::read_to_string(&spec_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", spec_path.display()));
    let spec: serde_json::Value = serde_json::from_str(&spec).unwrap();
    let ir = xtask::emitter::parse::parse(&spec);
    let fresh = xtask::emitter::emit::emit(&ir);
    let committed = std::fs::read_to_string(&committed_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", committed_path.display()));

    assert_eq!(
        fresh, committed,
        "crates/polar-api/src/types.rs is stale; run `cargo run -p xtask -- generate`"
    );
}

#[test]
fn generated_operations_are_up_to_date() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec_path = manifest.join("../spec/openapi.json");
    let committed_path = manifest.join("../crates/polar-api/src/operations.rs");

    let spec = std::fs::read_to_string(&spec_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", spec_path.display()));
    let spec: serde_json::Value = serde_json::from_str(&spec).unwrap();
    let ir = xtask::emitter::parse::parse(&spec);
    let fresh = xtask::emitter::emit::emit_operations(&ir);
    let committed = std::fs::read_to_string(&committed_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", committed_path.display()));

    assert_eq!(
        fresh, committed,
        "crates/polar-api/src/operations.rs is stale; run `cargo run -p xtask -- generate`"
    );
}
