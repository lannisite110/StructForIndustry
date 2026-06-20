use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let schema_dir = manifest_dir.join("../../schema");

    let entries: Vec<PathBuf> = std::fs::read_dir(&schema_dir)
        .expect("read schema dir")
        .map(|entry| entry.expect("dir entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "capnp"))
        .collect();

    let mut command = capnpc::CompilerCommand::new();
    command.src_prefix(&schema_dir).import_path(&schema_dir);
    for path in &entries {
        command.file(path);
    }
    command.run().expect("capnp compile failed");
}
