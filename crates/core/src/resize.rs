use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{LayoutRect, WindowId};

pub const DEFAULT_RESIZE_STEP_UNITS: u32 = 12;
pub const MIN_BRANCH_SHARE_UNITS: u32 = 1;
pub const DEFAULT_BRANCH_SHARE_UNITS: u32 = 120;
pub const AUTHORED_SHARE_SCALE: u32 = 12;

pub fn scale_authored_share_units(value: u32) -> u32 {
    value.saturating_mul(AUTHORED_SHARE_SCALE)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResizeDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PartitionId(pub String);

impl PartitionId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PartitionAxis {
    Horizontal,
    Vertical,
}

impl PartitionAxis {
    pub fn from_resize_direction(direction: ResizeDirection) -> Self {
        match direction {
            ResizeDirection::Left | ResizeDirection::Right => Self::Horizontal,
            ResizeDirection::Up | ResizeDirection::Down => Self::Vertical,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PartitionConstraints {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_share: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_share: Option<u32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fixed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartitionBranch {
    pub branch_id: String,
    pub rect: LayoutRect,
    pub descendant_window_ids: Vec<WindowId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_share: Option<u32>,
    #[serde(default, skip_serializing_if = "PartitionConstraints::is_default")]
    pub constraints: PartitionConstraints,
}

impl PartitionConstraints {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}

impl WorkspaceResizeState {
    pub fn is_empty(&self) -> bool {
        self.adjustments_by_partition_id.is_empty()
    }
}

impl ResizeState {
    pub fn is_empty(&self) -> bool {
        self.by_workspace_id.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartitionNode {
    pub partition_id: PartitionId,
    pub axis: PartitionAxis,
    pub rect: LayoutRect,
    pub branches: Vec<PartitionBranch>,
    pub adjustable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PartitionTree {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub root_partition_ids: Vec<PartitionId>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub partitions: BTreeMap<PartitionId, PartitionNode>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub window_to_partition_path: BTreeMap<WindowId, Vec<PartitionId>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartitionAdjustment {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub branch_ids: Vec<String>,
    pub branch_shares: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceResizeState {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub adjustments_by_partition_id: BTreeMap<PartitionId, PartitionAdjustment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResizeState {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub by_workspace_id: BTreeMap<crate::WorkspaceId, WorkspaceResizeState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResizeCandidate {
    pub partition_id: PartitionId,
    pub grow_branch_index: usize,
    pub shrink_branch_index: usize,
}

pub fn reconciled_branch_shares(
    adjustment: &PartitionAdjustment,
    branch_ids: &[String],
    branch_default_shares: &[Option<u32>],
) -> Vec<u32> {
    let use_branch_ids = adjustment.branch_ids.len() == adjustment.branch_shares.len()
        && !adjustment.branch_ids.is_empty();

    let saved_by_branch_id = if use_branch_ids {
        adjustment
            .branch_ids
            .iter()
            .cloned()
            .zip(adjustment.branch_shares.iter().copied())
            .collect::<BTreeMap<_, _>>()
    } else {
        BTreeMap::new()
    };

    branch_ids
        .iter()
        .enumerate()
        .map(|(index, branch_id)| {
            if use_branch_ids {
                saved_by_branch_id
                    .get(branch_id)
                    .copied()
                    .unwrap_or_else(|| default_branch_share(branch_default_shares, index))
            } else {
                adjustment
                    .branch_shares
                    .get(index)
                    .copied()
                    .unwrap_or_else(|| default_branch_share(branch_default_shares, index))
            }
        })
        .collect()
}

pub fn select_resize_candidate(
    partition_tree: &PartitionTree,
    focused_window_id: &WindowId,
    direction: ResizeDirection,
) -> Option<ResizeCandidate> {
    let target_axis = PartitionAxis::from_resize_direction(direction);
    let partition_path = partition_tree.window_to_partition_path.get(focused_window_id)?;

    for partition_id in partition_path.iter().rev() {
        let partition = partition_tree.partitions.get(partition_id)?;
        if !partition.adjustable || partition.axis != target_axis {
            continue;
        }

        let focused_branch_index = partition
            .branches
            .iter()
            .position(|branch| branch.descendant_window_ids.contains(focused_window_id));
        let Some(focused_branch_index) = focused_branch_index else {
            continue;
        };

        let candidate = match direction {
            ResizeDirection::Left | ResizeDirection::Up => focused_branch_index
                .checked_sub(1)
                .map(|previous_branch_index| ResizeCandidate {
                    partition_id: partition.partition_id.clone(),
                    grow_branch_index: focused_branch_index,
                    shrink_branch_index: previous_branch_index,
                })
                .or_else(|| {
                    (focused_branch_index + 1 < partition.branches.len()).then_some(
                        ResizeCandidate {
                            partition_id: partition.partition_id.clone(),
                            grow_branch_index: focused_branch_index + 1,
                            shrink_branch_index: focused_branch_index,
                        },
                    )
                }),
            ResizeDirection::Right | ResizeDirection::Down => (focused_branch_index + 1
                < partition.branches.len())
            .then_some(ResizeCandidate {
                partition_id: partition.partition_id.clone(),
                grow_branch_index: focused_branch_index,
                shrink_branch_index: focused_branch_index + 1,
            })
            .or_else(|| {
                focused_branch_index.checked_sub(1).map(|previous_branch_index| ResizeCandidate {
                    partition_id: partition.partition_id.clone(),
                    grow_branch_index: previous_branch_index,
                    shrink_branch_index: focused_branch_index,
                })
            }),
        };
        let Some(candidate) = candidate else {
            continue;
        };

        return Some(candidate);
    }

    None
}

pub fn apply_resize_step(
    state: &mut WorkspaceResizeState,
    partition_tree: &PartitionTree,
    candidate: &ResizeCandidate,
    step_units: u32,
) -> bool {
    let Some(partition) = partition_tree.partitions.get(&candidate.partition_id) else {
        return false;
    };

    if candidate.grow_branch_index >= partition.branches.len()
        || candidate.shrink_branch_index >= partition.branches.len()
        || candidate.grow_branch_index == candidate.shrink_branch_index
    {
        return false;
    }

    let grow_branch = &partition.branches[candidate.grow_branch_index];
    let shrink_branch = &partition.branches[candidate.shrink_branch_index];
    if grow_branch.constraints.fixed || shrink_branch.constraints.fixed {
        return false;
    }

    let adjustment = state
        .adjustments_by_partition_id
        .entry(candidate.partition_id.clone())
        .or_insert_with(|| default_partition_adjustment(partition));
    sync_partition_adjustment(adjustment, partition);

    let grow_share = adjustment.branch_shares[candidate.grow_branch_index];
    let shrink_share = adjustment.branch_shares[candidate.shrink_branch_index];
    let shrink_min = shrink_branch.constraints.min_share.unwrap_or(MIN_BRANCH_SHARE_UNITS);
    let grow_max = grow_branch.constraints.max_share;

    if shrink_share <= shrink_min {
        return false;
    }

    let applicable_step = step_units.min(shrink_share.saturating_sub(shrink_min));
    if applicable_step == 0 {
        return false;
    }

    if let Some(grow_max) = grow_max {
        let grow_headroom = grow_max.saturating_sub(grow_share);
        if grow_headroom == 0 {
            return false;
        }
        let applicable_step = applicable_step.min(grow_headroom);
        if applicable_step == 0 {
            return false;
        }

        adjustment.branch_shares[candidate.grow_branch_index] += applicable_step;
        adjustment.branch_shares[candidate.shrink_branch_index] -= applicable_step;
        return true;
    }

    adjustment.branch_shares[candidate.grow_branch_index] += applicable_step;
    adjustment.branch_shares[candidate.shrink_branch_index] -= applicable_step;
    true
}

pub fn gc_resize_state(state: &mut WorkspaceResizeState, partition_tree: &PartitionTree) {
    let valid_partition_ids = partition_tree.partitions.keys().cloned().collect::<BTreeSet<_>>();
    state
        .adjustments_by_partition_id
        .retain(|partition_id, _| valid_partition_ids.contains(partition_id));
}

fn default_partition_adjustment(partition: &PartitionNode) -> PartitionAdjustment {
    PartitionAdjustment {
        branch_ids: partition.branches.iter().map(|branch| branch.branch_id.clone()).collect(),
        branch_shares: partition
            .branches
            .iter()
            .map(|branch| branch.default_share.unwrap_or(DEFAULT_BRANCH_SHARE_UNITS))
            .collect(),
    }
}

fn sync_partition_adjustment(adjustment: &mut PartitionAdjustment, partition: &PartitionNode) {
    let branch_ids =
        partition.branches.iter().map(|branch| branch.branch_id.clone()).collect::<Vec<_>>();
    let branch_default_shares =
        partition.branches.iter().map(|branch| branch.default_share).collect::<Vec<_>>();
    let branch_shares = reconciled_branch_shares(adjustment, &branch_ids, &branch_default_shares);

    adjustment.branch_ids = branch_ids;
    adjustment.branch_shares = branch_shares;
}

fn default_branch_share(branch_default_shares: &[Option<u32>], index: usize) -> u32 {
    branch_default_shares.get(index).copied().flatten().unwrap_or(DEFAULT_BRANCH_SHARE_UNITS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn partition_tree() -> PartitionTree {
        let partition_id = PartitionId::new("frame");
        let master = WindowId::from("master");
        let stack = WindowId::from("stack");

        PartitionTree {
            root_partition_ids: vec![partition_id.clone()],
            partitions: BTreeMap::from([(
                partition_id.clone(),
                PartitionNode {
                    partition_id: partition_id.clone(),
                    axis: PartitionAxis::Horizontal,
                    rect: LayoutRect { x: 0.0, y: 0.0, width: 1000.0, height: 700.0 },
                    adjustable: true,
                    branches: vec![
                        PartitionBranch {
                            branch_id: "master".to_string(),
                            rect: LayoutRect { x: 0.0, y: 0.0, width: 600.0, height: 700.0 },
                            descendant_window_ids: vec![master.clone()],
                            default_share: None,
                            constraints: PartitionConstraints::default(),
                        },
                        PartitionBranch {
                            branch_id: "stack".to_string(),
                            rect: LayoutRect { x: 600.0, y: 0.0, width: 400.0, height: 700.0 },
                            descendant_window_ids: vec![stack.clone()],
                            default_share: None,
                            constraints: PartitionConstraints::default(),
                        },
                    ],
                },
            )]),
            window_to_partition_path: BTreeMap::from([
                (master, vec![partition_id.clone()]),
                (stack, vec![partition_id]),
            ]),
        }
    }

    #[test]
    fn select_resize_candidate_uses_requested_side() {
        let tree = partition_tree();

        assert_eq!(
            select_resize_candidate(&tree, &WindowId::from("master"), ResizeDirection::Right),
            Some(ResizeCandidate {
                partition_id: PartitionId::new("frame"),
                grow_branch_index: 0,
                shrink_branch_index: 1,
            })
        );
        assert_eq!(
            select_resize_candidate(&tree, &WindowId::from("stack"), ResizeDirection::Left),
            Some(ResizeCandidate {
                partition_id: PartitionId::new("frame"),
                grow_branch_index: 1,
                shrink_branch_index: 0,
            })
        );
        assert_eq!(
            select_resize_candidate(&tree, &WindowId::from("master"), ResizeDirection::Left),
            Some(ResizeCandidate {
                partition_id: PartitionId::new("frame"),
                grow_branch_index: 1,
                shrink_branch_index: 0,
            })
        );
        assert_eq!(
            select_resize_candidate(&tree, &WindowId::from("stack"), ResizeDirection::Right),
            Some(ResizeCandidate {
                partition_id: PartitionId::new("frame"),
                grow_branch_index: 0,
                shrink_branch_index: 1,
            })
        );
    }

    #[test]
    fn select_resize_candidate_rejects_missing_side() {
        let tree = partition_tree();

        assert_eq!(
            select_resize_candidate(&tree, &WindowId::from("master"), ResizeDirection::Up),
            None
        );
        assert_eq!(
            select_resize_candidate(&tree, &WindowId::from("stack"), ResizeDirection::Down),
            None
        );
    }

    #[test]
    fn apply_resize_step_mutates_partition_shares() {
        let tree = partition_tree();
        let mut state = WorkspaceResizeState::default();
        let candidate =
            select_resize_candidate(&tree, &WindowId::from("master"), ResizeDirection::Right)
                .expect("resize candidate");

        assert!(apply_resize_step(&mut state, &tree, &candidate, DEFAULT_RESIZE_STEP_UNITS,));
        assert_eq!(
            state.adjustments_by_partition_id[&PartitionId::new("frame")].branch_shares,
            vec![132, 108]
        );
    }

    #[test]
    fn gc_resize_state_removes_unknown_partitions() {
        let tree = partition_tree();
        let mut state = WorkspaceResizeState {
            adjustments_by_partition_id: BTreeMap::from([
                (
                    PartitionId::new("frame"),
                    PartitionAdjustment {
                        branch_ids: vec!["master".into(), "stack".into()],
                        branch_shares: vec![12, 12],
                    },
                ),
                (
                    PartitionId::new("stale"),
                    PartitionAdjustment {
                        branch_ids: vec!["left".into(), "right".into()],
                        branch_shares: vec![8, 16],
                    },
                ),
            ]),
        };

        gc_resize_state(&mut state, &tree);

        assert!(state.adjustments_by_partition_id.contains_key(&PartitionId::new("frame")));
        assert!(!state.adjustments_by_partition_id.contains_key(&PartitionId::new("stale")));
    }

    #[test]
    fn apply_resize_step_uses_authored_default_branch_shares() {
        let mut tree = partition_tree();
        tree.partitions.get_mut(&PartitionId::new("frame")).expect("frame partition").branches = vec![
            PartitionBranch {
                branch_id: "master".to_string(),
                rect: LayoutRect { x: 0.0, y: 0.0, width: 600.0, height: 700.0 },
                descendant_window_ids: vec![WindowId::from("master")],
                default_share: Some(scale_authored_share_units(3)),
                constraints: PartitionConstraints::default(),
            },
            PartitionBranch {
                branch_id: "stack".to_string(),
                rect: LayoutRect { x: 600.0, y: 0.0, width: 400.0, height: 700.0 },
                descendant_window_ids: vec![WindowId::from("stack")],
                default_share: Some(scale_authored_share_units(2)),
                constraints: PartitionConstraints::default(),
            },
        ];
        let mut state = WorkspaceResizeState::default();
        let candidate =
            select_resize_candidate(&tree, &WindowId::from("master"), ResizeDirection::Right)
                .expect("resize candidate");

        assert!(apply_resize_step(&mut state, &tree, &candidate, DEFAULT_RESIZE_STEP_UNITS,));
        assert_eq!(
            state.adjustments_by_partition_id[&PartitionId::new("frame")].branch_shares,
            vec![48, 12]
        );
    }

    #[test]
    fn reconciled_branch_shares_preserve_matching_branch_ids_and_default_new_branches() {
        let adjustment = PartitionAdjustment {
            branch_ids: vec!["master".into(), "stack".into()],
            branch_shares: vec![5, 7],
        };

        assert_eq!(
            reconciled_branch_shares(
                &adjustment,
                &["stack".into(), "master".into(), "extra".into()],
                &[
                    Some(scale_authored_share_units(2)),
                    Some(scale_authored_share_units(3)),
                    Some(scale_authored_share_units(4)),
                ],
            ),
            vec![7, 5, 48]
        );
    }

    #[test]
    fn apply_resize_step_clamps_against_branch_min_and_max() {
        let mut tree = partition_tree();
        let frame = tree.partitions.get_mut(&PartitionId::new("frame")).expect("frame partition");
        frame.branches[0].default_share = Some(scale_authored_share_units(3));
        frame.branches[0].constraints.max_share = Some(scale_authored_share_units(4));
        frame.branches[1].default_share = Some(scale_authored_share_units(2));
        frame.branches[1].constraints.min_share = Some(scale_authored_share_units(1));

        let mut state = WorkspaceResizeState::default();
        let candidate =
            select_resize_candidate(&tree, &WindowId::from("master"), ResizeDirection::Right)
                .expect("resize candidate");

        assert!(apply_resize_step(&mut state, &tree, &candidate, 10));
        assert_eq!(
            state.adjustments_by_partition_id[&PartitionId::new("frame")].branch_shares,
            vec![46, 14]
        );
        assert!(apply_resize_step(&mut state, &tree, &candidate, 10));
        assert_eq!(
            state.adjustments_by_partition_id[&PartitionId::new("frame")].branch_shares,
            vec![48, 12]
        );
        assert!(!apply_resize_step(&mut state, &tree, &candidate, 10));
    }
}
