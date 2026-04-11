use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use crate::focus_visual::{VisualChild, VisualEntry, VisualScope, infer_visual_scope};
use crate::wm::{WindowGeometry, WmModel};
use crate::{LayoutNodeMeta, ResolvedLayoutNode, WindowId};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FocusScopeSegment {
    Workspace,
    Group { child_index: usize, label: String },
    Visual { child_index: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FocusScopePath(Vec<FocusScopeSegment>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusScopePathParseError {
    input: String,
}

impl FocusScopePath {
    pub fn workspace() -> Self {
        Self(vec![FocusScopeSegment::Workspace])
    }

    pub fn child_group(&self, meta: &LayoutNodeMeta, child_index: usize) -> Self {
        let label = meta.id.as_deref().or(meta.name.as_deref()).unwrap_or("group").to_string();

        let mut segments = self.0.clone();
        segments.push(FocusScopeSegment::Group { child_index, label });
        Self(segments)
    }

    pub fn child_visual(&self, child_index: usize) -> Self {
        let mut segments = self.0.clone();
        segments.push(FocusScopeSegment::Visual { child_index });
        Self(segments)
    }
}

impl fmt::Display for FocusScopePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, segment) in self.0.iter().enumerate() {
            if index > 0 {
                write!(f, "/")?;
            }

            match segment {
                FocusScopeSegment::Workspace => write!(f, "$workspace")?,
                FocusScopeSegment::Group { child_index, label } => {
                    write!(f, "group[{child_index}]:{label}")?
                }
                FocusScopeSegment::Visual { child_index } => write!(f, "visual[{child_index}]")?,
            }
        }

        Ok(())
    }
}

impl FromStr for FocusScopePath {
    type Err = FocusScopePathParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let mut segments = Vec::new();

        for (index, part) in input.split('/').enumerate() {
            let segment = if index == 0 {
                if part != "$workspace" {
                    return Err(FocusScopePathParseError { input: input.to_string() });
                }

                FocusScopeSegment::Workspace
            } else if let Some(rest) = part.strip_prefix("group[") {
                let Some((index_part, label)) = rest.split_once("]:") else {
                    return Err(FocusScopePathParseError { input: input.to_string() });
                };
                let Ok(child_index) = index_part.parse::<usize>() else {
                    return Err(FocusScopePathParseError { input: input.to_string() });
                };

                FocusScopeSegment::Group { child_index, label: label.to_string() }
            } else if let Some(rest) = part.strip_prefix("visual[") {
                let Some(index_part) = rest.strip_suffix(']') else {
                    return Err(FocusScopePathParseError { input: input.to_string() });
                };
                let Ok(child_index) = index_part.parse::<usize>() else {
                    return Err(FocusScopePathParseError { input: input.to_string() });
                };

                FocusScopeSegment::Visual { child_index }
            } else {
                return Err(FocusScopePathParseError { input: input.to_string() });
            };

            segments.push(segment);
        }

        if segments.is_empty() {
            return Err(FocusScopePathParseError { input: input.to_string() });
        }

        Ok(Self(segments))
    }
}

impl fmt::Display for FocusScopePathParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid focus scope path: {}", self.input)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusTreeWindowGeometry {
    pub window_id: WindowId,
    pub geometry: WindowGeometry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusBranchKey {
    Scope(FocusScopePath),
    Window(WindowId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusScopeNavigation {
    pub axis: FocusAxis,
    pub branches: Vec<FocusBranchKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FocusTree {
    ordered_window_ids: Vec<WindowId>,
    scope_path_by_window: BTreeMap<WindowId, Vec<FocusScopePath>>,
    descendant_window_ids_by_scope: BTreeMap<FocusScopePath, Vec<WindowId>>,
    navigation_by_scope: BTreeMap<FocusScopePath, FocusScopeNavigation>,
}

impl FocusTree {
    pub fn from_resolved_root(root: &ResolvedLayoutNode) -> Self {
        let mut tree = Self {
            descendant_window_ids_by_scope: BTreeMap::from([(Self::workspace_scope(), Vec::new())]),
            ..Self::default()
        };
        let mut scope_path = vec![Self::workspace_scope()];

        collect_focus_tree_children(root.children(), &mut scope_path, &mut tree);
        tree
    }

    pub fn from_window_geometries(entries: &[FocusTreeWindowGeometry]) -> Self {
        let mut tree = Self {
            descendant_window_ids_by_scope: BTreeMap::from([(Self::workspace_scope(), Vec::new())]),
            ..Self::default()
        };

        let visual_entries = entries
            .iter()
            .enumerate()
            .map(|(original_index, entry)| VisualEntry {
                window_id: entry.window_id.clone(),
                geometry: entry.geometry,
                original_index,
            })
            .collect::<Vec<_>>();

        if visual_entries.is_empty() {
            return tree;
        }

        let root_scope = infer_visual_scope(&visual_entries);
        let mut scope_path = vec![Self::workspace_scope()];
        collect_visual_scope(
            &root_scope,
            &Self::workspace_scope(),
            &mut scope_path,
            &mut tree,
            true,
        );
        tree
    }

    pub fn workspace_scope() -> FocusScopePath {
        FocusScopePath::workspace()
    }

    pub fn workspace_scope_key() -> &'static str {
        "$workspace"
    }

    pub fn ordered_window_ids(&self) -> &[WindowId] {
        &self.ordered_window_ids
    }

    pub fn scope_path(&self, window_id: &WindowId) -> Option<&[FocusScopePath]> {
        self.scope_path_by_window.get(window_id).map(Vec::as_slice)
    }

    pub fn descendants(&self, scope_key: &FocusScopePath) -> Option<&[WindowId]> {
        self.descendant_window_ids_by_scope.get(scope_key).map(Vec::as_slice)
    }

    pub fn scope_keys(&self) -> impl Iterator<Item = &FocusScopePath> {
        self.descendant_window_ids_by_scope.keys()
    }

    pub fn navigation(&self, scope_key: &FocusScopePath) -> Option<&FocusScopeNavigation> {
        self.navigation_by_scope.get(scope_key)
    }

    pub fn set_navigation(
        &mut self,
        navigation_by_scope: BTreeMap<FocusScopePath, FocusScopeNavigation>,
    ) {
        self.navigation_by_scope = navigation_by_scope;
    }

    pub fn contains_window(&self, window_id: &WindowId) -> bool {
        self.scope_path_by_window.contains_key(window_id)
    }
}

fn collect_visual_scope(
    scope: &VisualScope,
    current_scope_key: &FocusScopePath,
    scope_path: &mut Vec<FocusScopePath>,
    tree: &mut FocusTree,
    is_root: bool,
) {
    if let Some(axis) = scope.axis
        && scope.children.len() > 1
    {
        let branches = scope
            .children
            .iter()
            .enumerate()
            .map(|(child_index, child)| match child {
                VisualChild::Scope(_) => {
                    FocusBranchKey::Scope(current_scope_key.child_visual(child_index))
                }
                VisualChild::Window(entry) => FocusBranchKey::Window(entry.window_id.clone()),
            })
            .collect::<Vec<_>>();

        tree.navigation_by_scope
            .insert(current_scope_key.clone(), FocusScopeNavigation { axis, branches });
    }

    for (child_index, child) in scope.children.iter().enumerate() {
        match child {
            VisualChild::Scope(child_scope) => {
                let child_scope_key = current_scope_key.child_visual(child_index);
                tree.descendant_window_ids_by_scope.entry(child_scope_key.clone()).or_default();
                scope_path.push(child_scope_key.clone());
                collect_visual_scope(child_scope, &child_scope_key, scope_path, tree, false);
                scope_path.pop();
            }
            VisualChild::Window(entry) => {
                tree.ordered_window_ids.push(entry.window_id.clone());
                tree.scope_path_by_window.insert(entry.window_id.clone(), scope_path.clone());

                for scope_key in scope_path.iter() {
                    tree.descendant_window_ids_by_scope
                        .entry(scope_key.clone())
                        .or_default()
                        .push(entry.window_id.clone());
                }
            }
        }
    }

    if is_root && scope.children.is_empty() {
        tree.descendant_window_ids_by_scope.entry(current_scope_key.clone()).or_default();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusUpdate {
    Unchanged,
    Set(Option<WindowId>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusSelection {
    pub focused_window_id: Option<WindowId>,
}

pub fn set_focused_window(model: &mut WmModel, focused_id: Option<WindowId>) -> Option<WindowId> {
    let focused_id = focused_id.filter(|window_id| model.windows.contains_key(window_id));
    model.set_window_focused(focused_id.clone());
    focused_id
}

pub fn focus_next_window<I>(model: &mut WmModel, hinted_window_ids: I) -> Option<WindowId>
where
    I: IntoIterator<Item = WindowId>,
{
    let focusable_window_ids =
        model.ordered_focusable_window_ids_on_current_workspace(hinted_window_ids);
    let next_focus = next_focus_in_order(&focusable_window_ids, model.focused_window_id.clone());

    set_focused_window(model, next_focus)
}

pub fn focus_previous_window<I>(model: &mut WmModel, hinted_window_ids: I) -> Option<WindowId>
where
    I: IntoIterator<Item = WindowId>,
{
    let focusable_window_ids =
        model.ordered_focusable_window_ids_on_current_workspace(hinted_window_ids);
    let previous_focus =
        previous_focus_in_order(&focusable_window_ids, model.focused_window_id.clone());

    set_focused_window(model, previous_focus)
}

pub fn request_focus_window(model: &mut WmModel, window_id: Option<WindowId>) -> FocusSelection {
    FocusSelection { focused_window_id: set_focused_window(model, window_id) }
}

pub fn request_focus_next_window<I>(model: &mut WmModel, hinted_window_ids: I) -> FocusSelection
where
    I: IntoIterator<Item = WindowId>,
{
    FocusSelection { focused_window_id: focus_next_window(model, hinted_window_ids) }
}

pub fn request_focus_previous_window<I>(model: &mut WmModel, hinted_window_ids: I) -> FocusSelection
where
    I: IntoIterator<Item = WindowId>,
{
    FocusSelection { focused_window_id: focus_previous_window(model, hinted_window_ids) }
}

pub fn remove_window<I>(
    model: &mut WmModel,
    removed_id: WindowId,
    hinted_window_ids: I,
) -> FocusUpdate
where
    I: IntoIterator<Item = WindowId>,
{
    let removed_was_focused = model.focused_window_id.as_ref() == Some(&removed_id);
    let hinted_window_ids = hinted_window_ids.into_iter().collect::<Vec<_>>();
    let next_focus = removed_was_focused
        .then(|| {
            preferred_focus_after_focus_loss(model, &removed_id, hinted_window_ids.iter().cloned())
        })
        .flatten();

    model.remove_window(removed_id);

    if !removed_was_focused {
        return FocusUpdate::Unchanged;
    }

    let next_focus = set_focused_window(model, next_focus);
    FocusUpdate::Set(next_focus)
}

pub fn unmap_window<I>(
    model: &mut WmModel,
    unmapped_id: WindowId,
    hinted_window_ids: I,
) -> FocusUpdate
where
    I: IntoIterator<Item = WindowId>,
{
    if !model.windows.contains_key(&unmapped_id) {
        return FocusUpdate::Unchanged;
    }

    let unmapped_was_focused = model.focused_window_id.as_ref() == Some(&unmapped_id);
    let hinted_window_ids = hinted_window_ids.into_iter().collect::<Vec<_>>();
    model.set_window_mapped(unmapped_id.clone(), false);

    if !unmapped_was_focused {
        return FocusUpdate::Unchanged;
    }

    let next_focus = preferred_focus_after_focus_loss(model, &unmapped_id, hinted_window_ids);
    let next_focus = set_focused_window(model, next_focus);
    FocusUpdate::Set(next_focus)
}

fn collect_focus_tree_children(
    children: &[ResolvedLayoutNode],
    scope_path: &mut Vec<FocusScopePath>,
    tree: &mut FocusTree,
) {
    for (child_index, child) in children.iter().enumerate() {
        collect_focus_tree_node(child, child_index, scope_path, tree);
    }
}

fn collect_focus_tree_node(
    node: &ResolvedLayoutNode,
    child_index: usize,
    scope_path: &mut Vec<FocusScopePath>,
    tree: &mut FocusTree,
) {
    match node {
        ResolvedLayoutNode::Workspace { children, .. } => {
            collect_focus_tree_children(children, scope_path, tree);
        }
        ResolvedLayoutNode::Group { meta, children, .. } => {
            let parent_scope =
                scope_path.last().cloned().unwrap_or_else(FocusTree::workspace_scope);
            let scope_key = parent_scope.child_group(meta, child_index);
            tree.descendant_window_ids_by_scope.entry(scope_key.clone()).or_default();
            scope_path.push(scope_key);
            collect_focus_tree_children(children, scope_path, tree);
            scope_path.pop();
        }
        ResolvedLayoutNode::Window { window_id: Some(window_id), .. } => {
            tree.ordered_window_ids.push(window_id.clone());
            tree.scope_path_by_window.insert(window_id.clone(), scope_path.clone());

            for scope_key in scope_path.iter() {
                tree.descendant_window_ids_by_scope
                    .entry(scope_key.clone())
                    .or_default()
                    .push(window_id.clone());
            }
        }
        ResolvedLayoutNode::Content { children, .. } => {
            collect_focus_tree_children(children, scope_path, tree);
        }
        ResolvedLayoutNode::Window { window_id: None, .. } => {}
    }
}

fn next_focus_in_order(
    ordered_window_ids: &[WindowId],
    current_window_id: Option<WindowId>,
) -> Option<WindowId> {
    match current_window_id.filter(|window_id| ordered_window_ids.contains(window_id)).and_then(
        |current_window_id| {
            ordered_window_ids.iter().position(|window_id| *window_id == current_window_id)
        },
    ) {
        Some(current_index) if !ordered_window_ids.is_empty() => {
            ordered_window_ids.get((current_index + 1) % ordered_window_ids.len()).cloned()
        }
        _ => ordered_window_ids.first().cloned(),
    }
}

fn previous_focus_in_order(
    ordered_window_ids: &[WindowId],
    current_window_id: Option<WindowId>,
) -> Option<WindowId> {
    match current_window_id.filter(|window_id| ordered_window_ids.contains(window_id)).and_then(
        |current_window_id| {
            ordered_window_ids.iter().position(|window_id| *window_id == current_window_id)
        },
    ) {
        Some(current_index) if !ordered_window_ids.is_empty() => ordered_window_ids
            .get((current_index + ordered_window_ids.len() - 1) % ordered_window_ids.len())
            .cloned(),
        _ => ordered_window_ids.last().cloned(),
    }
}

fn preferred_focus_after_focus_loss<I>(
    model: &WmModel,
    lost_window_id: &WindowId,
    hinted_window_ids: I,
) -> Option<WindowId>
where
    I: IntoIterator<Item = WindowId>,
{
    if let Some(scope_path) = model.focus_scope_path(lost_window_id) {
        for scope_key in scope_path.iter().rev() {
            if let Some(candidate) = preferred_focus_for_scope(model, scope_key, lost_window_id) {
                return Some(candidate);
            }
        }
    }

    let ordered_window_ids = model.ordered_window_ids_on_current_workspace(hinted_window_ids);
    let workspace_scope = FocusTree::workspace_scope();
    preferred_focus_from_order(model, &workspace_scope, &ordered_window_ids, lost_window_id)
}

fn preferred_focus_for_scope(
    model: &WmModel,
    scope_key: &FocusScopePath,
    lost_window_id: &WindowId,
) -> Option<WindowId> {
    let ordered_window_ids = model.focus_scope_descendants(scope_key)?;
    preferred_focus_from_order(model, scope_key, ordered_window_ids, lost_window_id)
}

fn preferred_focus_from_order(
    model: &WmModel,
    scope_key: &FocusScopePath,
    ordered_window_ids: &[WindowId],
    lost_window_id: &WindowId,
) -> Option<WindowId> {
    let focusable_candidates = ordered_window_ids
        .iter()
        .filter(|window_id| {
            *window_id != lost_window_id && model.window_is_focus_cycle_candidate(window_id)
        })
        .cloned()
        .collect::<Vec<_>>();

    if focusable_candidates.is_empty() {
        return None;
    }

    if let Some(remembered_window_id) = model.remembered_focus_for_scope(scope_key)
        && remembered_window_id != lost_window_id
        && focusable_candidates.contains(remembered_window_id)
    {
        return Some(remembered_window_id.clone());
    }

    if let Some(lost_index) =
        ordered_window_ids.iter().position(|window_id| window_id == lost_window_id)
    {
        for window_id in ordered_window_ids
            .iter()
            .skip(lost_index + 1)
            .chain(ordered_window_ids.iter().take(lost_index))
        {
            if window_id != lost_window_id && model.window_is_focus_cycle_candidate(window_id) {
                return Some(window_id.clone());
            }
        }
    }

    focusable_candidates.last().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LayoutNodeMeta, ResolvedLayoutNode, WorkspaceId, window_id};

    #[test]
    fn focus_scope_path_round_trips_through_string_format() {
        let path = FocusTree::workspace_scope().child_visual(1).child_group(
            &LayoutNodeMeta { id: Some("bottom-row".into()), ..LayoutNodeMeta::default() },
            2,
        );

        let encoded = path.to_string();
        let decoded = encoded.parse::<FocusScopePath>().expect("valid focus scope path");

        assert_eq!(decoded, path);
        assert_eq!(encoded, "$workspace/visual[1]/group[2]:bottom-row");
    }

    fn flat_root(window_ids: &[u64]) -> ResolvedLayoutNode {
        ResolvedLayoutNode::Workspace {
            meta: LayoutNodeMeta::default(),
            children: window_ids
                .iter()
                .map(|id| ResolvedLayoutNode::Window {
                    meta: LayoutNodeMeta::default(),
                    window_id: Some(window_id(*id)),
                    children: vec![],
                })
                .collect(),
        }
    }

    fn grouped_root() -> ResolvedLayoutNode {
        ResolvedLayoutNode::Workspace {
            meta: LayoutNodeMeta::default(),
            children: vec![
                ResolvedLayoutNode::Group {
                    meta: LayoutNodeMeta { id: Some("main".into()), ..LayoutNodeMeta::default() },
                    children: vec![
                        ResolvedLayoutNode::Window {
                            meta: LayoutNodeMeta::default(),
                            window_id: Some(window_id(1)),
                            children: vec![],
                        },
                        ResolvedLayoutNode::Window {
                            meta: LayoutNodeMeta::default(),
                            window_id: Some(window_id(2)),
                            children: vec![],
                        },
                    ],
                },
                ResolvedLayoutNode::Group {
                    meta: LayoutNodeMeta { id: Some("stack".into()), ..LayoutNodeMeta::default() },
                    children: vec![ResolvedLayoutNode::Window {
                        meta: LayoutNodeMeta::default(),
                        window_id: Some(window_id(3)),
                        children: vec![],
                    }],
                },
            ],
        }
    }

    #[test]
    fn focusing_unknown_window_clears_focus() {
        let mut model = WmModel::default();
        model.insert_window(window_id(1), None, None);
        model.set_window_focused(Some(window_id(1)));

        let resolved = set_focused_window(&mut model, Some(window_id(99)));

        assert_eq!(resolved, None);
        assert_eq!(model.focused_window_id, None);
        assert_eq!(model.windows.get(&window_id(1)).map(|window| window.focused), Some(false));
    }

    #[test]
    fn focusing_known_window_marks_only_that_window_focused() {
        let mut model = WmModel::default();
        model.insert_window(window_id(1), None, None);
        model.insert_window(window_id(2), None, None);

        let resolved = set_focused_window(&mut model, Some(window_id(2)));

        assert_eq!(resolved, Some(window_id(2)));
        assert_eq!(model.focused_window_id, Some(window_id(2)));
        assert_eq!(model.windows.get(&window_id(1)).map(|window| window.focused), Some(false));
        assert_eq!(model.windows.get(&window_id(2)).map(|window| window.focused), Some(true));
    }

    #[test]
    fn removing_non_focused_window_keeps_existing_focus() {
        let mut model = WmModel::default();
        model.insert_window(window_id(1), None, None);
        model.insert_window(window_id(2), None, None);
        model.set_window_focused(Some(window_id(2)));

        let update = remove_window(&mut model, window_id(1), Vec::new());

        assert_eq!(update, FocusUpdate::Unchanged);
        assert_eq!(model.focused_window_id, Some(window_id(2)));
        assert!(!model.windows.contains_key(&window_id(1)));
    }

    #[test]
    fn removing_focused_window_selects_latest_remaining_window() {
        let mut model = WmModel::default();
        model.insert_window(window_id(1), None, None);
        model.insert_window(window_id(2), None, None);
        model.insert_window(window_id(3), None, None);
        model.set_current_workspace(WorkspaceId::from("1"));
        model.set_window_workspace(window_id(1), Some(WorkspaceId::from("1")));
        model.set_window_workspace(window_id(2), Some(WorkspaceId::from("1")));
        model.set_window_workspace(window_id(3), Some(WorkspaceId::from("1")));
        model.set_window_mapped(window_id(1), true);
        model.set_window_mapped(window_id(2), true);
        model.set_window_mapped(window_id(3), true);
        model.set_focus_tree(Some(&flat_root(&[1, 2, 3])));
        model.set_window_focused(Some(window_id(2)));

        let update = remove_window(&mut model, window_id(2), Vec::new());

        assert_eq!(update, FocusUpdate::Set(Some(window_id(3))));
        assert_eq!(model.focused_window_id, Some(window_id(3)));
        assert_eq!(model.windows.get(&window_id(1)).map(|window| window.focused), Some(false));
        assert_eq!(model.windows.get(&window_id(3)).map(|window| window.focused), Some(true));
    }

    #[test]
    fn removing_last_focused_window_clears_focus() {
        let mut model = WmModel::default();
        model.insert_window(window_id(4), None, None);
        model.set_window_focused(Some(window_id(4)));

        let update = remove_window(&mut model, window_id(4), Vec::new());

        assert_eq!(update, FocusUpdate::Set(None));
        assert_eq!(model.focused_window_id, None);
        assert!(model.windows.is_empty());
    }

    #[test]
    fn focusing_next_window_advances_and_wraps() {
        let mut model = WmModel::default();
        model.insert_window(window_id(1), None, None);
        model.insert_window(window_id(3), None, None);
        model.insert_window(window_id(8), None, None);
        model.set_current_workspace(WorkspaceId::from("1"));
        model.set_window_workspace(window_id(1), Some(WorkspaceId::from("1")));
        model.set_window_workspace(window_id(3), Some(WorkspaceId::from("1")));
        model.set_window_workspace(window_id(8), Some(WorkspaceId::from("1")));
        model.set_window_mapped(window_id(1), true);
        model.set_window_mapped(window_id(3), true);
        model.set_window_mapped(window_id(8), true);
        model.set_focus_tree(Some(&flat_root(&[3, 1, 8])));
        model.set_window_focused(Some(window_id(3)));

        let next = focus_next_window(&mut model, Vec::new());
        assert_eq!(next, Some(window_id(1)));

        let wrapped = focus_next_window(&mut model, Vec::new());
        assert_eq!(wrapped, Some(window_id(8)));
    }

    #[test]
    fn focusing_previous_window_rewinds_and_wraps() {
        let mut model = WmModel::default();
        model.insert_window(window_id(1), None, None);
        model.insert_window(window_id(3), None, None);
        model.insert_window(window_id(8), None, None);
        model.set_current_workspace(WorkspaceId::from("1"));
        model.set_window_workspace(window_id(1), Some(WorkspaceId::from("1")));
        model.set_window_workspace(window_id(3), Some(WorkspaceId::from("1")));
        model.set_window_workspace(window_id(8), Some(WorkspaceId::from("1")));
        model.set_window_mapped(window_id(1), true);
        model.set_window_mapped(window_id(3), true);
        model.set_window_mapped(window_id(8), true);
        model.set_focus_tree(Some(&flat_root(&[3, 1, 8])));
        model.set_window_focused(Some(window_id(3)));

        let previous = focus_previous_window(&mut model, Vec::new());
        assert_eq!(previous, Some(window_id(8)));

        let wrapped = focus_previous_window(&mut model, Vec::new());
        assert_eq!(wrapped, Some(window_id(1)));
    }

    #[test]
    fn request_focus_window_returns_selection() {
        let mut model = WmModel::default();
        model.insert_window(window_id(2), None, None);

        let selection = request_focus_window(&mut model, Some(window_id(2)));

        assert_eq!(selection, FocusSelection { focused_window_id: Some(window_id(2)) });
    }

    #[test]
    fn request_focus_next_window_returns_selection() {
        let mut model = WmModel::default();
        model.insert_window(window_id(2), None, None);
        model.set_current_workspace(WorkspaceId::from("1"));
        model.set_window_workspace(window_id(2), Some(WorkspaceId::from("1")));
        model.set_window_mapped(window_id(2), true);
        model.set_focus_tree(Some(&flat_root(&[2])));

        let selection = request_focus_next_window(&mut model, Vec::new());

        assert_eq!(selection, FocusSelection { focused_window_id: Some(window_id(2)) });
    }

    #[test]
    fn request_focus_previous_window_returns_selection() {
        let mut model = WmModel::default();
        model.insert_window(window_id(2), None, None);
        model.set_current_workspace(WorkspaceId::from("1"));
        model.set_window_workspace(window_id(2), Some(WorkspaceId::from("1")));
        model.set_window_mapped(window_id(2), true);
        model.set_focus_tree(Some(&flat_root(&[2])));

        let selection = request_focus_previous_window(&mut model, Vec::new());

        assert_eq!(selection, FocusSelection { focused_window_id: Some(window_id(2)) });
    }

    #[test]
    fn focusing_next_window_skips_unknown_focus() {
        let mut model = WmModel::default();
        model.insert_window(window_id(1), None, None);
        model.insert_window(window_id(2), None, None);
        model.set_current_workspace(WorkspaceId::from("1"));
        model.set_window_workspace(window_id(1), Some(WorkspaceId::from("1")));
        model.set_window_workspace(window_id(2), Some(WorkspaceId::from("1")));
        model.set_window_mapped(window_id(1), true);
        model.set_window_mapped(window_id(2), true);
        model.set_focus_tree(Some(&flat_root(&[1, 2])));
        model.set_window_focused(Some(window_id(99)));

        let next = focus_next_window(&mut model, Vec::new());

        assert_eq!(next, Some(window_id(1)));
    }

    #[test]
    fn unmapping_focused_window_selects_previous_mapped_candidate() {
        let mut model = WmModel::default();
        model.insert_window(window_id(1), None, None);
        model.insert_window(window_id(2), None, None);
        model.set_current_workspace(WorkspaceId::from("1"));
        model.set_window_workspace(window_id(1), Some(WorkspaceId::from("1")));
        model.set_window_workspace(window_id(2), Some(WorkspaceId::from("1")));
        model.set_window_mapped(window_id(1), true);
        model.set_window_mapped(window_id(2), true);
        model.set_focus_tree(Some(&flat_root(&[1, 2])));
        model.set_window_focused(Some(window_id(2)));

        let update = unmap_window(&mut model, window_id(2), Vec::new());

        assert_eq!(update, FocusUpdate::Set(Some(window_id(1))));
        assert_eq!(model.windows.get(&window_id(2)).map(|window| window.mapped), Some(false));
    }

    #[test]
    fn removing_focused_window_prefers_same_group_memory_before_workspace_fallback() {
        let mut model = WmModel::default();
        let workspace_id = WorkspaceId::from("1");

        for id in [1, 2, 3] {
            model.insert_window(window_id(id), Some(workspace_id.clone()), None);
            model.set_window_mapped(window_id(id), true);
        }

        model.set_current_workspace(workspace_id);
        model.set_focus_tree(Some(&grouped_root()));
        model.set_window_focused(Some(window_id(1)));
        model.set_window_focused(Some(window_id(3)));
        model.set_window_focused(Some(window_id(2)));

        let update = remove_window(&mut model, window_id(2), Vec::new());

        assert_eq!(update, FocusUpdate::Set(Some(window_id(1))));
        assert_eq!(model.focused_window_id, Some(window_id(1)));
    }
}
