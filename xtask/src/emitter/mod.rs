use std::path::Path;

pub mod emit;
pub mod ir;
pub mod parse;

pub fn generate(spec_path: &Path, types_out: &Path, ops_out: &Path) -> std::io::Result<()> {
    let raw = std::fs::read_to_string(spec_path)?;
    let spec: serde_json::Value = serde_json::from_str(&raw).expect("spec is valid JSON");
    let ir = parse::parse(&spec);
    std::fs::write(types_out, emit::emit(&ir))?;
    std::fs::write(ops_out, emit::emit_operations(&ir))?;
    Ok(())
}
