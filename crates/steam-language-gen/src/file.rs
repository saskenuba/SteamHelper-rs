use petgraph::prelude::*;

pub fn generate_file_from_tree(graph: Graph<String, &str>, entry: NodeIndex) -> String {
    let mut file = String::new();
    let mut dfs = Dfs::new(&graph, entry);
    while let Some(nx) = dfs.next(&graph) {
        if !graph[nx].starts_with("Msg") { continue; }

        file.push_str(&format!("#[derive(Debug, Serialize, Deserialize)]\n"));
        file.push_str(&format!("struct: {} {{\n", graph[nx]));

        let neighbors = graph.neighbors_directed(nx, Direction::Outgoing);

        // the printing is reversed because neighbors_directed does not have a method to reverse
        // found nodes
        let mut new_str: Vec<String> = Vec::new();
        for (mut iterator, x) in neighbors.enumerate() {
            iterator += 1;
            if iterator % 2 == 0 {
                new_str.insert(0, format!("\t{}: ", graph[x]));
            }
            if iterator % 2 == 1 {
                new_str.insert(0, format!("{},\n", graph[x]));
            }
        }
        file.push_str(&new_str.join(""));
        file.push_str("}\n\n");
    }
    file
}

fn generate_enum_from_tree(graph: Graph<String, &str>, entry: NodeIndex) {}
