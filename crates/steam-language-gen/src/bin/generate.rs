use inflector::cases::snakecase::to_snake_case;
use nom::bytes::complete::{is_a, tag, take_until};
use nom::IResult;
use petgraph::Graph;

use steam_language_gen::file::generate_file_from_tree;

fn main() {
    let file_steam_msg: &'static str =
        include_str!("../../assets/SteamKit/Resources/SteamLanguage/steammsg.steamd");

    let mut graph = Graph::<String, &str>::new();
    let entry = graph.add_node("entry".to_string());
    let mut next_block = file_steam_msg.as_ref();

    while let Some(value) = extract_class_code(next_block) {
        let current_class_identifier = String::from_utf8(Vec::from(value.2)).unwrap();

        // node insertion
        let class_node = graph.add_node(current_class_identifier);
        graph.add_edge(entry, class_node, "0");

        let member = extract_attr_lines(value.0).unwrap();

        let members: Vec<String> = extract_members_exhaustive(member.0)
            .iter()
            .map(|c| String::from_utf8(Vec::from(*c)).unwrap())
            .collect();

        let parsed_stmts = parse_stmts(members);
        for stmt in parsed_stmts {
            stmt.iter().for_each(|c| {
                let edge = graph.add_node(c.to_string());
                graph.add_edge(class_node, edge, "0");
            })
        }

        next_block = value.1;
    }
    let file = generate_file_from_tree(graph, entry);
    print!("{}", file);
}

const CLASS_KEYWORD: &[u8] = br#"class "#;
const ENUM_KEYWORD: &[u8] = br#"enum "#;
const BLOCK_EOF: &[u8] = br#"};"#;

type ResultSlice<'a> = IResult<&'a [u8], &'a [u8]>;
type U82tuple<'a> = (&'a [u8], &'a [u8]);
type U83tuple<'a> = (&'a [u8], &'a [u8], &'a [u8]);

fn take_until_class(stream: &[u8]) -> ResultSlice {
    take_until(CLASS_KEYWORD)(&stream)
}

fn take_until_enum(stream: &[u8]) -> ResultSlice {
    take_until(ENUM_KEYWORD)(&stream)
}

fn take_until_block_eof(stream: &[u8]) -> ResultSlice {
    take_until(BLOCK_EOF)(&stream)
}

fn take_until_open_bracket(stream: &[u8]) -> ResultSlice {
    take_until("{")(&stream)
}

fn take_until_lessthan(stream: &[u8]) -> ResultSlice { take_until("<")(&stream) }


/// Returns enum code, along with enum name
fn extract_enum_code(stream: &[u8]) -> Option<U83tuple> {
    unimplemented!()
}

/// Returns class code, along with class name
fn extract_class_code(stream: &[u8]) -> Option<U83tuple> {
    let parser = nom::sequence::preceded(
        // Ditch anything before the preamble
        take_until_class,
        nom::sequence::preceded(tag(CLASS_KEYWORD), take_until_block_eof),
    );

    // swap positions so index 1 is the rest
    parser(stream).ok().map(|c| {
        let parsed_classname = take_until_lessthan(c.1).unwrap();
        (c.1, c.0, parsed_classname.1)
    })
}

fn extract_attr_lines(stream: &[u8]) -> Option<U82tuple> {
    let preamble_parser = nom::sequence::preceded(take_until_open_bracket, tag("{"));
    preamble_parser(stream).ok()
}

/// Returns None if there are no more available members for extraction
fn clear_lines_tab(stream: &[u8]) -> ResultSlice {
    is_a("\r\n\t")(stream)
}

/// Discard newlines, tabs and ';' eof
fn extract_member(stream: &[u8]) -> Option<U82tuple> {
    nom::sequence::preceded(clear_lines_tab, take_until(";"))(stream).ok().map(|c| {
        //removes ; on the 1st byte
        let x = &c.0[1..];
        (c.1, x)
    })
}

/// Extract attributes inside a class and returns as Vec of bytes
fn extract_members_exhaustive(mut attributes_code: &[u8]) -> Vec<&[u8]> {
    let mut tokens = Vec::new();
    while let Some(value) = extract_member(attributes_code) {
        tokens.push(value.0);
        attributes_code = value.1;
    }
    tokens
}

fn split_words_to_vec(declaration: &str) -> Vec<&str> {
    declaration.split(' ').collect()
}

/// Returns matched types
fn match_type(mut slice: &str) -> &str {

    // simply discard const
    if slice.starts_with("const ") { slice = &slice[6..] }

    match slice {
        "ulong" => "u64",
        "long" => "i64",
        "uint" => "u32",
        "int" => "i32",
        "ushort" => "u16",
        "short" => "i16",
        "byte" => "u8",

        // we later may change how this is parsed
        // on the original steamkit parser each one of this serves as a marker to
        // getters and setters of different behavior
        "steamidmarshal ulong" => "u64",
        "boolmarshal byte" => "u8",
        "gameidmarshal ulong" => "u64",
        value => value
    }
}

/// Returns Vector that has each stmt(declarations non assignment) parsed into rust code
fn parse_stmts(stmt_vector: Vec<String>) -> Vec<Vec<String>> {
    let mut final_vector: Vec<Vec<String>> = Vec::new();
    for stmt in stmt_vector {

        // if only one token and assignment, we know it is an enum
        let stmt_split_by_assign: Vec<&str> = stmt.split(" =").collect();
        let stmt_tokens = split_words_to_vec(&stmt_split_by_assign[0]);

        let mut new_vec: Vec<String> = Vec::with_capacity(stmt_tokens.len());

        let mut token_ident = stmt_tokens[1];
        let mut token_type = stmt_tokens[0].to_string();

        // in the case of three tokens
        // we join the first two to match its type
        if stmt_tokens.len() > 2 {
            token_type.push(' ');
            token_type.push_str(stmt_tokens[1]);
            token_ident = stmt_tokens[2];
        }

        new_vec.push(to_snake_case(token_ident));

        if is_array(&token_type) {
            new_vec.push(format!("[u8; {}]", array_extract_size(&token_type)));
        } else {
            new_vec.push(match_type(token_type.as_ref()).to_string());
        }

        final_vector.push(new_vec);
    }
    final_vector
}

/// Extracts size from byte<%> where % is an integer
fn array_extract_size(slice: &str) -> String {
    slice.to_string().replacen(|c| !char::is_numeric(c), "", 10)
}

/// Checks if type is array - only possible type is byte array
fn is_array(string: &str) -> bool {
    string.find(|c: char| (c == '<') || (c == '>')).is_some()
}

#[cfg(test)]
mod tests {
    use super::{
        array_extract_size, extract_members_exhaustive, is_array, parse_stmts, split_words_to_vec,
    };

    fn gen_stmt_unknown_type() -> &'static str {
        "steamidmarshal ulong steamId"
    }

    fn gen_stmt_known_type() -> &'static str {
        "ulong steamId"
    }

    fn gen_members_code() -> &'static str {
        "\r\n\tulong giftId;\r\n\tbyte giftType;\r\n\tuint accountId;\r\n"
    }

    fn gen_members_vec() -> Vec<String> {
        vec!["ulong giftId".into(), "byte<10> giftType".into(), "uint accountId".into()]
    }

    fn gen_three_token_vec() -> Vec<String> {
        vec!["steamidmarshal ulong accountId".into(), "const uint ObfuscationMask".into(),
             "boolmarshal byte validated".into()]
    }

    fn gen_assignment_vec() -> Vec<String> {
        vec!["EUniverse universe = EUniverse::Invalid".into(), "const uint ObfuscationMask = 0xBAADF00D".into(),
             "uint protocolVersion = MsgChannelEncryptRequest::PROTOCOL_VERSION".into()]
    }

    fn vec_string_to_str<'a>(vec: &(&[&str; 2], &'a Vec<String>)) -> Vec<&'a str> {
        vec.1.iter().map(|c| c.as_str()).collect()
    }

    #[test]
    fn test_split_tokens() {
        let stmt = gen_stmt_known_type();
        let stmt_tokens = split_words_to_vec(stmt);
        assert_eq!(vec!["ulong", "steamId"], stmt_tokens);
    }

    #[test]
    fn test_extract_members_exhaustive() {
        let code = gen_members_code();
        let members = extract_members_exhaustive(code.as_ref());
        let stringify: Vec<String> =
            members.iter().map(|c| String::from_utf8(c.to_vec()).unwrap()).collect();
        assert_eq!(vec!["ulong giftId", "byte giftType", "uint accountId"], stringify)
    }

    #[test]
    fn test_parse_known_types() {
        let non_parsed_vec = gen_members_vec();
        let parsed_vec = parse_stmts(non_parsed_vec);
        let test_vec = [["gift_id", "u64"], ["gift_type", "[u8; 10]"], ["account_id", "u32"]];

        for vec in test_vec.iter().zip(parsed_vec.iter()) {
            let x: Vec<&str> = vec_string_to_str(&vec);
            assert_eq!(vec.0.to_vec(), x)
        }
    }

    #[test]
    fn test_parse_three_tokens() {
        let non_parsed_vec = gen_three_token_vec();
        let parsed_vec = parse_stmts(non_parsed_vec);
        let test_vec = [["account_id", "u64"], ["obfuscation_mask", "u32"], ["validated", "u8"]];

        for vec in test_vec.iter().zip(parsed_vec.iter()) {
            let x: Vec<&str> = vec_string_to_str(&vec);
            assert_eq!(vec.0.to_vec(), x)
        }
    }

    #[test]
    fn test_parse_assignment_tokens() {
        let non_parsed_vec = gen_assignment_vec();
        let parsed_vec = parse_stmts(non_parsed_vec);
        let test_vec = [
            ["universe", "EUniverse"], ["obfuscation_mask", "u32"], ["protocol_version", "u32"]];

        for vec in test_vec.iter().zip(parsed_vec.iter()) {
            let x: Vec<&str> = vec_string_to_str(&vec);
            assert_eq!(vec.0.to_vec(), x)
        }
    }

    #[test]
    fn test_array() {
        let array = "byte<10>";
        let not_array = "byte";

        assert_eq!(true, is_array(array));
        assert_eq!(false, is_array(not_array));
        assert_eq!(10, array_extract_size(array).parse::<u32>().unwrap());
    }
}
