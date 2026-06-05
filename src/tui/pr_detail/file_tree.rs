use std::collections::HashSet;

use crate::domain::diff::FileDiff;

#[derive(Debug, Clone)]
pub enum TreeRow {
    Dir {
        path: String,
        name: String,
        depth: usize,
        expanded: bool,
    },
    File {
        name: String,
        depth: usize,
        file_index: usize,
    },
}

pub fn build_visible_rows(files: &[FileDiff], collapsed: &HashSet<String>) -> Vec<TreeRow> {
    let root = build_tree(files);
    let mut rows = Vec::new();
    for child in &root.children {
        walk(child, 0, collapsed, &mut rows);
    }
    rows
}

struct Node {
    name: String,
    full_path: String,
    children: Vec<Node>,
    file_index: Option<usize>,
}

fn build_tree(files: &[FileDiff]) -> Node {
    let mut root = Node {
        name: String::new(),
        full_path: String::new(),
        children: Vec::new(),
        file_index: None,
    };
    for (idx, file) in files.iter().enumerate() {
        let segments: Vec<&str> = file.path.split('/').filter(|s| !s.is_empty()).collect();
        insert(&mut root, &segments, idx);
    }
    sort_node(&mut root);
    root
}

fn insert(node: &mut Node, segments: &[&str], file_index: usize) {
    if segments.is_empty() {
        return;
    }
    let head = segments[0];
    let rest = &segments[1..];
    let new_path = if node.full_path.is_empty() {
        head.to_string()
    } else {
        format!("{}/{}", node.full_path, head)
    };

    if rest.is_empty() {
        node.children.push(Node {
            name: head.to_string(),
            full_path: new_path,
            children: Vec::new(),
            file_index: Some(file_index),
        });
    } else {
        let pos = node
            .children
            .iter()
            .position(|c| c.name == head && c.file_index.is_none());
        match pos {
            Some(idx) => insert(&mut node.children[idx], rest, file_index),
            None => {
                let mut new_child = Node {
                    name: head.to_string(),
                    full_path: new_path,
                    children: Vec::new(),
                    file_index: None,
                };
                insert(&mut new_child, rest, file_index);
                node.children.push(new_child);
            }
        }
    }
}

fn sort_node(node: &mut Node) {
    node.children
        .sort_by(|a, b| match (a.file_index.is_some(), b.file_index.is_some()) {
            (false, true) => std::cmp::Ordering::Less,
            (true, false) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        });
    for c in &mut node.children {
        sort_node(c);
    }
}

fn walk(node: &Node, depth: usize, collapsed: &HashSet<String>, rows: &mut Vec<TreeRow>) {
    if let Some(idx) = node.file_index {
        rows.push(TreeRow::File {
            name: node.name.clone(),
            depth,
            file_index: idx,
        });
    } else {
        let is_collapsed = collapsed.contains(&node.full_path);
        rows.push(TreeRow::Dir {
            path: node.full_path.clone(),
            name: node.name.clone(),
            depth,
            expanded: !is_collapsed,
        });
        if !is_collapsed {
            for child in &node.children {
                walk(child, depth + 1, collapsed, rows);
            }
        }
    }
}
