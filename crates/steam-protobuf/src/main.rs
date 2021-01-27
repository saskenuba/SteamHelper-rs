use std::{ffi::OsString, fs};

use glob::glob;

macro_rules! generate_protos_for {
    ($folder_name:literal) => {{
        let mut entries: Vec<OsString> = Vec::new();
        let mut filenames: Vec<OsString> = Vec::new();

        for entry in glob(concat!(
            "crates/steam-protobuf/assets/Protobufs/",
            $folder_name,
            "/*"
        ))
        .unwrap()
        {
            let entry = entry.unwrap();
            entries.insert(0, entry.clone().into());
            filenames.insert(0, entry.file_name().unwrap().into());
        }

        println!("Entries: {:#?}", entries);

        let entries_as_slice: Vec<&str> = entries.iter().map(|c| c.to_str().unwrap()).collect();
        let filenames_as_slice: Vec<&str> = filenames.iter().map(|c| c.to_str().unwrap()).collect();

        // generate mod file with exports for each proto file
        let new_filenames: Vec<String> = filenames_as_slice
            .into_iter()
            .map(|x| x.replace(".proto", ""))
            .map(|x| x.replace(".", "_"))
            .map(|x| "pub mod ".to_owned() + &x)
            .map(|x| x + ";\n")
            .collect();

        println!("New filenames: {:#?}", new_filenames);

        let modfile_path = concat!("crates/steam-protobuf/src/", $folder_name, "/mod.rs");
        fs::File::create(modfile_path).unwrap();
        fs::write(modfile_path, new_filenames.join("")).unwrap();

        protoc_rust::Codegen::new()
            .out_dir(&concat!("crates/steam-protobuf/src/", $folder_name))
            .inputs(entries_as_slice)
            .includes(&[
                concat!("crates/steam-protobuf/assets/Protobufs/", $folder_name),
                "crates/steam-protobuf/assets/Protobufs/google/protobuf",
                "crates/steam-protobuf/assets/Protobufs/steam",
            ])
            .run()
            .expect("protoc");
    }};
}

/// we also need to generate a mod file inside the chosen folder, with pub mod of each module
/// generated
fn main() {
    generate_protos_for!("google");
    generate_protos_for!("steam");
}
