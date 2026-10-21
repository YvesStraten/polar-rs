use xtask::emitter;

use std::path::Path;

fn main() {
    let cmd = std::env::args().nth(1);
    match cmd.as_deref() {
        Some("generate") => {
            emitter::generate(
                Path::new("spec/openapi.json"),
                Path::new("crates/polar-api/src/types.rs"),
                Path::new("crates/polar-api/src/operations.rs"),
            )
            .expect("generate");
            println!(
                "generated crates/polar-api/src/types.rs and crates/polar-api/src/operations.rs"
            );
        }
        Some("update-spec") => {
            let url = "https://polar.sh/docs/openapi.json";
            let out = std::process::Command::new("curl")
                .args(["-sS", "-o", "spec/openapi.json", url])
                .status()
                .expect("run curl");
            assert!(out.success(), "curl failed");
            println!("wrote spec/openapi.json");
        }
        _ => {
            eprintln!("usage: xtask <generate|update-spec>");
            std::process::exit(2);
        }
    }
}
