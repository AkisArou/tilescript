use std::collections::BTreeMap;

use crate::WindowId;
use crate::focus::{FocusAxis, FocusBranchKey, FocusScopePath, FocusTree};
use crate::wm::WindowGeometry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowGeometryCandidate {
    pub window_id: WindowId,
    pub geometry: WindowGeometry,
    pub scope_path: Vec<FocusScopePath>,
}

pub fn select_directional_focus_candidate(
    candidates: &[WindowGeometryCandidate],
    current_focused_window_id: Option<WindowId>,
    direction: NavigationDirection,
    remembered_focus_by_scope: &BTreeMap<FocusScopePath, WindowId>,
    focus_tree: Option<&FocusTree>,
) -> Option<WindowId> {
    let current = current_focused_window_id.and_then(|window_id| {
        candidates.iter().find(|candidate| candidate.window_id == window_id)
    })?;

    if let Some(focus_tree) = focus_tree
        && let Some(window_id) = select_directional_focus_candidate_from_tree(
            current,
            direction,
            remembered_focus_by_scope,
            focus_tree,
        )
    {
        return Some(window_id);
    }

    for scope_depth in (0..current.scope_path.len()).rev() {
        let scope_key = &current.scope_path[scope_depth];
        let mut branches = scope_branches(candidates, scope_key, scope_depth);
        let Some(axis) = infer_split_axis(&branches) else {
            continue;
        };

        if !direction_matches_axis(direction, axis) || branches.len() < 2 {
            continue;
        }

        sort_scope_branches(&mut branches, axis);
        let current_branch = current_branch_key(current, scope_depth);
        let Some(current_index) = branches.iter().position(|branch| branch.key == current_branch)
        else {
            continue;
        };

        let Some(target_index) = wrapped_branch_index(current_index, branches.len(), direction)
        else {
            continue;
        };

        let Some(target_branch) = branches.get(target_index) else {
            continue;
        };

        if let Some(window_id) =
            resolve_branch_target(candidates, target_branch, direction, remembered_focus_by_scope)
        {
            return Some(window_id);
        }
    }

    select_geometric_candidate(candidates, current, direction)
}

pub fn managed_window_swap_positions(
    window_order: &[WindowId],
    first_window_id: WindowId,
    second_window_id: WindowId,
) -> Option<(usize, usize)> {
    let first_index = window_order.iter().position(|window_id| *window_id == first_window_id)?;
    let second_index = window_order.iter().position(|window_id| *window_id == second_window_id)?;
    Some((first_index, second_index))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SplitAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone)]
struct ScopeBranch<'a> {
    key: FocusBranchKey,
    geometry: WindowGeometry,
    descendants: Vec<&'a WindowGeometryCandidate>,
    scope_depth: Option<usize>,
}

fn select_directional_focus_candidate_from_tree(
    current: &WindowGeometryCandidate,
    direction: NavigationDirection,
    remembered_focus_by_scope: &BTreeMap<FocusScopePath, WindowId>,
    focus_tree: &FocusTree,
) -> Option<WindowId> {
    let scope_path = focus_tree.scope_path(&current.window_id)?;
    let mut wrap_candidate = None;

    for scope_depth in (0..scope_path.len()).rev() {
        let scope_key = &scope_path[scope_depth];
        let Some(navigation) = focus_tree.navigation(scope_key) else {
            continue;
        };

        if !direction_matches_focus_axis(direction, navigation.axis)
            || navigation.branches.len() < 2
        {
            continue;
        }

        let current_branch = current_branch_key(current, scope_depth);
        let Some(current_index) =
            navigation.branches.iter().position(|branch| branch == &current_branch)
        else {
            continue;
        };

        if let Some(target_index) =
            adjacent_branch_index(current_index, navigation.branches.len(), direction)
            && let Some(target_branch) = navigation.branches.get(target_index)
            && let Some(window_id) = resolve_tree_branch_target(
                focus_tree,
                target_branch,
                direction,
                remembered_focus_by_scope,
            )
        {
            return Some(window_id);
        }

        wrap_candidate = Some((scope_key.clone(), current_index, navigation.branches.len()));
    }

    let Some((scope_key, current_index, branch_count)) = wrap_candidate else {
        return None;
    };
    let navigation = focus_tree.navigation(&scope_key)?;
    let target_index = wrapped_branch_index(current_index, branch_count, direction)?;
    let target_branch = navigation.branches.get(target_index)?;

    resolve_tree_branch_target(focus_tree, target_branch, direction, remembered_focus_by_scope)
}

fn adjacent_branch_index(
    current_index: usize,
    branch_count: usize,
    direction: NavigationDirection,
) -> Option<usize> {
    if branch_count < 2 {
        return None;
    }

    match direction {
        NavigationDirection::Left | NavigationDirection::Up => current_index.checked_sub(1),
        NavigationDirection::Right | NavigationDirection::Down => {
            (current_index + 1 < branch_count).then_some(current_index + 1)
        }
    }
}

fn scope_branches<'a>(
    candidates: &'a [WindowGeometryCandidate],
    scope_key: &FocusScopePath,
    scope_depth: usize,
) -> Vec<ScopeBranch<'a>> {
    let mut branches: Vec<ScopeBranch<'a>> = Vec::new();

    for candidate in candidates.iter().filter(|candidate| {
        candidate
            .scope_path
            .get(scope_depth)
            .is_some_and(|candidate_scope| candidate_scope == scope_key)
    }) {
        let key = if candidate.scope_path.len() > scope_depth + 1 {
            FocusBranchKey::Scope(candidate.scope_path[scope_depth + 1].clone())
        } else {
            FocusBranchKey::Window(candidate.window_id.clone())
        };

        if let Some(branch) = branches.iter_mut().find(|branch| branch.key == key) {
            branch.geometry = union_geometry(branch.geometry, candidate.geometry);
            branch.descendants.push(candidate);
            continue;
        }

        branches.push(ScopeBranch {
            scope_depth: match &key {
                FocusBranchKey::Scope(_) => Some(scope_depth + 1),
                FocusBranchKey::Window(_) => None,
            },
            key,
            geometry: candidate.geometry,
            descendants: vec![candidate],
        });
    }

    branches
}

fn infer_split_axis(branches: &[ScopeBranch<'_>]) -> Option<SplitAxis> {
    if branches.len() < 2 {
        return None;
    }

    let mut min_center_x = i32::MAX;
    let mut max_center_x = i32::MIN;
    let mut min_center_y = i32::MAX;
    let mut max_center_y = i32::MIN;

    for branch in branches {
        let center = rect_center(branch.geometry);
        min_center_x = min_center_x.min(center.0);
        max_center_x = max_center_x.max(center.0);
        min_center_y = min_center_y.min(center.1);
        max_center_y = max_center_y.max(center.1);
    }

    let x_span = max_center_x - min_center_x;
    let y_span = max_center_y - min_center_y;

    if x_span == 0 && y_span == 0 {
        None
    } else if x_span >= y_span {
        Some(SplitAxis::Horizontal)
    } else {
        Some(SplitAxis::Vertical)
    }
}

fn direction_matches_axis(direction: NavigationDirection, axis: SplitAxis) -> bool {
    matches!(
        (direction, axis),
        (NavigationDirection::Left | NavigationDirection::Right, SplitAxis::Horizontal,)
            | (NavigationDirection::Up | NavigationDirection::Down, SplitAxis::Vertical,)
    )
}

fn sort_scope_branches(branches: &mut [ScopeBranch<'_>], axis: SplitAxis) {
    branches.sort_by_key(|branch| match axis {
        SplitAxis::Horizontal => (branch.geometry.x, branch.geometry.y),
        SplitAxis::Vertical => (branch.geometry.y, branch.geometry.x),
    });
}

fn current_branch_key(current: &WindowGeometryCandidate, scope_depth: usize) -> FocusBranchKey {
    if current.scope_path.len() > scope_depth + 1 {
        FocusBranchKey::Scope(current.scope_path[scope_depth + 1].clone())
    } else {
        FocusBranchKey::Window(current.window_id.clone())
    }
}

fn wrapped_branch_index(
    current_index: usize,
    branch_count: usize,
    direction: NavigationDirection,
) -> Option<usize> {
    if branch_count < 2 {
        return None;
    }

    Some(match direction {
        NavigationDirection::Left | NavigationDirection::Up => {
            current_index.checked_sub(1).unwrap_or(branch_count - 1)
        }
        NavigationDirection::Right | NavigationDirection::Down => {
            if current_index + 1 < branch_count { current_index + 1 } else { 0 }
        }
    })
}

fn direction_matches_focus_axis(direction: NavigationDirection, axis: FocusAxis) -> bool {
    matches!(
        (direction, axis),
        (NavigationDirection::Left | NavigationDirection::Right, FocusAxis::Horizontal,)
            | (NavigationDirection::Up | NavigationDirection::Down, FocusAxis::Vertical,)
    )
}

fn resolve_tree_branch_target(
    focus_tree: &FocusTree,
    branch: &FocusBranchKey,
    direction: NavigationDirection,
    remembered_focus_by_scope: &BTreeMap<FocusScopePath, WindowId>,
) -> Option<WindowId> {
    match branch {
        FocusBranchKey::Window(window_id) => Some(window_id.clone()),
        FocusBranchKey::Scope(scope_key) => {
            if let Some(remembered_window_id) = remembered_focus_by_scope.get(scope_key)
                && focus_tree
                    .descendants(scope_key)
                    .is_some_and(|descendants| descendants.contains(remembered_window_id))
            {
                return Some(remembered_window_id.clone());
            }

            default_focus_in_tree_scope(focus_tree, scope_key, direction, remembered_focus_by_scope)
        }
    }
}

fn default_focus_in_tree_scope(
    focus_tree: &FocusTree,
    scope_key: &FocusScopePath,
    direction: NavigationDirection,
    remembered_focus_by_scope: &BTreeMap<FocusScopePath, WindowId>,
) -> Option<WindowId> {
    let Some(navigation) = focus_tree.navigation(scope_key) else {
        let descendants = focus_tree.descendants(scope_key)?;
        return match direction {
            NavigationDirection::Left | NavigationDirection::Up => descendants.last().cloned(),
            NavigationDirection::Right | NavigationDirection::Down => descendants.first().cloned(),
        };
    };

    let branch = match direction {
        NavigationDirection::Left | NavigationDirection::Up => navigation.branches.last(),
        NavigationDirection::Right | NavigationDirection::Down => navigation.branches.first(),
    }?;

    resolve_tree_branch_target(focus_tree, branch, direction, remembered_focus_by_scope)
}

fn resolve_branch_target(
    candidates: &[WindowGeometryCandidate],
    branch: &ScopeBranch<'_>,
    direction: NavigationDirection,
    remembered_focus_by_scope: &BTreeMap<FocusScopePath, WindowId>,
) -> Option<WindowId> {
    match &branch.key {
        FocusBranchKey::Window(window_id) => Some(window_id.clone()),
        FocusBranchKey::Scope(scope_key) => {
            if let Some(remembered_window_id) = remembered_focus_by_scope.get(scope_key)
                && branch
                    .descendants
                    .iter()
                    .any(|candidate| candidate.window_id == *remembered_window_id)
            {
                return Some(remembered_window_id.clone());
            }

            default_focus_in_scope(
                candidates,
                scope_key,
                branch.scope_depth?,
                direction,
                remembered_focus_by_scope,
            )
        }
    }
}

fn default_focus_in_scope(
    candidates: &[WindowGeometryCandidate],
    scope_key: &FocusScopePath,
    scope_depth: usize,
    direction: NavigationDirection,
    remembered_focus_by_scope: &BTreeMap<FocusScopePath, WindowId>,
) -> Option<WindowId> {
    let mut branches = scope_branches(candidates, scope_key, scope_depth);
    if branches.is_empty() {
        return None;
    }

    branches.sort_by_key(|branch| match direction {
        NavigationDirection::Left | NavigationDirection::Right => {
            (branch.geometry.x, branch.geometry.y)
        }
        NavigationDirection::Up | NavigationDirection::Down => {
            (branch.geometry.y, branch.geometry.x)
        }
    });

    let branch = match direction {
        NavigationDirection::Left | NavigationDirection::Up => branches.last(),
        NavigationDirection::Right | NavigationDirection::Down => branches.first(),
    }?;

    resolve_branch_target(candidates, branch, direction, remembered_focus_by_scope)
}

fn select_geometric_candidate(
    candidates: &[WindowGeometryCandidate],
    current: &WindowGeometryCandidate,
    direction: NavigationDirection,
) -> Option<WindowId> {
    let current_center = rect_center(current.geometry);

    candidates
        .iter()
        .filter(|candidate| candidate.window_id != current.window_id)
        .filter_map(|candidate| {
            let candidate_center = rect_center(candidate.geometry);
            directional_score(current_center, candidate_center, direction)
                .map(|score| (score, candidate.window_id.clone()))
        })
        .min_by_key(|(score, _)| *score)
        .map(|(_, window_id)| window_id)
}

fn directional_score(
    current_center: (i32, i32),
    candidate_center: (i32, i32),
    direction: NavigationDirection,
) -> Option<(i32, i32, i32)> {
    let dx = candidate_center.0 - current_center.0;
    let dy = candidate_center.1 - current_center.1;
    let total_distance = dx.abs() + dy.abs();

    match direction {
        NavigationDirection::Left if dx < 0 => Some((total_distance, dy.abs(), -dx)),
        NavigationDirection::Right if dx > 0 => Some((total_distance, dy.abs(), dx)),
        NavigationDirection::Up if dy < 0 => Some((total_distance, dx.abs(), -dy)),
        NavigationDirection::Down if dy > 0 => Some((total_distance, dx.abs(), dy)),
        _ => None,
    }
}

fn rect_center(rect: WindowGeometry) -> (i32, i32) {
    (rect.x + rect.width / 2, rect.y + rect.height / 2)
}

fn union_geometry(left: WindowGeometry, right: WindowGeometry) -> WindowGeometry {
    let x1 = left.x.min(right.x);
    let y1 = left.y.min(right.y);
    let x2 = (left.x + left.width).max(right.x + right.width);
    let y2 = (left.y + left.height).max(right.y + right.height);

    WindowGeometry { x: x1, y: y1, width: x2 - x1, height: y2 - y1 }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::LayoutNodeMeta;
    use crate::focus::{
        FocusAxis, FocusBranchKey, FocusScopeNavigation, FocusScopePath, FocusTree,
    };
    use crate::window_id;

    fn scope_path(scope_path: &[&str]) -> Vec<FocusScopePath> {
        let mut resolved = Vec::new();
        let mut current = FocusTree::workspace_scope();

        for (depth, scope) in scope_path.iter().enumerate() {
            let next = if depth == 0 && *scope == FocusTree::workspace_scope_key() {
                FocusTree::workspace_scope()
            } else if scope.starts_with(FocusTree::workspace_scope_key()) {
                scope.parse().expect("valid focus scope path")
            } else {
                current.child_group(
                    &LayoutNodeMeta { id: Some((*scope).to_string()), ..LayoutNodeMeta::default() },
                    depth,
                )
            };

            current = next.clone();
            resolved.push(next);
        }

        resolved
    }

    fn remember(
        remembered: &mut BTreeMap<FocusScopePath, WindowId>,
        scopes: &[&str],
        window_id: WindowId,
    ) {
        for scope_key in scope_path(scopes) {
            remembered.insert(scope_key, window_id.clone());
        }
    }

    fn candidate(
        id: u64,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        scopes: &[&str],
    ) -> WindowGeometryCandidate {
        WindowGeometryCandidate {
            window_id: window_id(id),
            geometry: WindowGeometry { x, y, width, height },
            scope_path: scope_path(scopes),
        }
    }

    #[test]
    fn directional_focus_prefers_nearest_window_in_direction() {
        let candidates = vec![
            candidate(1, 0, 0, 100, 100, &["$workspace", "main"]),
            candidate(2, 140, 10, 100, 100, &["$workspace", "main"]),
            candidate(3, 320, 0, 100, 100, &["$workspace", "stack"]),
        ];

        assert_eq!(
            select_directional_focus_candidate(
                &candidates,
                Some(window_id(1)),
                NavigationDirection::Right,
                &BTreeMap::new(),
                None,
            ),
            Some(window_id(2))
        );
    }

    #[test]
    fn directional_focus_cycles_within_requested_axis() {
        let candidates = vec![
            candidate(1, 120, 120, 100, 100, &["$workspace", "main"]),
            candidate(2, 120, 0, 100, 100, &["$workspace", "main"]),
            candidate(3, 260, 120, 100, 100, &["$workspace", "stack"]),
        ];

        assert_eq!(
            select_directional_focus_candidate(
                &candidates,
                Some(window_id(1)),
                NavigationDirection::Up,
                &BTreeMap::new(),
                None,
            ),
            Some(window_id(2))
        );
        assert_eq!(
            select_directional_focus_candidate(
                &candidates,
                Some(window_id(1)),
                NavigationDirection::Left,
                &BTreeMap::new(),
                None,
            ),
            Some(window_id(3))
        );
    }

    #[test]
    fn directional_focus_prefers_lower_cross_axis_offset() {
        let candidates = vec![
            candidate(1, 100, 100, 100, 100, &["$workspace", "main"]),
            candidate(2, 260, 90, 100, 100, &["$workspace", "main"]),
            candidate(3, 250, 220, 100, 100, &["$workspace", "stack"]),
        ];

        assert_eq!(
            select_directional_focus_candidate(
                &candidates,
                Some(window_id(1)),
                NavigationDirection::Right,
                &BTreeMap::new(),
                None,
            ),
            Some(window_id(2))
        );
    }

    #[test]
    fn directional_focus_prefers_same_group_before_climbing() {
        let candidates = vec![
            candidate(1, 100, 100, 100, 100, &["$workspace", "main"]),
            candidate(2, 280, 105, 100, 100, &["$workspace", "main"]),
            candidate(3, 220, 100, 100, 100, &["$workspace", "stack"]),
        ];

        assert_eq!(
            select_directional_focus_candidate(
                &candidates,
                Some(window_id(1)),
                NavigationDirection::Right,
                &BTreeMap::new(),
                None,
            ),
            Some(window_id(2))
        );
    }

    #[test]
    fn directional_focus_climbs_to_parent_scope_when_group_has_no_match() {
        let candidates = vec![
            candidate(1, 100, 100, 100, 100, &["$workspace", "main"]),
            candidate(2, 100, 260, 100, 100, &["$workspace", "main"]),
            candidate(3, 260, 100, 100, 100, &["$workspace", "stack"]),
        ];

        assert_eq!(
            select_directional_focus_candidate(
                &candidates,
                Some(window_id(1)),
                NavigationDirection::Right,
                &BTreeMap::new(),
                None,
            ),
            Some(window_id(3))
        );
    }

    #[test]
    fn directional_focus_descends_into_remembered_nested_branch() {
        let candidates = vec![
            candidate(1, 0, 0, 100, 400, &["$workspace"]),
            candidate(2, 100, 0, 100, 200, &["$workspace", "right"]),
            candidate(3, 100, 200, 50, 200, &["$workspace", "right", "bottom"]),
            candidate(4, 150, 200, 50, 200, &["$workspace", "right", "bottom"]),
        ];
        let mut remembered = BTreeMap::new();

        assert_eq!(
            select_directional_focus_candidate(
                &candidates,
                Some(window_id(1)),
                NavigationDirection::Right,
                &remembered,
                None,
            ),
            Some(window_id(2))
        );

        remember(&mut remembered, &["$workspace", "right"], window_id(2));

        assert_eq!(
            select_directional_focus_candidate(
                &candidates,
                Some(window_id(2)),
                NavigationDirection::Down,
                &remembered,
                None,
            ),
            Some(window_id(3))
        );

        remember(&mut remembered, &["$workspace", "right", "bottom"], window_id(4));

        assert_eq!(
            select_directional_focus_candidate(
                &candidates,
                Some(window_id(2)),
                NavigationDirection::Down,
                &remembered,
                None,
            ),
            Some(window_id(4))
        );

        remember(&mut remembered, &["$workspace", "right", "bottom"], window_id(3));

        assert_eq!(
            select_directional_focus_candidate(
                &candidates,
                Some(window_id(1)),
                NavigationDirection::Right,
                &remembered,
                None,
            ),
            Some(window_id(3))
        );
    }

    #[test]
    fn directional_focus_replays_sway_memory_sequence() {
        fn step(
            candidates: &[WindowGeometryCandidate],
            focused: &mut WindowId,
            remembered: &mut BTreeMap<FocusScopePath, WindowId>,
            focus_tree: &FocusTree,
            direction: NavigationDirection,
            expected: u64,
        ) {
            *focused = select_directional_focus_candidate(
                candidates,
                Some(focused.clone()),
                direction,
                remembered,
                Some(focus_tree),
            )
            .expect("directional target");

            let scope_path = candidates
                .iter()
                .find(|candidate| candidate.window_id == *focused)
                .expect("focused candidate")
                .scope_path
                .clone();

            for scope_key in scope_path {
                remembered.insert(scope_key, focused.clone());
            }

            assert_eq!(*focused, window_id(expected));
        }

        let candidates = vec![
            candidate(1, 0, 0, 600, 600, &["$workspace"]),
            candidate(2, 600, 0, 400, 300, &["$workspace", "$workspace/group[1]:right"]),
            candidate(
                3,
                600,
                300,
                200,
                300,
                &[
                    "$workspace",
                    "$workspace/group[1]:right",
                    "$workspace/group[1]:right/group[1]:bottom",
                ],
            ),
            candidate(
                4,
                800,
                300,
                200,
                300,
                &[
                    "$workspace",
                    "$workspace/group[1]:right",
                    "$workspace/group[1]:right/group[1]:bottom",
                ],
            ),
        ];
        let mut focus_tree = FocusTree::from_resolved_root(&crate::ResolvedLayoutNode::Workspace {
            meta: crate::LayoutNodeMeta::default(),
            children: vec![
                crate::ResolvedLayoutNode::Window {
                    meta: crate::LayoutNodeMeta::default(),
                    window_id: Some(window_id(1)),
                    children: vec![],
                },
                crate::ResolvedLayoutNode::Group {
                    meta: crate::LayoutNodeMeta {
                        id: Some("right".into()),
                        ..crate::LayoutNodeMeta::default()
                    },
                    children: vec![
                        crate::ResolvedLayoutNode::Window {
                            meta: crate::LayoutNodeMeta::default(),
                            window_id: Some(window_id(2)),
                            children: vec![],
                        },
                        crate::ResolvedLayoutNode::Group {
                            meta: crate::LayoutNodeMeta {
                                id: Some("bottom".into()),
                                ..crate::LayoutNodeMeta::default()
                            },
                            children: vec![
                                crate::ResolvedLayoutNode::Window {
                                    meta: crate::LayoutNodeMeta::default(),
                                    window_id: Some(window_id(3)),
                                    children: vec![],
                                },
                                crate::ResolvedLayoutNode::Window {
                                    meta: crate::LayoutNodeMeta::default(),
                                    window_id: Some(window_id(4)),
                                    children: vec![],
                                },
                            ],
                        },
                    ],
                },
            ],
        });
        focus_tree.set_navigation(
            [
                (
                    FocusTree::workspace_scope(),
                    FocusScopeNavigation {
                        axis: FocusAxis::Horizontal,
                        branches: vec![
                            FocusBranchKey::Window(window_id(1)),
                            FocusBranchKey::Scope(
                                "$workspace/group[1]:right"
                                    .parse()
                                    .expect("valid focus scope path"),
                            ),
                        ],
                    },
                ),
                (
                    "$workspace/group[1]:right".parse().expect("valid focus scope path"),
                    FocusScopeNavigation {
                        axis: FocusAxis::Vertical,
                        branches: vec![
                            FocusBranchKey::Window(window_id(2)),
                            FocusBranchKey::Scope(
                                "$workspace/group[1]:right/group[1]:bottom"
                                    .parse()
                                    .expect("valid focus scope path"),
                            ),
                        ],
                    },
                ),
                (
                    "$workspace/group[1]:right/group[1]:bottom"
                        .parse()
                        .expect("valid focus scope path"),
                    FocusScopeNavigation {
                        axis: FocusAxis::Horizontal,
                        branches: vec![
                            FocusBranchKey::Window(window_id(3)),
                            FocusBranchKey::Window(window_id(4)),
                        ],
                    },
                ),
            ]
            .into_iter()
            .collect(),
        );
        let mut remembered = BTreeMap::new();
        let mut focused = window_id(1);

        step(
            &candidates,
            &mut focused,
            &mut remembered,
            &focus_tree,
            NavigationDirection::Right,
            2,
        );
        focused = window_id(1);
        remembered.insert(FocusTree::workspace_scope(), window_id(2));

        step(&candidates, &mut focused, &mut remembered, &focus_tree, NavigationDirection::Down, 3);
        step(
            &candidates,
            &mut focused,
            &mut remembered,
            &focus_tree,
            NavigationDirection::Right,
            4,
        );
        step(&candidates, &mut focused, &mut remembered, &focus_tree, NavigationDirection::Up, 2);
        step(&candidates, &mut focused, &mut remembered, &focus_tree, NavigationDirection::Down, 4);
        step(&candidates, &mut focused, &mut remembered, &focus_tree, NavigationDirection::Left, 3);
        step(&candidates, &mut focused, &mut remembered, &focus_tree, NavigationDirection::Left, 1);
        step(
            &candidates,
            &mut focused,
            &mut remembered,
            &focus_tree,
            NavigationDirection::Right,
            3,
        );
    }

    #[test]
    fn directional_focus_wraps_within_axis_and_preserves_column_memory() {
        fn step(
            candidates: &[WindowGeometryCandidate],
            focused: &mut WindowId,
            remembered: &mut BTreeMap<FocusScopePath, WindowId>,
            focus_tree: &FocusTree,
            direction: NavigationDirection,
            expected: u64,
        ) {
            *focused = select_directional_focus_candidate(
                candidates,
                Some(focused.clone()),
                direction,
                remembered,
                Some(focus_tree),
            )
            .expect("directional target");

            let scope_path = candidates
                .iter()
                .find(|candidate| candidate.window_id == *focused)
                .expect("focused candidate")
                .scope_path
                .clone();

            for scope_key in scope_path {
                remembered.insert(scope_key, focused.clone());
            }

            assert_eq!(*focused, window_id(expected));
        }

        let main_scope = "$workspace/group[0]:main-column";
        let side_scope = "$workspace/group[1]:side-column";
        let candidates = vec![
            candidate(1, 0, 0, 2553, 702, &["$workspace", main_scope]),
            candidate(2, 0, 702, 2553, 702, &["$workspace", main_scope]),
            candidate(3, 2553, 0, 851, 464, &["$workspace", side_scope]),
            candidate(4, 2553, 464, 851, 464, &["$workspace", side_scope]),
            candidate(5, 2553, 928, 851, 464, &["$workspace", side_scope]),
        ];
        let mut focus_tree = FocusTree::from_resolved_root(&crate::ResolvedLayoutNode::Workspace {
            meta: crate::LayoutNodeMeta::default(),
            children: vec![
                crate::ResolvedLayoutNode::Group {
                    meta: crate::LayoutNodeMeta {
                        id: Some("main-column".into()),
                        ..crate::LayoutNodeMeta::default()
                    },
                    children: vec![
                        crate::ResolvedLayoutNode::Window {
                            meta: crate::LayoutNodeMeta::default(),
                            window_id: Some(window_id(1)),
                            children: vec![],
                        },
                        crate::ResolvedLayoutNode::Window {
                            meta: crate::LayoutNodeMeta::default(),
                            window_id: Some(window_id(2)),
                            children: vec![],
                        },
                    ],
                },
                crate::ResolvedLayoutNode::Group {
                    meta: crate::LayoutNodeMeta {
                        id: Some("side-column".into()),
                        ..crate::LayoutNodeMeta::default()
                    },
                    children: vec![
                        crate::ResolvedLayoutNode::Window {
                            meta: crate::LayoutNodeMeta::default(),
                            window_id: Some(window_id(3)),
                            children: vec![],
                        },
                        crate::ResolvedLayoutNode::Window {
                            meta: crate::LayoutNodeMeta::default(),
                            window_id: Some(window_id(4)),
                            children: vec![],
                        },
                        crate::ResolvedLayoutNode::Window {
                            meta: crate::LayoutNodeMeta::default(),
                            window_id: Some(window_id(5)),
                            children: vec![],
                        },
                    ],
                },
            ],
        });
        focus_tree.set_navigation(
            [
                (
                    FocusTree::workspace_scope(),
                    FocusScopeNavigation {
                        axis: FocusAxis::Horizontal,
                        branches: vec![
                            FocusBranchKey::Scope(
                                main_scope.parse().expect("valid focus scope path"),
                            ),
                            FocusBranchKey::Scope(
                                side_scope.parse().expect("valid focus scope path"),
                            ),
                        ],
                    },
                ),
                (
                    main_scope.parse().expect("valid focus scope path"),
                    FocusScopeNavigation {
                        axis: FocusAxis::Vertical,
                        branches: vec![
                            FocusBranchKey::Window(window_id(1)),
                            FocusBranchKey::Window(window_id(2)),
                        ],
                    },
                ),
                (
                    side_scope.parse().expect("valid focus scope path"),
                    FocusScopeNavigation {
                        axis: FocusAxis::Vertical,
                        branches: vec![
                            FocusBranchKey::Window(window_id(3)),
                            FocusBranchKey::Window(window_id(4)),
                            FocusBranchKey::Window(window_id(5)),
                        ],
                    },
                ),
            ]
            .into_iter()
            .collect(),
        );
        let mut remembered = BTreeMap::new();
        let mut focused = window_id(1);
        remembered.insert(FocusTree::workspace_scope(), focused.clone());
        remembered.insert(main_scope.parse().expect("valid focus scope path"), focused.clone());

        step(&candidates, &mut focused, &mut remembered, &focus_tree, NavigationDirection::Down, 2);
        step(&candidates, &mut focused, &mut remembered, &focus_tree, NavigationDirection::Down, 1);
        step(
            &candidates,
            &mut focused,
            &mut remembered,
            &focus_tree,
            NavigationDirection::Right,
            3,
        );
        step(&candidates, &mut focused, &mut remembered, &focus_tree, NavigationDirection::Down, 4);
        step(&candidates, &mut focused, &mut remembered, &focus_tree, NavigationDirection::Down, 5);
        step(&candidates, &mut focused, &mut remembered, &focus_tree, NavigationDirection::Left, 1);
        step(
            &candidates,
            &mut focused,
            &mut remembered,
            &focus_tree,
            NavigationDirection::Right,
            5,
        );
        step(
            &candidates,
            &mut focused,
            &mut remembered,
            &focus_tree,
            NavigationDirection::Right,
            1,
        );
    }

    #[test]
    fn directional_focus_prefers_visual_ancestor_neighbor_before_inner_wrap() {
        use crate::focus::FocusTreeWindowGeometry;

        let candidates = vec![
            candidate(1, 0, 0, 2052, 1416, &["$workspace", "$workspace/visual[0]"]),
            candidate(
                2,
                2052,
                0,
                1352,
                714,
                &["$workspace", "$workspace/visual[1]", "$workspace/visual[1]/visual[0]"],
            ),
            candidate(
                3,
                2052,
                714,
                670,
                690,
                &[
                    "$workspace",
                    "$workspace/visual[1]",
                    "$workspace/visual[1]/visual[1]",
                    "$workspace/visual[1]/visual[1]/visual[0]",
                ],
            ),
            candidate(
                4,
                2734,
                714,
                670,
                690,
                &[
                    "$workspace",
                    "$workspace/visual[1]",
                    "$workspace/visual[1]/visual[1]",
                    "$workspace/visual[1]/visual[1]/visual[1]",
                ],
            ),
        ];
        let focus_tree = FocusTree::from_window_geometries(&[
            FocusTreeWindowGeometry {
                window_id: window_id(1),
                geometry: WindowGeometry { x: 0, y: 0, width: 2052, height: 1416 },
            },
            FocusTreeWindowGeometry {
                window_id: window_id(2),
                geometry: WindowGeometry { x: 2052, y: 0, width: 1352, height: 714 },
            },
            FocusTreeWindowGeometry {
                window_id: window_id(3),
                geometry: WindowGeometry { x: 2052, y: 714, width: 670, height: 690 },
            },
            FocusTreeWindowGeometry {
                window_id: window_id(4),
                geometry: WindowGeometry { x: 2734, y: 714, width: 670, height: 690 },
            },
        ]);
        let mut remembered = BTreeMap::new();
        remembered.insert(FocusTree::workspace_scope(), window_id(3));
        remembered
            .insert("$workspace/visual[1]".parse().expect("valid focus scope path"), window_id(3));
        remembered.insert(
            "$workspace/visual[1]/visual[1]".parse().expect("valid focus scope path"),
            window_id(3),
        );

        assert_eq!(
            select_directional_focus_candidate(
                &candidates,
                Some(window_id(3)),
                NavigationDirection::Left,
                &remembered,
                Some(&focus_tree),
            ),
            Some(window_id(1))
        );

        assert_eq!(
            select_directional_focus_candidate(
                &candidates,
                Some(window_id(4)),
                NavigationDirection::Right,
                &remembered,
                Some(&focus_tree),
            ),
            Some(window_id(1))
        );
    }

    #[test]
    fn directional_focus_wraps_within_visual_grid_rows() {
        use crate::focus::{FocusTree, FocusTreeWindowGeometry};

        let candidates = vec![
            candidate(
                1,
                0,
                0,
                500,
                400,
                &["$workspace", "$workspace/visual[0]", "$workspace/visual[0]/visual[0]"],
            ),
            candidate(
                2,
                500,
                0,
                500,
                400,
                &["$workspace", "$workspace/visual[0]", "$workspace/visual[0]/visual[1]"],
            ),
            candidate(
                3,
                0,
                400,
                500,
                400,
                &["$workspace", "$workspace/visual[1]", "$workspace/visual[1]/visual[0]"],
            ),
            candidate(
                4,
                500,
                400,
                500,
                400,
                &["$workspace", "$workspace/visual[1]", "$workspace/visual[1]/visual[1]"],
            ),
        ];
        let focus_tree = FocusTree::from_window_geometries(&[
            FocusTreeWindowGeometry {
                window_id: window_id(1),
                geometry: WindowGeometry { x: 0, y: 0, width: 500, height: 400 },
            },
            FocusTreeWindowGeometry {
                window_id: window_id(2),
                geometry: WindowGeometry { x: 500, y: 0, width: 500, height: 400 },
            },
            FocusTreeWindowGeometry {
                window_id: window_id(3),
                geometry: WindowGeometry { x: 0, y: 400, width: 500, height: 400 },
            },
            FocusTreeWindowGeometry {
                window_id: window_id(4),
                geometry: WindowGeometry { x: 500, y: 400, width: 500, height: 400 },
            },
        ]);

        assert_eq!(
            select_directional_focus_candidate(
                &candidates,
                Some(window_id(2)),
                NavigationDirection::Right,
                &BTreeMap::new(),
                Some(&focus_tree),
            ),
            Some(window_id(1))
        );

        assert_eq!(
            select_directional_focus_candidate(
                &candidates,
                Some(window_id(4)),
                NavigationDirection::Right,
                &BTreeMap::new(),
                Some(&focus_tree),
            ),
            Some(window_id(3))
        );
    }

    #[test]
    fn directional_focus_preserves_memory_in_nested_wrap_rows() {
        use crate::focus::{FocusTree, FocusTreeWindowGeometry};

        let candidates = vec![
            candidate(1, 0, 0, 1200, 900, &["$workspace", "$workspace/visual[0]"]),
            candidate(
                2,
                1200,
                0,
                1200,
                300,
                &["$workspace", "$workspace/visual[1]", "$workspace/visual[1]/visual[0]"],
            ),
            candidate(
                3,
                1200,
                300,
                600,
                300,
                &[
                    "$workspace",
                    "$workspace/visual[1]",
                    "$workspace/visual[1]/visual[1]",
                    "$workspace/visual[1]/visual[1]/visual[0]",
                ],
            ),
            candidate(
                4,
                1800,
                300,
                600,
                300,
                &[
                    "$workspace",
                    "$workspace/visual[1]",
                    "$workspace/visual[1]/visual[1]",
                    "$workspace/visual[1]/visual[1]/visual[1]",
                ],
            ),
            candidate(
                5,
                1200,
                600,
                1200,
                300,
                &["$workspace", "$workspace/visual[1]", "$workspace/visual[1]/visual[2]"],
            ),
        ];
        let focus_tree = FocusTree::from_window_geometries(&[
            FocusTreeWindowGeometry {
                window_id: window_id(1),
                geometry: WindowGeometry { x: 0, y: 0, width: 1200, height: 900 },
            },
            FocusTreeWindowGeometry {
                window_id: window_id(2),
                geometry: WindowGeometry { x: 1200, y: 0, width: 1200, height: 300 },
            },
            FocusTreeWindowGeometry {
                window_id: window_id(3),
                geometry: WindowGeometry { x: 1200, y: 300, width: 600, height: 300 },
            },
            FocusTreeWindowGeometry {
                window_id: window_id(4),
                geometry: WindowGeometry { x: 1800, y: 300, width: 600, height: 300 },
            },
            FocusTreeWindowGeometry {
                window_id: window_id(5),
                geometry: WindowGeometry { x: 1200, y: 600, width: 1200, height: 300 },
            },
        ]);
        let mut remembered = BTreeMap::new();
        remembered.insert(FocusTree::workspace_scope(), window_id(4));
        remembered
            .insert("$workspace/visual[1]".parse().expect("valid focus scope path"), window_id(4));
        remembered.insert(
            "$workspace/visual[1]/visual[1]".parse().expect("valid focus scope path"),
            window_id(4),
        );

        assert_eq!(
            select_directional_focus_candidate(
                &candidates,
                Some(window_id(1)),
                NavigationDirection::Right,
                &remembered,
                Some(&focus_tree),
            ),
            Some(window_id(4))
        );

        assert_eq!(
            select_directional_focus_candidate(
                &candidates,
                Some(window_id(4)),
                NavigationDirection::Down,
                &remembered,
                Some(&focus_tree),
            ),
            Some(window_id(5))
        );
    }

    #[test]
    fn managed_window_swap_positions_resolves_both_indices() {
        let window_order = vec![window_id(10), window_id(20), window_id(30)];

        assert_eq!(
            managed_window_swap_positions(&window_order, window_id(10), window_id(30)),
            Some((0, 2))
        );
    }

    #[test]
    fn managed_window_swap_positions_requires_both_windows() {
        let window_order = vec![window_id(10), window_id(20)];

        assert_eq!(
            managed_window_swap_positions(&window_order, window_id(10), window_id(30)),
            None
        );
    }
}
