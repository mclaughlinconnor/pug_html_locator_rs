extern crate cc;

use std::path::PathBuf;

fn main() {
    let tree_sitter_pug: PathBuf =
        std::fs::canonicalize::<PathBuf>(["..", "tree-sitter-pug", "src"].iter().collect())
            .unwrap();

    cc::Build::new()
        .include(&tree_sitter_pug)
        .file(tree_sitter_pug.join("parser.c"))
        .file(tree_sitter_pug.join("scanner.c"))
        .compile("tree-sitter-pug");
}
