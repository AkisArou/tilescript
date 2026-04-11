use std::collections::BTreeMap;

use hypreact_core::{SlotTake, SourceLayoutNode};
use hypreact_scene::ast::{AuthoredLayoutNode, AuthoredNodeMeta, ValidatedLayoutTree};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DecodePath(Vec<String>);

impl DecodePath {
    fn root() -> Self {
        Self(vec!["root".into()])
    }

    fn field(&self, field: &str) -> Self {
        let mut path = self.0.clone();
        path.push(field.to_owned());
        Self(path)
    }

    fn index(&self, index: usize) -> Self {
        let mut path = self.0.clone();
        path.push(format!("[{index}]"));
        Self(path)
    }

    fn display(&self) -> String {
        self.0.join(".")
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
struct JsAuthoredNodeMeta {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    class: JsClassName,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    data: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(untagged)]
enum JsClassName {
    #[default]
    Missing,
    One(String),
    Many(Vec<String>),
}

impl JsClassName {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::Missing => Vec::new(),
            Self::One(value) => value
                .split_ascii_whitespace()
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .collect(),
            Self::Many(values) => values,
        }
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
struct JsAuthoredNodeProps {
    #[serde(flatten)]
    meta: JsAuthoredNodeMeta,
    #[serde(default, rename = "match")]
    match_expr: Option<String>,
    #[serde(default)]
    take: Option<SlotTake>,
}

impl JsAuthoredNodeProps {
    fn merge(self, nested: JsAuthoredNodeProps) -> Self {
        Self {
            meta: self.meta.merge(nested.meta),
            match_expr: nested.match_expr.or(self.match_expr),
            take: nested.take.or(self.take),
        }
    }
}

impl JsAuthoredNodeMeta {
    fn merge(self, nested: JsAuthoredNodeMeta) -> Self {
        Self {
            id: nested.id.or(self.id),
            class: match nested.class {
                JsClassName::Missing => self.class,
                other => other,
            },
            name: nested.name.or(self.name),
            data: if nested.data.is_empty() {
                self.data
            } else {
                nested.data
            },
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum JsAuthoredLayoutNode {
    Workspace {
        #[serde(default)]
        props: Option<JsAuthoredNodeProps>,
        #[serde(flatten)]
        legacy: JsAuthoredNodeProps,
        #[serde(default)]
        children: Vec<JsAuthoredLayoutNode>,
    },
    Group {
        #[serde(default)]
        props: Option<JsAuthoredNodeProps>,
        #[serde(flatten)]
        legacy: JsAuthoredNodeProps,
        #[serde(default)]
        children: Vec<JsAuthoredLayoutNode>,
    },
    Window {
        #[serde(default)]
        props: Option<JsAuthoredNodeProps>,
        #[serde(flatten)]
        legacy: JsAuthoredNodeProps,
    },
    Slot {
        #[serde(default)]
        props: Option<JsAuthoredNodeProps>,
        #[serde(flatten)]
        legacy: JsAuthoredNodeProps,
    },
}

pub fn decode_js_layout_value(value: &serde_json::Value) -> Result<SourceLayoutNode, String> {
    let authored = decode_authored_layout_node(value, &DecodePath::root())?;
    ValidatedLayoutTree::from_authored(authored)
        .map(|validated| validated.root)
        .map_err(|error| error.to_string())
}

fn decode_authored_layout_node(
    value: &serde_json::Value,
    path: &DecodePath,
) -> Result<AuthoredLayoutNode, String> {
    let node: JsAuthoredLayoutNode = serde_json::from_value(value.clone())
        .map_err(|error| format!("{}: {error}", path.display()))?;

    decode_authored_layout_node_from_node(node, path)
}

fn decode_children(
    children: Vec<JsAuthoredLayoutNode>,
    path: &DecodePath,
) -> Result<Vec<AuthoredLayoutNode>, String> {
    children
        .into_iter()
        .enumerate()
        .map(|(index, child)| decode_authored_layout_node_from_node(child, &path.index(index)))
        .collect()
}

fn decode_authored_layout_node_from_node(
    node: JsAuthoredLayoutNode,
    path: &DecodePath,
) -> Result<AuthoredLayoutNode, String> {
    Ok(match node {
        JsAuthoredLayoutNode::Workspace {
            props,
            legacy,
            children,
        } => {
            let props = merge_node_props(props, legacy);
            AuthoredLayoutNode::Workspace {
                meta: decode_meta(props.meta),
                children: decode_children(children, &path.field("children"))?,
            }
        }
        JsAuthoredLayoutNode::Group {
            props,
            legacy,
            children,
        } => {
            let props = merge_node_props(props, legacy);
            AuthoredLayoutNode::Group {
                meta: decode_meta(props.meta),
                children: decode_children(children, &path.field("children"))?,
            }
        }
        JsAuthoredLayoutNode::Window { props, legacy } => {
            let props = merge_node_props(props, legacy);
            AuthoredLayoutNode::Window {
                meta: decode_meta(props.meta),
                match_expr: props.match_expr,
            }
        }
        JsAuthoredLayoutNode::Slot { props, legacy } => {
            let props = merge_node_props(props, legacy);
            AuthoredLayoutNode::Slot {
                meta: decode_meta(props.meta),
                match_expr: props.match_expr,
                take: props.take.unwrap_or_default(),
            }
        }
    })
}

fn merge_node_props(
    props: Option<JsAuthoredNodeProps>,
    legacy: JsAuthoredNodeProps,
) -> JsAuthoredNodeProps {
    match props {
        Some(props) => legacy.merge(props),
        None => legacy,
    }
}

fn decode_meta(meta: JsAuthoredNodeMeta) -> AuthoredNodeMeta {
    AuthoredNodeMeta {
        id: meta.id,
        class: meta.class.into_vec(),
        name: meta.name,
        data: meta.data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_js_layout_value_preserves_jsx_props_metadata() {
        let value = serde_json::json!({
            "type": "workspace",
            "props": { "id": "root" },
            "children": [{
                "type": "group",
                "props": { "id": "frame" },
                "children": [{
                    "type": "slot",
                    "props": { "id": "master", "class": "master-slot", "take": 1 },
                    "children": []
                }]
            }]
        });

        let decoded = decode_js_layout_value(&value).unwrap();

        let SourceLayoutNode::Workspace { meta, children } = decoded else {
            panic!("expected workspace root");
        };
        assert_eq!(meta.id.as_deref(), Some("root"));

        let SourceLayoutNode::Group {
            meta: group_meta,
            children: group_children,
        } = &children[0]
        else {
            panic!("expected frame group");
        };
        assert_eq!(group_meta.id.as_deref(), Some("frame"));

        let SourceLayoutNode::Slot { meta, take, .. } = &group_children[0] else {
            panic!("expected master slot");
        };
        assert_eq!(meta.id.as_deref(), Some("master"));
        assert_eq!(meta.class, vec!["master-slot".to_owned()]);
        assert_eq!(*take, SlotTake::Count(1));
    }
}
