use tree_sitter::{Language, Parser, TreeCursor};
extern "C" {
    fn tree_sitter_pug() -> Language;
}

fn main() {
    let mut parser = Parser::new();

    let pug_input = "tag(attribute='value')";

    let language = unsafe { tree_sitter_pug() };
    parser.set_language(language).unwrap();

    let tree = parser.parse(pug_input, None).unwrap();
    let root_node = tree.root_node();
    let mut cursor = root_node.walk();

    traverse_tree(&mut cursor, pug_input.as_bytes(), 0);
}

fn traverse_tree(cursor: &mut TreeCursor, source: &[u8], depth: usize) {
    let node = cursor.node();

    if node.is_named() {
        let node_type = node.kind();
        println!("{:indent$}{}", "", node_type, indent = depth * 4);
    }

    if cursor.goto_first_child() {
        loop {
            traverse_tree(cursor, source, depth + 1);
            if !cursor.goto_next_sibling() {
                break;
            }
        }

        cursor.goto_parent();
    }
}
