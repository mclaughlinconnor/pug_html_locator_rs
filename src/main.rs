use tree_sitter::{Language, Node, Parser, TreeCursor};
extern "C" {
    fn tree_sitter_pug() -> Language;
}

fn is_void_element(tag_name: &str) -> bool {
    match tag_name {
        "area" | "base" | "br" | "col" | "embed" | "hr" | "img" | "input" | "link" | "meta"
        | "param" | "source" | "track" | "wbr" => {
            return true;
        }
        _ => {
            return false;
        }
    }
}

fn main() {
    let mut parser = Parser::new();

    let pug_input = r#"
        tag(attribute=isAuthenticated ? true : false, attribute)
          tag_two(attribute)
    "#;

    let language = unsafe { tree_sitter_pug() };
    parser.set_language(language).unwrap();

    let tree = parser.parse(pug_input, None).unwrap();
    let root_node = tree.root_node();
    let mut cursor = root_node.walk();

    let mut s = String::new();

    traverse_tree(&mut cursor, pug_input.as_bytes(), 0, &mut s);

    println!("{}", pug_input);
    println!("{}\n", root_node.to_sexp());
    println!("{}", s);
}

fn visit_attributes(cursor: &mut TreeCursor, node: &mut Node, source: &[u8], s: &mut String) {
    let mut first = true;

    let mut child_cursor = cursor.clone();
    for attribute in node.named_children(&mut child_cursor) {
        if !first {
            s.push_str(", ");
        } else {
            first = false;
        }

        let mut attribute_cursor = cursor.clone();
        let mut children = attribute.named_children(&mut attribute_cursor);

        let attribute_name = children.next();
        let attribute_value = children.next();

        match (attribute_name, attribute_value) {
            (Some(attribute_name), Some(attribute_value)) => {
                s.push_str(
                    &format!(
                        "{}={}",
                        attribute_name.utf8_text(source).unwrap(),
                        attribute_value.utf8_text(source).unwrap()
                    )
                    .to_string(),
                );
            }
            (Some(attribute_name), _) => {
                s.push_str(
                    &format!("{0}='{0}'", attribute_name.utf8_text(source).unwrap(),).to_string(),
                );
            }
            (_, _) => {}
        }
    }
}

fn visit_tag(cursor: &mut TreeCursor, node: &mut Node, source: &[u8], s: &mut String) {
    let mut cursor_mutable = cursor.clone();

    let mut child_nodes = node.named_children(&mut cursor_mutable);
    let name = child_nodes.next().unwrap().utf8_text(source).unwrap();
    s.push_str(&format!("<{} ", name).to_string());

    let mut attribute_cursor = cursor.clone();
    let attributes = child_nodes.next();

    match attributes {
        Some(mut attributes) => {
            visit_attributes(&mut attribute_cursor, &mut attributes, source, s);
            if is_void_element(name) {
                s.push_str("/>");
            } else {
                s.push_str(">");
                let children_elements = child_nodes.next();
                match children_elements {
                    Some(children_elements) => {
                        traverse_tree(&mut children_elements.walk(), source, 0, s);
                    }
                    None => {}
                }
                s.push_str(&format!("</{}>", name).to_string());
            }
        }
        None => {}
    }
}

fn traverse_tree(cursor: &mut TreeCursor, source: &[u8], depth: usize, s: &mut String) {
    let mut node = cursor.node();

    if node.is_named() {
        let node_type = node.kind();

        match node_type {
            "source_file" | "children" => {
                let mut child_cursor = cursor.clone();
                let children = node.named_children(&mut child_cursor);
                for child in children {
                    traverse_tree(&mut child.walk(), source, depth, s);
                }
            }
            "tag" => visit_tag(cursor, &mut node, source, s),
            _ => {}
        }
    }
}
