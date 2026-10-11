use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::Path;
use std::path::PathBuf;

use protobuf_codegen::Customize;

const OUT_DIR: &str = "src/protobufs";
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
        .inputs(&entries)
        .includes(&["assets/protobufs/steam"])
        .out_dir(OUT_DIR)
        .run()
        .unwrap();

    // Each proto file becomes a module gated behind a feature of the same name,
    // which enables the features of the protos it imports.
    let mut modules: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in &entries {
        let source = std::fs::read_to_string(entry).unwrap();
        let mut imports: Vec<String> = source
            .lines()
            .filter_map(|line| line.trim().strip_prefix("import \""))
            .filter_map(|line| line.split('"').next())
            .filter(|import| !import.starts_with("google/"))
            .map(|import| proto_to_module(Path::new(import)))
            .collect();
        imports.sort();
        imports.dedup();

        let module = proto_to_module(entry);
        assert!(
            Path::new(OUT_DIR).join(format!("{module}.rs")).exists(),
            "codegen did not produce module `{}`",
            module
        );
        modules.insert(module, imports);
    }

    write_mod_rs(&modules);
    write_cargo_features(&modules);
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
    std::fs::write(Path::new(OUT_DIR).join("mod.rs"), out).unwrap();
}

fn write_cargo_features(modules: &BTreeMap<String, Vec<String>>) {
    let quoted = |names: &mut dyn Iterator<Item = &String>| {
        names.map(|name| format!("\"{name}\"")).collect::<Vec<_>>().join(", ")
    };

    let mut block = String::new();
    writeln!(block, "{FEATURES_BEGIN}").unwrap();
    writeln!(block, "full = [{}]", quoted(&mut modules.keys())).unwrap();
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
