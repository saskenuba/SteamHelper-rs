use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::Path;
use std::path::PathBuf;

use protobuf_codegen::Customize;

const OUT_DIR: &str = "src/protobufs";
const CSGO_DIR: &str = "assets/protobufs/csgo";
const CSGO_OUT_DIR: &str = "src/protobufs/csgo";
/// The CS2 protos a game-coordinator client needs; the protos they import are generated too.
const CSGO_ROOTS: &[&str] = &[
    "base_gcmessages.proto",
    "cstrike15_gcmessages.proto",
    "gcsdk_gcmessages.proto",
    "gcsystemmsgs.proto",
];
const CSGO_FEATURE: &str = "csgo";
const FEATURES_BEGIN: &str = "# @generated features begin";
const FEATURES_END: &str = "# @generated features end";

pub(crate) fn generate() {
    std::env::set_current_dir(env!("CARGO_MANIFEST_DIR")).unwrap();

    // Remove previously generated files so protos deleted upstream don't linger.
    for entry in std::fs::read_dir(OUT_DIR).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().map_or(false, |ext| ext == "rs") {
            std::fs::remove_file(path).unwrap();
        }
    }

    let mut entries: Vec<PathBuf> = glob::glob("assets/protobufs/steam/*.proto")
        .unwrap()
        .filter_map(|path| path.ok())
        .collect();
    entries.sort();
    eprintln!("{:?}", entries);

    codegen(&entries, "assets/protobufs/steam", OUT_DIR);

    // Each proto file becomes a module gated behind a feature of the same name,
    // which enables the features of the protos it imports.
    let mut modules: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in &entries {
        let mut imports: Vec<String> = proto_imports(entry)
            .iter()
            .map(|import| proto_to_module(Path::new(import)))
            .collect();
        imports.sort();
        imports.dedup();

        let module = proto_to_module(entry);
        assert_generated(OUT_DIR, &module);
        modules.insert(module, imports);
    }

    generate_csgo();

    write_mod_rs(&modules);
    write_cargo_features(&modules);
}

/// CS2 protos are package-less and redefine steam messages such as `CMsgProtoBufHeader`, so they
/// get their own codegen run and module tree, gated as a whole behind the `csgo` feature.
fn generate_csgo() {
    if Path::new(CSGO_OUT_DIR).exists() {
        std::fs::remove_dir_all(CSGO_OUT_DIR).unwrap();
    }
    std::fs::create_dir_all(CSGO_OUT_DIR).unwrap();

    // Walk the imports of the roots, so only what they need is generated.
    let mut pending: Vec<String> = CSGO_ROOTS.iter().map(|root| root.to_string()).collect();
    let mut protos: Vec<String> = Vec::new();
    while let Some(proto) = pending.pop() {
        if protos.contains(&proto) {
            continue;
        }
        pending.extend(proto_imports(&Path::new(CSGO_DIR).join(&proto)));
        protos.push(proto);
    }
    protos.sort();
    let entries: Vec<PathBuf> = protos.iter().map(|proto| Path::new(CSGO_DIR).join(proto)).collect();
    eprintln!("{:?}", entries);

    codegen(&entries, CSGO_DIR, CSGO_OUT_DIR);

    let mut out = String::from("// @generated\n");
    for entry in &entries {
        let module = proto_to_module(entry);
        assert_generated(CSGO_OUT_DIR, &module);
        write!(out, "\npub mod {module};\n").unwrap();
    }
    std::fs::write(Path::new(CSGO_OUT_DIR).join("mod.rs"), out).unwrap();
}

fn codegen(inputs: &[PathBuf], include: &str, out_dir: &str) {
    // `lite_runtime` skips descriptors and reflection, which make up most of the generated code.
    let protoc_cfg = Customize::default()
        .generate_accessors(true)
        .lite_runtime(true)
        .tokio_bytes(true);

    protobuf_codegen::Codegen::new()
        .customize(protoc_cfg)
        .protoc()
        .protoc_path(&protoc_bin_vendored::protoc_bin_path().unwrap())
        .capture_stderr()
        .inputs(inputs)
        .includes(&[include])
        .out_dir(out_dir)
        .run()
        .unwrap();
}

/// The protos a proto file imports, except `google/` ones, which the protobuf crate provides.
fn proto_imports(path: &Path) -> Vec<String> {
    let source = std::fs::read_to_string(path).unwrap();
    source
        .lines()
        .filter_map(|line| line.trim().strip_prefix("import \""))
        .filter_map(|line| line.split('"').next())
        .filter(|import| !import.starts_with("google/"))
        .map(str::to_string)
        .collect()
}

fn assert_generated(out_dir: &str, module: &str) {
    assert!(
        Path::new(out_dir).join(format!("{module}.rs")).exists(),
        "codegen did not produce module `{}`",
        module
    );
}

/// Mirrors how protobuf-codegen names the module generated from a proto file.
fn proto_to_module(path: &Path) -> String {
    let stem = path.file_stem().unwrap().to_str().unwrap();
    stem.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn write_mod_rs(modules: &BTreeMap<String, Vec<String>>) {
    let mut out = String::from("// @generated\n");
    for module in modules.keys() {
        write!(out, "\n#[cfg(feature = \"{module}\")]\npub mod {module};\n").unwrap();
    }
    write!(out, "\n#[cfg(feature = \"{CSGO_FEATURE}\")]\npub mod csgo;\n").unwrap();
    std::fs::write(Path::new(OUT_DIR).join("mod.rs"), out).unwrap();
}

fn write_cargo_features(modules: &BTreeMap<String, Vec<String>>) {
    let quoted = |names: &mut dyn Iterator<Item = &String>| {
        names.map(|name| format!("\"{name}\"")).collect::<Vec<_>>().join(", ")
    };

    let mut block = String::new();
    writeln!(block, "{FEATURES_BEGIN}").unwrap();
    writeln!(block, "full = [{}]", quoted(&mut modules.keys())).unwrap();
    // Not part of `full`: the CS2 protos are large and only wanted by CS2 clients.
    writeln!(block, "{CSGO_FEATURE} = []").unwrap();
    for (module, imports) in modules {
        writeln!(block, "{module} = [{}]", quoted(&mut imports.iter())).unwrap();
    }
    write!(block, "{FEATURES_END}").unwrap();

    let manifest = std::fs::read_to_string("Cargo.toml").unwrap();
    let start = manifest.find(FEATURES_BEGIN).expect("missing features begin marker in Cargo.toml");
    let end = manifest.find(FEATURES_END).expect("missing features end marker in Cargo.toml") + FEATURES_END.len();
    let manifest = format!("{}{}{}", &manifest[..start], block, &manifest[end..]);
    std::fs::write("Cargo.toml", manifest).unwrap();
}
