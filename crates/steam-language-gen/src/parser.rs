use inflector::cases::snakecase::to_snake_case;
use nom::{FindSubstring, IResult};
use nom::bytes::complete::{is_a, tag, take_until};
use nom::character::complete::not_line_ending;
use petgraph::Graph;
use petgraph::graph::NodeIndex;
use regex::bytes::Regex;

use crate::{Element, Token};

const CLASS_KEYWORD: &[u8] = br#"class "#;
const ENUM_KEYWORD: &[u8] = br#"enum "#;
const BLOCK_START: &[u8] = br#"{"#;
const BLOCK_EOF: &[u8] = br#"};"#;

type ResultSlice<'a> = IResult<&'a [u8], &'a [u8]>;
type U82tuple<'a> = (&'a [u8], &'a [u8]);
type U83tuple<'a> = (&'a [u8], &'a [u8], &'a [u8]);
// type U83tuple<'a> = (&'a [u8], &'a [u8], &'a [u8]);
type U84tuple<'a> = (&'a [u8], &'a [u8], &'a [u8], &'a str);

pub fn parse_enums_to_tree(file: &'static str) -> (Graph<Token, Element>, NodeIndex<u32>) {
    let mut graph = Graph::<Token, Element>::new();
    let entry = graph.add_node(Token { value: "entry".to_string(), default: None });
    let mut next_block = file.as_ref();

    while let Some(enum_code_block) = extract_main_block(ENUM_KEYWORD, next_block) {
        let current_identifier = String::from_utf8(Vec::from(enum_code_block.2)).unwrap();
        let current_type = String::from_utf8(Vec::from(enum_code_block.3)).unwrap();

        // head insertion
        let block_node = graph.add_node(Token { value: current_identifier, default: Some("enum") });
        graph.add_edge(entry, block_node, Element::Head);

        // type insertion
        let type_node = graph.add_node(Token { value: current_type, default: None });
        graph.add_edge(block_node, type_node, Element::Type);

        let enum_attr_block = extract_attr_lines(enum_code_block.0).unwrap();

        let members: Vec<String> = extract_members_exhaustive(
            enum_attr_block.0,
            extract_member_enum)
            .iter()
            .rev()
            .map(|c| String::from_utf8(Vec::from(*c)).unwrap())
            .collect();

        members.iter()

            // ignore blank members, and discriminants added arbitrarily
            // https://github.com/SteamRE/SteamKit/issues/793#issuecomment-557824772
            .filter(|c| c != &"")
            .filter(|c| !c.starts_with("First") && !c.starts_with("Max"))
            .for_each(|c| {

                // member insertion
                let member_node = graph.add_node(Token { value: c.to_string(), default: None });
                graph.add_edge(type_node, member_node, Element::Member);
            });
        next_block = enum_code_block.1;
    }
    (graph, entry)
}

pub fn parse_classes_to_tree(file: &'static str) -> (Graph<Token, Element>, NodeIndex<u32>) {
    let mut graph = Graph::<Token, Element>::new();
    let entry = graph.add_node(Token { value: "entry".to_string(), default: None });
    let mut next_block = file.as_ref();

    while let Some(value) = extract_main_block(CLASS_KEYWORD, next_block) {
        // this holds the Msg Name (Ident)
        let current_identifier = String::from_utf8(Vec::from(value.2)).unwrap();
        // this is holding the EMSg type of our Msg
        let current_type = String::from_utf8(Vec::from(value.3)).unwrap();

        // node insertion
        let block_node = graph.add_node(Token { value: current_identifier, default: Some("struct") });
        graph.add_edge(entry, block_node, Element::Head);

        // type insertion
        let type_node = graph.add_node(Token { value: current_type, default: None });
        graph.add_edge(block_node, type_node, Element::Type);

        let member = extract_attr_lines(value.0).unwrap();

        let members: Vec<String> = extract_members_exhaustive(
            member.0,
            extract_member_struct)
            .iter()
            .rev()
            .map(|c| String::from_utf8(Vec::from(*c)).unwrap())
            .collect();

        let parsed_stmts = parse_stmts(members);
//        dbg!(&parsed_stmts);
        for stmt in parsed_stmts {
            stmt.iter().rev().for_each(|c| {
                // member insertion
                let member_node = graph.add_node(Token { value: c.to_string(), default: None });
                graph.add_edge(type_node, member_node, Element::Member);
            })
        }

        next_block = value.1;
    }
    (graph, entry)
}

fn take_until_open_bracket(stream: &[u8]) -> ResultSlice { take_until(BLOCK_START)(&stream) }

fn take_until_block_eof(stream: &[u8]) -> ResultSlice { take_until(BLOCK_EOF)(&stream) }

fn take_tabs_newlines(stream: &[u8]) -> ResultSlice { not_line_ending(stream) }

fn preamble_parser<'a>(stream: &'a [u8], keyword: &'static [u8]) -> Option<U82tuple<'a>> {
    nom::sequence::preceded(
        // Ditch anything before the preamble
        take_until(keyword),
        nom::sequence::preceded(tag(keyword), take_until_block_eof),
    )(stream).ok()
}

/// Returns a tuple with:
/// (class_code, rest, class_name, class_type)
fn extract_main_block<'a>(keyword: &'static [u8], stream: &'a [u8]) -> Option<U84tuple<'a>> {
    let parser = preamble_parser(stream, keyword);
    parser.map(|c| {
        let (_, enum_declaration_line) = take_tabs_newlines(c.1).unwrap();
        let (enum_filtered_name, enum_type) = filter_name_type(enum_declaration_line);
        (c.1, c.0, enum_filtered_name, enum_type)
    })
}

/// Returns a `tuple` with type and name of a head from its declaration line
fn filter_name_type(line_stream: &[u8]) -> (&[u8], &str) {
    // the standard type for all enums is i32
    let mut found_type = "i32";
    let mut index = line_stream.len();

    // check for type between greater and less than OR flags
    let re = Regex::new(r"<(.+)>| (flags)").unwrap();
    if let Some(capture) = re.captures(line_stream) {

        // if we cant match the first, match the second
        let captured_type = capture.get(1).unwrap_or_else(|| capture.get(2).unwrap());
        found_type = match_type(std::str::from_utf8(captured_type.as_bytes()).unwrap());
        index = captured_type.start() - 1;
    }
    (&line_stream[..index], found_type)
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
/// Member could be an attribute of a struct, or an enum
fn extract_member_struct(stream: &[u8]) -> Option<U82tuple> {
    nom::sequence::preceded(clear_lines_tab, take_until(";"))(stream)
        .ok()
        .map(|c| {
            //removes ; on the 1st byte
            let x = &c.0[1..];
            (c.1, x)
        })
}

/// Discard newlines, tabs and ';' eof
/// Returns every member of enum.
/// In the case of an enum has some keywords to indicate no more in use, such as obsolete, removed
/// a empty slice is returned
fn extract_member_enum(stream: &[u8]) -> Option<U82tuple> {
    let mut flag: bool = false;
    nom::sequence::preceded(clear_lines_tab, take_until("\r"))(stream)
        .ok()
        .map(|c| {

            // removes ; from last pass
            let rest = &c.0[1..];
            // check for commentaries
            if c.1.find_substring("removed").is_some() || c.1.find_substring("obsolete").is_some() { flag = true };

            let match_len = c.1.len();

            if flag {
                let slice: &[u8] = b"";
                return (slice, rest);
            }
            (&c.1[..match_len - 1], rest)
        })
}


/// Extract attributes inside a class and returns as Vec of bytes
/// Accepts a custom extraction function
fn extract_members_exhaustive<F>(mut attributes_code: &[u8], member_extraction_function: F) -> Vec<&[u8]>
    where F: Fn(&[u8]) -> Option<U82tuple> {
    let mut tokens = Vec::new();
    while let Some(value) = member_extraction_function(attributes_code) {
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
        let stmt_split_by_assign: Vec<&str> = stmt.split(" = ").collect();
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
    use crate::parser::{ENUM_KEYWORD, CLASS_KEYWORD};

    use super::{
        array_extract_size, extract_attr_lines, extract_main_block, extract_member_enum,
        extract_member_struct, extract_members_exhaustive, is_array, parse_stmts, split_words_to_vec,
    };

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

    fn gen_class_block() -> &'static str {
        "\r\n\r\nclass MsgClientNewLoginKeyAccepted<EMsg::ClientNewLoginKeyAccepted>\r\n{\r\n\tuint uniqueID;\r\n};\r\n\r\nclass"
    }

    fn gen_enum() -> &'static str {
        "\r\n\r\nenum EChatEntryType flags\r\n{\r\n\tInvalid = 0;\r\n\r\n\tChatMsg = 1;\r\n\tTyping = 2;\
        \r\n\tInviteGame = 3;\r\n\tEmote = 4; removed \"No longer supported by clients\"\r\n\tLobby\
        GameStart = 5; removed \"Listen for LobbyGameCreated_t callback instead\"\r\n\tLeftConversa\
        tion = 6;\r\n\tEntered = 7;\r\n\tWasKicked = 8;\r\n\tWasBanned = 9;\r\n\tDisconnected = 10;\
        \r\n\tHistoricalChat = 11;\r\n\tReserved1 = 12;\r\n\tReserved2 = 13;\r\n\tLinkBlocked = 14;\
        \r\n};"
    }


    fn vec_string_to_str<'a>(vec: &(&[&str; 2], &'a Vec<String>)) -> Vec<&'a str> {
        vec.1.iter().map(|c| c.as_str()).collect()
    }

    fn bytes_to_str(vec: Vec<&[u8]>) -> Vec<String> {
        vec.iter().map(|c| {
            String::from_utf8((*c).to_vec()).unwrap()
        }).collect()
    }

    #[test]
    fn test_extract_class() {
        let raw_class = gen_class_block();
        let (_whole_class_block, _rest, parsed_classname, emsg_type) = extract_main_block(CLASS_KEYWORD, raw_class.as_ref()).unwrap();

        assert_eq!(
            String::from_utf8(parsed_classname.to_vec()).unwrap(),
            "MsgClientNewLoginKeyAccepted".to_string()
        );
        assert_eq!(
            emsg_type,
            "EMsg::ClientNewLoginKeyAccepted"
        );
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
        let members = extract_members_exhaustive(code.as_ref(), extract_member_struct);
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

    #[test]
    fn test_enum() {
        let correct_output = vec!["Invalid = 0", "ChatMsg = 1", "Typing = 2", "InviteGame = 3",
                                  "", "", "LeftConversation = 6"];
        let stmt = gen_enum().as_ref();
        let enum_code_block = extract_main_block(ENUM_KEYWORD, stmt).unwrap();
        let enum_attr_block = extract_attr_lines(enum_code_block.0).unwrap();
        let parsed_attr_block = extract_members_exhaustive(enum_attr_block.0, extract_member_enum);
        let parsed_attr_block_str = bytes_to_str(parsed_attr_block);

        assert_eq!(correct_output[..7], parsed_attr_block_str[..7]);
    }
}