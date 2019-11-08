use nom::bytes::complete::{is_a, tag, take_until};
use nom::IResult;
use petgraph::Graph;

struct Keyword {
    keyword: String,
    equivalent: String,
}

fn main() {
    let file_steam_msg: &'static str =
        include_str!("../../assets/SteamKit/Resources/SteamLanguage/steammsg.steamd");

    let mut graph = Graph::<String, &str>::new();
    let mut next_block = file_steam_msg.as_ref();

    while let Some(value) = extract_class_code(next_block) {
        let current_class_identifier = String::from_utf8(Vec::from(value.2)).unwrap();
        let member = extract_attr_lines(value.0).unwrap();
        let one_member = clear_lines_tab(member.0).unwrap();
        graph.add_node(current_class_identifier);
        next_block = value.1;
        println!("{:?}", String::from_utf8(one_member.0.to_vec()).unwrap());
    }
    println!("{:#?}", graph);
}

const CLASS: &[u8] = br#"class "#;
const CLASS_EOF: &[u8] = br#"};"#;

type ResultSlice<'a> = IResult<&'a [u8], &'a [u8]>;
type U8_2Tuple<'a> = (&'a [u8], &'a [u8]);
type U8_3Tuple<'a> = (&'a [u8], &'a [u8], &'a [u8]);

fn take_until_class(stream: &[u8]) -> ResultSlice {
    take_until(CLASS)(&stream)
}

fn take_until_class_eof(stream: &[u8]) -> ResultSlice {
    take_until(CLASS_EOF)(&stream)
}

fn take_until_open_bracket(stream: &[u8]) -> ResultSlice {
    take_until("{")(&stream)
}

fn take_until_ident(stream: &[u8]) -> ResultSlice {
    take_until("uint")(&stream)
}

fn take_until_lessthan(stream: &[u8]) -> ResultSlice {
    take_until("<")(&stream)
}

/// takes a class ident and returns as a node
fn class_as_node() {}

/// Returns class code, along with class name
fn extract_class_code(stream: &[u8]) -> Option<U8_3Tuple> {
    let parser = nom::sequence::preceded(
        // Ditch anything before the preamble
        take_until_class,
        nom::sequence::preceded(tag(CLASS), take_until_class_eof),
    );

    // swap positions so index 1 is the rest
    parser(stream).ok().map(|c| {
        let parsed_classname = take_until_lessthan(c.1).unwrap();
        (c.1, c.0, parsed_classname.1)
    })
}

fn extract_attr_lines(stream: &[u8]) -> Option<U8_2Tuple> {
    let preamble_parser = nom::sequence::preceded(take_until_open_bracket, tag("{"));
    preamble_parser(stream).ok()
}

/// Returns None if there are no more available members for extraction
fn clear_lines_tab(stream: &[u8]) -> ResultSlice {
    is_a("\r\n\t")(stream)
}
