use tree_sitter::{Language, Node, Parser};
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
block append name
  span
    div
  append name
    img(src="test")
      h1

section

    "#;

    let language = unsafe { tree_sitter_pug() };
    parser.set_language(language).unwrap();

    let tree = parser.parse(pug_input, None).unwrap();
    let mut root_node = tree.root_node();

    let mut state = State {
        html_text: String::new(),
        pug_text: pug_input.to_string(),
        ranges: Vec::new(),
    };

    traverse_tree(&mut root_node, pug_input.as_bytes(), &mut state);

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

fn visit_attribute_name(node: &mut Node, source: &[u8], state: &mut State) {
    let name_text = node.utf8_text(source).unwrap();
    push_range(state, name_text, Some(node.range()));
}

fn visit_attributes(node: &mut Node, source: &[u8], state: &mut State) {
    let mut first = true;

    let mut child_cursor = node.walk();
    for attribute in node.named_children(&mut child_cursor) {
        if !first {
            push_range(state, ", ", None);
        } else {
            first = false;
        }

        let mut attribute_cursor = attribute.walk();
        let mut children = attribute.named_children(&mut attribute_cursor);

        let attribute_name = children.next();
        if let Some(mut attribute_name) = attribute_name {
            visit_attribute_name(&mut attribute_name, source, state)
        };

        let attribute_value = children.next();
        match attribute_value {
            Some(mut attribute_value) => {
                push_range(state, "=", None);
                traverse_tree(&mut attribute_value, source, state);
            }
            None => {
                if let Some(attribute_name) = attribute_name {
                    push_range(state, "=", None);
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

fn visit_tag_name(node: &mut Node, source: &[u8], state: &mut State) {
    let name = node.utf8_text(source).unwrap();
    push_range(state, name, Some(node.range()));
}

fn visit_id_class(nodes: &Vec<Node>, source: &[u8], state: &mut State) {
    let mut start = true;

    for node in nodes {
        if !start {
            push_range(state, " ", None);
        }

        let mut range = node.range();
        range.start_byte += 1;
        let text = node.utf8_text(source).unwrap()[1..].to_string();

        push_range(state, &text, Some(range));

        start = false;
    }
}

fn visit_tag(node: &mut Node, source: &[u8], state: &mut State) {
    let mut cursor = node.walk();
    let mut child_nodes = node.named_children(&mut cursor);

    let mut name_node = child_nodes.next().unwrap();
    push_range(state, "<", None);
    traverse_tree(&mut name_node, source, state);

    let mut has_children = false;

    let mut classes: Vec<Node> = Vec::new();
    let mut ids: Vec<Node> = Vec::new();

    for mut child_node in child_nodes {
        if child_node.kind() == "class" {
            classes.push(child_node);
            continue;
        }

        if child_node.kind() == "id" {
            ids.push(child_node);
            continue;
        }

        if child_node.kind() == "attributes" {
            if classes.len() > 0 {
                push_range(state, " class=\"", None);
                visit_id_class(&classes, source, state);
                push_range(state, "\"", None);
            }

            if ids.len() > 0 {
                push_range(state, " id=\"", None);
                visit_id_class(&ids, source, state);
                push_range(state, "\"", None);
            }

            push_range(state, " ", None);
            traverse_tree(&mut child_node, source, state);

            continue;
        }

        if !has_children {
            if is_void_element(name_node.utf8_text(source).unwrap()) {
                push_range(state, "/", None);
            }
            push_range(state, ">", None);
            has_children = true;
        }

        // found something else that needs no extra handling
        traverse_tree(&mut child_node, source, state);
    }

    if !has_children {
        if is_void_element(name_node.utf8_text(source).unwrap()) {
            push_range(state, "/", None);
        }
        push_range(state, ">", None);
    }

    if !is_void_element(name_node.utf8_text(source).unwrap()) {
        push_range(state, "</", None);
        push_range(state, name_node.utf8_text(source).unwrap(), None);
        push_range(state, ">", None);
    }

    // TODO: parse content for {{angular_interpolation}} using angular_content parser
}

fn visit_conditional(node: &mut Node, source: &[u8], state: &mut State) {
    let mut cursor = node.walk();
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

    let children = conditional_cursor.node().named_children(&mut cursor);
    for mut child in children {
        traverse_tree(&mut child, source, state);
    }
}

fn visit_pipe(node: &mut Node, source: &[u8], state: &mut State) {
    let mut cursor = node.walk();

    cursor.goto_first_child();
    while cursor.goto_next_sibling() {
        if cursor.node().is_named() {
            traverse_tree(&mut cursor.node(), source, state);
        }
    }
}

fn visit_tag_interpolation(node: &mut Node, source: &[u8], state: &mut State) {
    let mut interpolation_cursor = node.walk();

    interpolation_cursor.goto_first_child();
    interpolation_cursor.goto_next_sibling();
    let children = interpolation_cursor
        .node()
        .named_children(&mut interpolation_cursor);

    for mut child in children {
        traverse_tree(&mut child, source, state);
    }
}

fn visit_filename(node: &mut Node, source: &[u8], state: &mut State) {
    push_range(state, "<a href=\"", None);
    push_range(state, node.utf8_text(source).unwrap(), Some(node.range()));
    push_range(state, "\">", None);
}

fn visit_extends_include(node: &mut Node, source: &[u8], state: &mut State) {
    for mut child in node.named_children(&mut node.walk()) {
        traverse_tree(&mut child, source, state);
    }
}

fn visit_case_when(node: &mut Node, source: &[u8], state: &mut State) {
    let cursor = &mut node.walk();

    let children = node.named_children(cursor);
    for mut child in children {
        if child.kind() == "javascript" {
            push_range(state, "<script>return ", None);
            push_range(state, child.utf8_text(source).unwrap(), Some(child.range()));
            push_range(state, ";</script>", None);
        } else {
            traverse_tree(&mut child, source, state);
        }
    }
}

fn visit_unbuffered_code(node: &mut Node, source: &[u8], state: &mut State) {
    for mut child in node.named_children(&mut node.walk()) {
        if child.kind() == "javascript" {
            push_range(state, "<script>", None);
            push_range(
                state,
                &child.utf8_text(source).unwrap(),
                Some(child.range()),
            );
            push_range(state, ";</script>", None);
        } else {
            traverse_tree(&mut child, source, state);
        }
    }
}

fn visit_buffered_code(node: &mut Node, source: &[u8], state: &mut State) {
    for mut child in node.named_children(&mut node.walk()) {
        if child.kind() == "javascript" {
            push_range(state, "<script>return ", None);
            push_range(
                state,
                &child.utf8_text(source).unwrap(),
                Some(child.range()),
            );
            push_range(state, ";</script>", None);
        } else {
            traverse_tree(&mut child, source, state);
        }
    }
}

fn traverse_tree(node: &mut Node, source: &[u8], state: &mut State) {
    let node_type = node.kind();

    let mut cursor = node.walk();

    if node.is_named() {
        match node_type {
            "source_file" | "children" | "mixin_definition" | "block_definition" | "block_use"
            | "each" => {
                let mut child_cursor = cursor.clone();
                let children = node.named_children(&mut child_cursor);
                for mut child in children {
                    traverse_tree(&mut child, source, state);
                }
            }
            "iteration_variable" | "iteration_iterator" => {
                for mut child in node.named_children(&mut cursor) {
                    if child.kind() == "javascript" {
                        push_range(state, "<script>return ", None);
                        push_range(state, child.utf8_text(source).unwrap(), Some(child.range()));
                        push_range(state, ";</script>", None);
                    } else {
                        traverse_tree(&mut child, source, state);
                    }
                }
            }
            "script_block" => {
                for mut child in node.named_children(&mut cursor) {
                    if child.kind() == "javascript" {
                        push_range(state, "<script>", None);
                        push_range(state, child.utf8_text(source).unwrap(), Some(child.range()));
                        push_range(state, ";</script>", None);
                    } else {
                        traverse_tree(&mut child, source, state);
                    }
                }
            }
            "unbuffered_code" => visit_unbuffered_code(node, source, state),
            "buffered_code" | "unescaped_buffered_code" => visit_buffered_code(node, source, state),
            "escaped_string_interpolation" => {
                let interpolation_content = node.named_children(&mut cursor).next();
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
            "when" | "case" => visit_case_when(node, source, state),
            "tag_interpolation" => {
                visit_tag_interpolation(node, source, state);
            }
            "pipe" => {
                visit_pipe(node, source, state);
            }
            "conditional" => {
                visit_conditional(node, source, state);
            }
            "tag" | "filter" => visit_tag(node, source, state),
            "tag_name" | "filter_name" => visit_tag_name(node, source, state),
            "attributes" => visit_attributes(node, source, state),
            "attribute_name" => visit_attribute_name(node, source, state),
            "javascript" => {
                push_range_surround(state, node.utf8_text(source).unwrap(), node.range(), "'");
            }
            "quoted_attribute_value" => {
                push_range(state, node.utf8_text(source).unwrap(), Some(node.range()));
            }
            "content" => {
                for mut interpolation in node.named_children(&mut cursor) {
                    traverse_tree(&mut interpolation, source, state);
                }
                // Always traverse the whole content after we've traversed the interpolation, so they
                // appear after in the conversion ranges
                push_range(state, node.utf8_text(source).unwrap(), Some(node.range()));
            }
            "extends" | "include" => visit_extends_include(node, source, state),
            "filename" => visit_filename(node, source, state),
            "keyword" | "mixin_attributes" | "comment" | "block_name" => {}
            "ERROR" => {
                for mut interpolation in node.named_children(&mut cursor) {
                    traverse_tree(&mut interpolation, source, state);
                }
            }
            _ => {
                println!("Unhandled node type: {}", node_type);
            }
        }
    }
}
