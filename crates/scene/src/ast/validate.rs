use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::matching::{MatchParseError, parse_window_match};
use hypreact_core::{LayoutNodeMeta, LayoutNodeType, SlotTake, SourceLayoutNode, WindowMatch};

#[derive(Debug, Clone)]
pub struct ValidatedLayoutTree {
    pub root: SourceLayoutNode,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthoredNodeMeta {
    pub id: Option<String>,
    pub class: Vec<String>,
    pub name: Option<String>,
    pub data: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthoredLayoutNode {
    Workspace { meta: AuthoredNodeMeta, children: Vec<AuthoredLayoutNode> },
    Group { meta: AuthoredNodeMeta, children: Vec<AuthoredLayoutNode> },
    Window { meta: AuthoredNodeMeta, match_expr: Option<String> },
    Slot { meta: AuthoredNodeMeta, match_expr: Option<String>, take: SlotTake },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LayoutValidationError {
    #[error("layout root must be a workspace node")]
    RootMustBeWorkspace,
    #[error("node id `{id}` is duplicated")]
    DuplicateId { id: String },
    #[error("node type `{child:?}` is not allowed under `{parent:?}`")]
    InvalidChild { parent: LayoutNodeType, child: LayoutNodeType },
    #[error("slot `take` must be a positive integer or `remaining`")]
    InvalidSlotTake,
    #[error("`match` must contain at least one clause when provided")]
    EmptyMatch,
    #[error("failed to parse `match`: {source}")]
    InvalidMatch {
        #[from]
        source: MatchParseError,
    },
}

impl ValidatedLayoutTree {
    pub fn from_authored(root: AuthoredLayoutNode) -> Result<Self, LayoutValidationError> {
        Self::new(normalize_authored_node(root)?)
    }

    pub fn new(root: SourceLayoutNode) -> Result<Self, LayoutValidationError> {
        if !matches!(root, SourceLayoutNode::Workspace { .. }) {
            return Err(LayoutValidationError::RootMustBeWorkspace);
        }

        let mut ids = BTreeSet::new();
        validate_node(&root, None, &mut ids)?;

        Ok(Self { root })
    }
}

fn normalize_authored_node(
    node: AuthoredLayoutNode,
) -> Result<SourceLayoutNode, LayoutValidationError> {
    Ok(match node {
        AuthoredLayoutNode::Workspace { meta, children } => SourceLayoutNode::Workspace {
            meta: normalize_meta(meta),
            children: children
                .into_iter()
                .map(normalize_authored_node)
                .collect::<Result<Vec<_>, _>>()?,
        },
        AuthoredLayoutNode::Group { meta, children } => SourceLayoutNode::Group {
            meta: normalize_meta(meta),
            children: children
                .into_iter()
                .map(normalize_authored_node)
                .collect::<Result<Vec<_>, _>>()?,
        },
        AuthoredLayoutNode::Window { meta, match_expr } => SourceLayoutNode::Window {
            meta: normalize_meta(meta),
            window_match: normalize_match(match_expr)?,
        },
        AuthoredLayoutNode::Slot { meta, match_expr, take } => SourceLayoutNode::Slot {
            meta: normalize_meta(meta),
            window_match: normalize_match(match_expr)?,
            take,
        },
    })
}

fn normalize_meta(meta: AuthoredNodeMeta) -> LayoutNodeMeta {
    LayoutNodeMeta { id: meta.id, class: meta.class, name: meta.name, data: meta.data }
}

fn normalize_match(
    match_expr: Option<String>,
) -> Result<Option<WindowMatch>, LayoutValidationError> {
    match match_expr {
        Some(match_expr) => Ok(Some(parse_window_match(&match_expr)?)),
        None => Ok(None),
    }
}

fn validate_node(
    node: &SourceLayoutNode,
    parent: Option<LayoutNodeType>,
    ids: &mut BTreeSet<String>,
) -> Result<(), LayoutValidationError> {
    let node_type = node.node_type();

    if let Some(parent) = parent {
        let child_allowed = matches!(
            (parent, node_type),
            (LayoutNodeType::Workspace, LayoutNodeType::Group)
                | (LayoutNodeType::Workspace, LayoutNodeType::Window)
                | (LayoutNodeType::Workspace, LayoutNodeType::Slot)
                | (LayoutNodeType::Group, LayoutNodeType::Group)
                | (LayoutNodeType::Group, LayoutNodeType::Window)
                | (LayoutNodeType::Group, LayoutNodeType::Slot)
        );

        if !child_allowed {
            return Err(LayoutValidationError::InvalidChild { parent, child: node_type });
        }
    }

    if let Some(id) = &node.meta().id {
        if !ids.insert(id.clone()) {
            return Err(LayoutValidationError::DuplicateId { id: id.clone() });
        }
    }

    match node {
        SourceLayoutNode::Window { window_match, .. } => validate_match(window_match.as_ref())?,
        SourceLayoutNode::Slot { window_match, take, .. } => {
            validate_match(window_match.as_ref())?;

            if matches!(take, SlotTake::Count(0)) {
                return Err(LayoutValidationError::InvalidSlotTake);
            }
        }
        SourceLayoutNode::Workspace { children, .. } | SourceLayoutNode::Group { children, .. } => {
            for child in children {
                validate_node(child, Some(node_type), ids)?;
            }
        }
    }

    Ok(())
}

fn validate_match(window_match: Option<&WindowMatch>) -> Result<(), LayoutValidationError> {
    if matches!(window_match, Some(WindowMatch { clauses }) if clauses.is_empty()) {
        return Err(LayoutValidationError::EmptyMatch);
    }

    Ok(())
}
