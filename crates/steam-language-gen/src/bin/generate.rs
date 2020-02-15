use steam_language_gen::generator::{append_to_file, generate_code, write_to_file};
use steam_language_gen::parser::{parse_classes_to_tree, parse_enums_to_tree};

fn main() {

    let path_enums_output: &'static str  = "crates/steam-language-gen/src/generated/";
    let path_steamsg_output: &'static str  = "crates/steam-language-gen/src/generated/messages.rs";

    let file_steam_msg: &'static str =
        include_str!("../../assets/SteamKit/Resources/SteamLanguage/steammsg.steamd");

    let file_eresult: &'static str =
        include_str!("../../assets/SteamKit/Resources/SteamLanguage/eresult.steamd");

    let file_emsg: &'static str =
        include_str!("../../assets/SteamKit/Resources/SteamLanguage/emsg.steamd");

    let file_steam_enums: &'static str =
        include_str!("../../assets/SteamKit/Resources/SteamLanguage/enums.steamd");

    let (class_graph, class_entry) = parse_classes_to_tree(file_steam_msg);
    let file_class = generate_code(class_graph, class_entry);
    write_to_file(&file_class, "messages.rs");

    let (enum_graph, enum_entry) = parse_enums_to_tree(file_steam_enums);
    let enum_class = generate_code(enum_graph, enum_entry);
    write_to_file(&enum_class, "enums.rs");

    let (enum_graph, enum_entry) = parse_enums_to_tree(file_eresult);
    let enum_class = generate_code(enum_graph, enum_entry);
    append_to_file(&enum_class, &(path_enums_output.to_owned() + "enums.rs"));

    let (enum_graph, enum_entry) = parse_enums_to_tree(file_emsg);
    let enum_class = generate_code(enum_graph, enum_entry);
    append_to_file(&enum_class, &(path_enums_output.to_owned() + "enums.rs"));
}


