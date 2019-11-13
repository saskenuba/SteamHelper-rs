use petgraph::prelude::*;

pub fn generate_file_from_tree(graph: Graph<String, &str>, entry: NodeIndex) {
    let mut dfs = Dfs::new(&graph, entry);
    while let Some(nx) = dfs.next(&graph) {
        if !graph[nx].starts_with("Msg") { continue; }

        println!("#[derive(Debug, Serialize, Deserialize)]");
        println!("struct: {} {{", graph[nx]);

        let neighbors = graph.neighbors_directed(nx, Direction::Outgoing);

        // the printing is reversed because neighbors_directed does not have a method to reverse
        // found nodes
        let mut new_str: Vec<String> = Vec::new();
        for (mut iterator, x) in neighbors.enumerate() {
            iterator += 1;
            if iterator % 2 == 0 {
                new_str.push(format!("\t{}: ", graph[x]));
                new_str.swap(iterator - 2, iterator - 1);
            }
            if iterator % 2 == 1 {
                new_str.push(format!("{},\n", graph[x]));
            }
        }
        print!("{}", new_str.join(""));
        println!("}}\n");
    }
}

fn generate_enum_from_tree(graph: Graph<String, &str>, entry: NodeIndex) {}
