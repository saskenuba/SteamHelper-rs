extern crate protoc_rust;
extern crate walkdir;

use std::ffi::OsString;

use protoc_rust::Customize;
use walkdir::WalkDir;

macro_rules! generate_protos_for {
    ($folder_name:literal) => {{
        let mut entries: Vec<OsString> = Vec::new();
        for entry in
            WalkDir::new(concat!("assets/Protobufs/", $folder_name, "/")).follow_links(false)
        {
            let entry = entry.unwrap();
            entries.insert(0, OsString::from(entry.path().as_os_str()));
        }
        let entries_as_slice: Vec<&str> = entries.iter().map(|c| c.to_str().unwrap()).collect();

        protoc_rust::run(protoc_rust::Args {
            out_dir: &concat!("assets/generated/", $folder_name),
            input: entries_as_slice.as_ref(),
            includes: &[concat!("assets/Protobufs/", $folder_name)],
            customize: Customize { ..Default::default() },
        })
        .expect("protoc")
    }};
}

fn main() {
    generate_protos_for!("steam")
}
