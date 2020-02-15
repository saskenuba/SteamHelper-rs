//! This file is the opinionated rust bindings generator

use std::fs;
use std::io::Write;

use petgraph::prelude::*;
use petgraph::visit::EdgeFiltered;

use crate::{Element, Token};

#[derive(PartialEq, Eq)]
enum DetectedStructure {
    EnumType,
    StructType,
}

pub fn generate_code(graph: Graph<Token, Element>, entry: NodeIndex) -> String {
    let mut file = String::new();

    let ef_graph_head = EdgeFiltered::from_fn(&graph, |edge| {
        *edge.weight() == Element::Head
    });
    let mut dfs = Dfs::new(&ef_graph_head, entry);

    let standard_imports = "use serde::{Deserialize, Serialize};\
    \nuse serde_repr::{Deserialize_repr, Serialize_repr};\n\n";
    file.push_str(standard_imports);

    // sends the head node to the proper function
    while let Some(current_node) = dfs.next(&ef_graph_head) {
        if let Some(detected_block) = graph[current_node].get_default() {
            match detected_block {
                "enum" => {
                    if graph[current_node].get_value() == "entry" { continue; }
                    generate_enum(&graph, current_node, &mut file)
                }
                "struct" => {
                    generate_struct(&graph, current_node, &mut file)
                }
                _ => {}
            }
        }
    }
    file
}

fn generate_struct(graph: &Graph<Token, Element, Directed, u32>, current_node: NodeIndex<u32>, file: &mut String) {
    for head_edges in graph.edges_directed(current_node, Direction::Outgoing) {
        let name_node = head_edges.source();
        let type_node = head_edges.target();

        let struct_name = graph[name_node].get_value();
        let _struct_type = graph[type_node].get_value();

        println!("{:?}", struct_name);

        file.push_str("#[derive(Debug, Serialize, Deserialize, SteamMsg)]\n");
        file.push_str(&format!("pub struct {} {{\n", struct_name));

        let mut member_edges = graph.edges_directed(type_node, Direction::Outgoing);


        while let Some(member) = member_edges.next() {
            let member_node = member.target();
            let member_name = graph[member_node].get_value().to_owned();

            // we can unwrap this one, because they always come in 2
            let member_type_node = member_edges.next().unwrap().target();
            let member_type = graph[member_type_node].get_value().to_owned();

            file.push_str(&format!("\t{}: {},\n", member_name, member_type));
        }
        file.push_str("}\n\n");
    }
}

fn generate_enum(graph: &Graph<Token, Element, Directed, u32>, current_node: NodeIndex<u32>, file: &mut String) {
    for head_edges in graph.edges_directed(current_node, Direction::Outgoing) {
        let name_node = head_edges.source();
        let type_node = head_edges.target();

        let enum_name = graph[name_node].get_value();
        let enum_type = graph[type_node].get_value();

        let is_flag_type = enum_type == "flags";

        if is_flag_type {
            file.push_str("bitflags! {\n");
            file.push_str("\t#[derive(Serialize, Deserialize)]\n");
            file.push_str(&format!("\tpub struct {}: i32 {{\n", enum_name));
        } else {
            file.push_str("#[derive(FromPrimitive, ToPrimitive, Debug, PartialEq, Eq, \
                Serialize, Deserialize)]\n");
            file.push_str(&format!("#[repr({})]\n", enum_type));
            file.push_str(&format!("pub enum {} {{\n", enum_name));
        }

//            println!("current node is: {}\n", enum_name);
//            println!("type is: {}\n", enum_type);

        for member in graph.edges_directed(type_node,
                                           Direction::Outgoing) {
            let member_node = member.target();
            let mut member_name = graph[member_node].get_value().to_owned();

//                println!("current member is: {}\n", member_name);

            if member_name.find('|').is_some() {
                member_name = flags_to_bitflags(member_name);
            }

            if is_flag_type {
                file.push_str(&format!("\t\tconst {};\n", member_name));
            } else {
                file.push_str(&format!("\t{},\n", member_name));
            }
        }
        if is_flag_type {
            file.push_str("\t}\n}\n\n");
        } else {
            file.push_str("}\n\n");
        }
    }
}


pub fn write_to_file(path: &str, buffer: &str) {
    fs::File::create(path).unwrap();
    fs::write(path, buffer).unwrap();
}

pub fn append_to_file(buffer: &str, path: &str) {
    let mut file = fs::OpenOptions::new().append(true).open(path).unwrap();
    file.write_all(buffer.as_bytes()).unwrap();
}

fn flags_to_bitflags(stream: String) -> String {
    let mut stream_string = stream;
    let offset = stream_string.find(';').unwrap_or_else(|| stream_string.len());

    let new_stream_string: String = stream_string.drain(..offset).collect();

    let split_assignment: Vec<&str> = new_stream_string.split("= ").collect();

    let variable_name = split_assignment[0];
    let variable_expression = split_assignment[1].to_owned();
    let flags_vector: Vec<&str> = variable_expression.split(" | ").collect();

    let translated_flags: Vec<_> = flags_vector.iter().map(|c| "Self::".to_owned() + c + ".bits").collect();
    variable_name.to_owned() + " = " + &translated_flags.join(" | ")
}

