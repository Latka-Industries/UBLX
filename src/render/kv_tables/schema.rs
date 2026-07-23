//! Schema tree: XML-style (attributes/children) and TOML-style (map as children).
//! Structured [`TreeNode`] for web collapse; flattened lines for the TUI.

use serde_json::{Map, Value};

use super::consts::{SchemaKeys, SectionKeys, tree_prefixes};
use super::format;
use super::sections::{Section, TreeNode, TreeSection};

fn flatten_node(node: &TreeNode, line_prefix: &str, continuation: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(ref val) = node.value {
        if node.children.is_empty() {
            out.push(format::prefixed_label_with_value(
                line_prefix,
                &node.label,
                val,
            ));
            return out;
        }
        out.push(format::prefixed_label_with_value(
            line_prefix,
            &node.label,
            val,
        ));
    } else {
        out.push(format::prefixed_label(line_prefix, &node.label));
    }
    let n = node.children.len();
    for (i, child) in node.children.iter().enumerate() {
        let (branch, cont) = tree_prefixes(continuation, i == n.saturating_sub(1));
        // Attribute-style leaves keep the TUI `"prefix key: value"` shape.
        if !child.branch
            && child.children.is_empty()
            && let Some(ref v) = child.value
        {
            out.push(format!(
                "{} {}: {}",
                branch,
                format::format_key(&child.label),
                v
            ));
        } else {
            out.extend(flatten_node(child, &branch, &cont));
        }
    }
    out
}

fn schema_node_tree(value: &Value, label: &str) -> TreeNode {
    let label = label.to_string();
    let Value::Object(map) = value else {
        return TreeNode {
            label,
            value: Some(format::value_to_string(
                value,
                format::DEFAULT_MAX_ARRAY_INLINE,
            )),
            children: Vec::new(),
            branch: false,
        };
    };

    if map.is_empty() {
        return TreeNode {
            label,
            value: None,
            children: Vec::new(),
            branch: false,
        };
    }

    if SchemaKeys::has_children_or_attributes(map) {
        let mut children = Vec::new();
        if let Some(attrs) = map.get(SchemaKeys::ATTRIBUTES).and_then(Value::as_object)
            && !attrs.is_empty()
        {
            for (k, v) in attrs {
                children.push(TreeNode {
                    label: k.clone(),
                    value: Some(format::value_to_string(v, format::DEFAULT_MAX_ARRAY_INLINE)),
                    children: Vec::new(),
                    branch: false,
                });
            }
        }
        if let Some(children_map) = map
            .get(SchemaKeys::CHILDREN)
            .and_then(Value::as_object)
            .filter(|c| !c.is_empty())
        {
            children.extend(walk_schema_child_nodes(children_map));
        }
        let branch = !children.is_empty();
        return TreeNode {
            label,
            value: None,
            children,
            branch,
        };
    }

    // TOML-style: each key is a child node or leaf
    let children = walk_schema_child_nodes(map);
    let branch = !children.is_empty();
    TreeNode {
        label,
        value: None,
        children,
        branch,
    }
}

fn walk_schema_child_nodes(map: &Map<String, Value>) -> Vec<TreeNode> {
    map.iter()
        .map(|(name, val)| schema_node_tree(val, name))
        .collect()
}

fn schema_value_to_tree(value: &Value) -> Vec<TreeNode> {
    match value {
        Value::Object(map) if !map.is_empty() => map
            .iter()
            .map(|(name, node_val)| schema_node_tree(node_val, name))
            .collect(),
        Value::Array(arr) if !arr.is_empty() => arr
            .iter()
            .map(|v| {
                let label = v
                    .as_object()
                    .and_then(|o| {
                        o.get("name")
                            .or_else(|| o.get("type"))
                            .or_else(|| o.get("id"))
                            .and_then(Value::as_str)
                    })
                    .unwrap_or("…");
                schema_node_tree(v, label)
            })
            .collect(),
        _ => vec![schema_node_tree(value, "…")],
    }
}

/// Flatten structured roots to the same line list the TUI used to build directly.
#[must_use]
pub fn tree_roots_to_lines(roots: &[TreeNode]) -> Vec<String> {
    let mut lines = Vec::new();
    for (idx, root) in roots.iter().enumerate() {
        if idx > 0 {
            lines.push(String::new());
        }
        lines.extend(flatten_node(root, "", ""));
    }
    if lines.is_empty() {
        lines.push("—".to_string());
    }
    lines
}

/// Push a schema tree section (structured for web; TUI flattens via [`tree_roots_to_lines`]).
pub fn push_schema_section(sections: &mut Vec<Section>, value: &Value) {
    let mut roots = schema_value_to_tree(value);
    if roots.is_empty() && !value.is_null() {
        roots.push(TreeNode {
            label: "—".into(),
            value: None,
            children: Vec::new(),
            branch: false,
        });
    }
    sections.push(Section::Tree(TreeSection {
        title: format::format_key(SectionKeys::SCHEMA),
        roots,
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn schema_tree_flattens_like_prefixes() {
        let v = json!({
            "root": {
                "attributes": { "id": "1" },
                "children": {
                    "child": {}
                }
            }
        });
        let roots = schema_value_to_tree(&v);
        let lines = tree_roots_to_lines(&roots);
        assert!(lines.iter().any(|l| l.contains("root")));
        assert!(lines.iter().any(|l| l.contains("Id") && l.contains('1')));
        assert!(lines.iter().any(|l| l.contains("child")));
    }
}
