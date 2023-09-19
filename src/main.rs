use tree_sitter::{Language, Node, Parser, TreeCursor};
extern "C" {
    fn tree_sitter_pug() -> Language;
}

struct Range {
    html_end: usize,
    html_start: usize,
    pug_end: usize,
    pug_start: usize,
}

struct State {
    html_text: String,
    pug_text: String,
    ranges: Vec<Range>,
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

    let mut state = State {
        html_text: String::new(),
        pug_text: pug_input.to_string(),
        ranges: Vec::new(),
    };

    traverse_tree(&mut cursor, pug_input.as_bytes(), &mut state);

    println!("{}", pug_input);
    println!("{}\n", root_node.to_sexp());
    println!("{}", state.html_text);
    for range in state.ranges {
        println!(
            "'{}' => '{}'",
            state.html_text[range.html_start..range.html_end].to_string(),
            state.pug_text[range.pug_start..range.pug_end].to_string()
        );
    }
}

fn push_range(state: &mut State, to_push: &str, pug_range: Option<tree_sitter::Range>) {
    match pug_range {
        Some(range) => {
            let html_len = state.html_text.len();

            let range = Range {
                html_start: html_len,
                html_end: html_len + to_push.len(),
                pug_start: range.start_byte,
                pug_end: range.end_byte,
            };

            state.ranges.push(range);
        }
        _ => {}
    }

    state.html_text.push_str(&to_push);
}

fn visit_attributes(cursor: &mut TreeCursor, node: &mut Node, source: &[u8], state: &mut State) {
    let mut first = true;

    let mut child_cursor = cursor.clone();
    for attribute in node.named_children(&mut child_cursor) {
        if !first {
            push_range(state, ", ", None);
        } else {
            first = false;
        }

        let mut attribute_cursor = cursor.clone();
        let mut children = attribute.named_children(&mut attribute_cursor);

        let attribute_name = children.next().unwrap();
        let attribute_value = children.next();

        let name_text = attribute_name.utf8_text(source).unwrap();
        push_range(state, name_text, Some(attribute_name.range()));
        push_range(state, "=", None);

        match attribute_value {
            Some(attribute_value) => {
                let text = attribute_value.utf8_text(source).unwrap().to_string();

                match attribute_value.kind() {
                    // Just make javascript attributes into valid HTML
                    "javascript" => {
                        push_range_surround(state, &text, attribute_value.range(), "'");
                    }
                    "quoted_attribute_value" => {
                        push_range(state, &text, Some(attribute_value.range()));
                    }
                    _ => {}
                }
            }
            None => {
                push_range_surround(
                    state,
                    attribute_name.utf8_text(source).unwrap(),
                    attribute_name.range(),
                    "'",
                );
            }
        }
    }
}

fn push_range_surround(
    state: &mut State,
    to_push: &str,
    pug_range: tree_sitter::Range,
    surround: &str,
) {
    push_range(state, surround, None);
    push_range(state, to_push, Some(pug_range));
    push_range(state, surround, None);
}

fn visit_tag(cursor: &mut TreeCursor, node: &mut Node, source: &[u8], state: &mut State) {
    let mut cursor_mutable = cursor.clone();

    let mut child_nodes = node.named_children(&mut cursor_mutable);
    let name_node = child_nodes.next().unwrap();
    let name = name_node.utf8_text(source).unwrap();

    push_range(state, "<", None);
    push_range(state, name, Some(name_node.range()));

    let mut attribute_cursor = cursor.clone();
    let attributes = child_nodes.next();

    match attributes {
        Some(mut attributes) => {
            push_range(state, " ", None);
            visit_attributes(&mut attribute_cursor, &mut attributes, source, state);
            if is_void_element(name) {
                push_range(state, "/>", None);
            } else {
                push_range(state, ">", None);
                let children_elements = child_nodes.next();
                match children_elements {
                    Some(children_elements) => {
                        traverse_tree(&mut children_elements.walk(), source, 0, state);
                    }
                    None => {}
                }
                push_range(state, &format!("</{}>", name).to_string(), None);
            }
        }
        None => {}
    }

    // TODO: parse content for {{angular_interpolation}} using angular_content parser
}

fn visit_conditional(cursor: &mut TreeCursor, node: &mut Node, source: &[u8], state: &mut State) {
    let mut child_cursor = cursor.clone();
    let mut conditional_cursor = node.walk();

    conditional_cursor.goto_first_child();
    conditional_cursor.goto_next_sibling();

    if conditional_cursor.node().kind() == "javascript" {
        let condition = conditional_cursor.node();

        push_range(state, "<script>return ", None);
        push_range(
            state,
            condition.utf8_text(source).unwrap(),
            Some(condition.range()),
        );
        push_range(state, ";</script>", None);
        conditional_cursor.goto_next_sibling();
    }

    conditional_cursor.goto_next_sibling();

    let children = conditional_cursor.node().named_children(&mut child_cursor);
    for child in children {
        traverse_tree(&mut child.walk(), source, 0, state);
    }
}

fn visit_pipe(cursor: &mut TreeCursor, _node: &mut Node, source: &[u8], state: &mut State) {
    cursor.goto_first_child();
    while cursor.goto_next_sibling() {
        if cursor.node().is_named() {
            for interpolation in cursor.node().named_children(cursor) {
                traverse_tree(&mut interpolation.walk(), source, 0, state);
            }
        }
    }
}

fn visit_tag_interpolation(_cursor: &mut TreeCursor, node: &mut Node, source: &[u8], state: &mut State) {
    let mut interpolation_cursor = node.walk();

    interpolation_cursor.goto_first_child();
    interpolation_cursor.goto_next_sibling();
    let children = interpolation_cursor
        .node()
        .named_children(&mut interpolation_cursor);

    for child in children {
        traverse_tree(&mut child.walk(), source, 0, state);
    }
}

fn traverse_tree(cursor: &mut TreeCursor, source: &[u8], depth: usize, state: &mut State) {
    let mut node = cursor.node();

    if node.is_named() {
        let node_type = node.kind();

        match node_type {
            "source_file" | "children" => {
                let mut child_cursor = cursor.clone();
                let children = node.named_children(&mut child_cursor);
                for child in children {
                    traverse_tree(&mut child.walk(), source, depth, state);
            "escaped_string_interpolation" => {
                let interpolation_content = node.named_children(cursor).next();
                match interpolation_content {
                    Some(interpolation_content) => {
                        let text = interpolation_content.utf8_text(source).unwrap();
                        push_range(state, "<script>return ", None);
                        push_range(state, text, Some(interpolation_content.range()));
                        push_range(state, ";</script>", None);
                    }
                    None => {}
                }
            }
            "tag_interpolation" => {
                visit_tag_interpolation(cursor, &mut node, source, state);
            }
            "pipe" => {
                visit_pipe(cursor, &mut node, source, state);
            }
            "conditional" => {
                visit_conditional(cursor, &mut node, source, state);
            }
            "tag" => visit_tag(cursor, &mut node, source, state),
            _ => {}
        }
    }
}
