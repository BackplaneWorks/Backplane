//! md-check CORPUS.json: render each string of a JSON array as markdown and print the nodes JSON (Mob.nodes), one line per string.
//! twin prints the same; test/screen/md_oracle.sh compares them. Reads only the corpus file.
#[path = "../md/mod.rs"]
mod md;

fn main() {
    let path = std::env::args().nth(1).expect("usage: md-check CORPUS.json");
    let corpus: Vec<String> = serde_json::from_str(&std::fs::read_to_string(&path).expect("read corpus")).expect("corpus is a JSON array of strings");
    for s in &corpus { println!("{}", serde_json::to_string(&md::nodes_json(&md::render(s))).unwrap()); }
}
