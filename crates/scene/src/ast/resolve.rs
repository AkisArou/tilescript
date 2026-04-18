use std::collections::BTreeSet;

use thiserror::Error;
use tilescript_core::snapshot::WindowSnapshot;
use tilescript_core::types::WindowShell;

use crate::ast::validate::ValidatedLayoutTree;
use crate::matching::matches_window;
use tilescript_core::{
    LayoutNodeMeta, ResolvedLayoutNode, SlotTake, SourceLayoutNode, WindowMatch,
};

#[derive(Debug, Clone)]
pub struct ResolvedLayoutTree {
    pub root: ResolvedLayoutNode,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LayoutResolveError {
    #[error("layout must be validated before resolution")]
    InvalidRoot,
}

impl ValidatedLayoutTree {
    pub fn resolve(
        &self,
        windows: &[WindowSnapshot],
    ) -> Result<ResolvedLayoutTree, LayoutResolveError> {
        let SourceLayoutNode::Workspace { meta, children } = &self.root else {
            return Err(LayoutResolveError::InvalidRoot);
        };

        let mut claimed = BTreeSet::new();
        let resolved_children =
            children.iter().flat_map(|child| resolve_node(child, windows, &mut claimed)).collect();

        Ok(ResolvedLayoutTree {
            root: ResolvedLayoutNode::Workspace { meta: meta.clone(), children: resolved_children },
        })
    }
}

fn resolved_window_meta(meta: &LayoutNodeMeta, window: Option<&WindowSnapshot>) -> LayoutNodeMeta {
    let mut resolved = meta.clone();

    let Some(window) = window else {
        return resolved;
    };

    let mut insert = |key: &str, value: Option<&str>| {
        if let Some(value) = value {
            resolved.data.insert(key.to_owned(), value.to_owned());
        }
    };

    insert("app_id", window.app_id.as_deref());
    insert("title", window.title.as_deref());
    insert("class", window.class.as_deref());
    insert("instance", window.instance.as_deref());
    insert("role", window.role.as_deref());
    insert("window_type", window.window_type.as_deref());
    resolved.data.insert(
        "shell".to_owned(),
        match window.shell {
            WindowShell::Wayland => "wayland",
            WindowShell::Xwayland => "xwayland",
        }
        .to_owned(),
    );

    let mut add_state_class = |class_name: &str, enabled: bool| {
        if enabled && !resolved.class.iter().any(|class| class == class_name) {
            resolved.class.push(class_name.to_owned());
        }
    };

    add_state_class("focused", window.focused);
    add_state_class("urgent", window.urgent);
    add_state_class("closing", window.closing);
    add_state_class("floating", window.mode.is_floating());
    add_state_class("fullscreen", window.mode.is_fullscreen());

    resolved
}

fn resolve_node(
    node: &SourceLayoutNode,
    windows: &[WindowSnapshot],
    claimed: &mut BTreeSet<String>,
) -> Vec<ResolvedLayoutNode> {
    match node {
        SourceLayoutNode::Workspace { meta, children } => vec![ResolvedLayoutNode::Workspace {
            meta: meta.clone(),
            children: children
                .iter()
                .flat_map(|child| resolve_node(child, windows, claimed))
                .collect(),
        }],
        SourceLayoutNode::Group { meta, children } => vec![ResolvedLayoutNode::Group {
            meta: meta.clone(),
            children: children
                .iter()
                .flat_map(|child| resolve_node(child, windows, claimed))
                .collect(),
        }],
        SourceLayoutNode::Window { meta, window_match } => {
            let claimed_window = windows
                .iter()
                .find(|window| can_claim_window(window_match.as_ref(), window, claimed))
                .inspect(|window| {
                    claimed.insert(window.id.to_string());
                });

            claimed_window
                .map(|window| ResolvedLayoutNode::Window {
                    meta: resolved_window_meta(meta, Some(window)),
                    window_id: Some(window.id.clone()),
                    children: Vec::new(),
                })
                .into_iter()
                .collect()
        }
        SourceLayoutNode::Slot { meta, window_match, take } => {
            let matching_ids: Vec<_> = windows
                .iter()
                .filter(|window| can_claim_window(window_match.as_ref(), window, claimed))
                .map(|window| window.id.clone())
                .collect();

            let limit = match take {
                SlotTake::Count(count) => *count as usize,
                SlotTake::Remaining(_) => matching_ids.len(),
            };

            matching_ids
                .into_iter()
                .take(limit)
                .map(|window_id| {
                    let window = windows.iter().find(|window| window.id == window_id);
                    claimed.insert(window_id.to_string());

                    ResolvedLayoutNode::Window {
                        meta: resolved_window_meta(meta, window),
                        window_id: Some(window_id),
                        children: Vec::new(),
                    }
                })
                .collect()
        }
    }
}

fn can_claim_window(
    window_match: Option<&WindowMatch>,
    window: &WindowSnapshot,
    claimed: &BTreeSet<String>,
) -> bool {
    !claimed.contains(window.id.as_str())
        && window.mapped
        && window_match.is_none_or(|window_match| matches_window(window_match, window))
}
