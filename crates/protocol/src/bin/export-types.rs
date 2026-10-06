use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let target =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/protocol/src/index.ts");
    let source = liar_protocol::public_types();
    if env::args().any(|arg| arg == "--check") {
        if fs::read_to_string(&target)? != source {
            return Err("Generated protocol is stale; run pnpm types:generate".into());
        }
    } else {
        fs::create_dir_all(target.parent().ok_or("Invalid output path")?)?;
        fs::write(target, source)?;
    }
    Ok(())
}
