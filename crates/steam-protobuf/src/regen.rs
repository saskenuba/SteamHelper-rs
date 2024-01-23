use std::path::PathBuf;

use protobuf_codegen::Customize;

pub(crate) fn generate() {
    let mut entries: Vec<PathBuf> = Vec::new();

    let file_iterator = glob::glob("**/steam/*.proto").unwrap().filter_map(|path| path.ok());
    for entry in file_iterator {
        eprintln!("{:?}", entry);
        entries.push(entry);
    }
    eprintln!("{:?}", entries);

    let protoc_cfg = Customize::default()
        .generate_accessors(true)
        .inside_protobuf(true)
        .tokio_bytes(true);

    // Use this in build.rs
    protobuf_codegen::Codegen::new()
        .customize(protoc_cfg)
        .protoc()
        .protoc_path(&protoc_bin_vendored::protoc_bin_path().unwrap())
        .capture_stderr()
        .inputs(entries)
        .includes(&["assets/protobufs/steam", "assets/protobufs/google"])
        .out_dir("src/protobufs")
        .run()
        .unwrap();
}
