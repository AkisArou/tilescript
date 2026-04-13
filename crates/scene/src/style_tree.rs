use crate::css::{
    compute_style, map_computed_style_to_taffy, CssValueError, NodeComputedStyle, StyledLayoutTree,
};
use hypreact_core::resize::{
    reconciled_branch_shares, scale_authored_share_units, PartitionAxis, PartitionId,
    WorkspaceResizeState,
};
use hypreact_core::ResolvedLayoutNode;
use hypreact_css::{Display, FlexDirectionValue, SizeValue};
use taffy::style::Dimension as TaffyDimension;

pub fn build_styled_layout_tree_from_sheet(
    root: &ResolvedLayoutNode,
    sheet: &crate::css::CompiledStyleSheet,
) -> Result<StyledLayoutTree, CssValueError> {
    Ok(StyledLayoutTree {
        root: style_node(root, sheet)?,
    })
}

pub fn build_styled_layout_tree_from_sheet_with_resize_state(
    root: &ResolvedLayoutNode,
    sheet: &crate::css::CompiledStyleSheet,
    resize_state: &WorkspaceResizeState,
) -> Result<StyledLayoutTree, CssValueError> {
    let mut root = style_node(root, sheet)?;
    let mut path = Vec::new();
    apply_resize_adjustments(&mut root, resize_state, &mut path, true);
    Ok(StyledLayoutTree { root })
}

fn style_node(
    node: &ResolvedLayoutNode,
    sheet: &crate::css::CompiledStyleSheet,
) -> Result<NodeComputedStyle, CssValueError> {
    let computed = compute_style(sheet, node)?;
    let taffy_style = map_computed_style_to_taffy(&computed);
    let children = match node {
        ResolvedLayoutNode::Workspace { children, .. }
        | ResolvedLayoutNode::Group { children, .. }
        | ResolvedLayoutNode::Window { children, .. }
        | ResolvedLayoutNode::Content { children, .. } => children
            .iter()
            .map(|child| style_node(child, sheet))
            .collect::<Result<Vec<_>, _>>()?,
    };

    Ok(NodeComputedStyle {
        node: node.clone(),
        computed,
        taffy_style,
        children,
    })
}

fn apply_resize_adjustments(
    node: &mut NodeComputedStyle,
    resize_state: &WorkspaceResizeState,
    path: &mut Vec<String>,
    is_root: bool,
) {
    let current_partition_id = partition_id_for_styled_node(node, path, is_root);
    if let Some(partition_id) = current_partition_id.as_ref() {
        path.push(partition_id.clone());
    }

    let path_len_before_children = path.len();
    for child in &mut node.children {
        apply_resize_adjustments(child, resize_state, path, false);
        path.truncate(path_len_before_children);
    }

    let Some(partition_id) = current_partition_id else {
        return;
    };
    let Some(adjustment) = resize_state
        .adjustments_by_partition_id
        .get(&PartitionId(partition_id.clone()))
    else {
        return;
    };
    let Some(axis) = partition_axis(&node.computed) else {
        return;
    };
    if node.children.len() < 2 {
        return;
    }
    if node
        .children
        .iter()
        .any(|child| branch_is_fixed_on_axis(&child.computed, axis))
    {
        return;
    }

    let branch_ids = node
        .children
        .iter()
        .enumerate()
        .map(|(index, child)| branch_id_for_styled_child(node, child, index))
        .collect::<Vec<_>>();
    let branch_default_shares = node
        .children
        .iter()
        .map(|child| inferred_branch_default_share(&child.computed, axis))
        .collect::<Vec<_>>();
    let branch_shares = reconciled_branch_shares(adjustment, &branch_ids, &branch_default_shares);

    for (child, share) in node.children.iter_mut().zip(branch_shares.into_iter()) {
        child.taffy_style.flex_grow = share as f32;
        child.taffy_style.flex_shrink = 1.0;
        child.taffy_style.flex_basis = TaffyDimension::length(0.0);
    }
}

fn partition_id_for_styled_node(
    node: &NodeComputedStyle,
    path: &[String],
    is_root: bool,
) -> Option<String> {
    if partition_axis(&node.computed).is_none() || node.children.len() < 2 {
        return None;
    }

    node.node
        .meta()
        .id
        .clone()
        .or_else(|| partition_structural_id(node, path, is_root))
}

fn partition_structural_id(
    node: &NodeComputedStyle,
    _path: &[String],
    is_root: bool,
) -> Option<String> {
    let node_kind = match node.node {
        ResolvedLayoutNode::Workspace { .. } => "workspace",
        ResolvedLayoutNode::Group { .. } => "group",
        ResolvedLayoutNode::Content { .. } => "content",
        ResolvedLayoutNode::Window { .. } => "window",
    };

    if !is_root
        && !matches!(
            node.node,
            ResolvedLayoutNode::Group { .. } | ResolvedLayoutNode::Content { .. }
        )
    {
        return None;
    }

    Some(format!("{node_kind}-partition"))
}

fn branch_id_for_styled_child(
    parent: &NodeComputedStyle,
    child: &NodeComputedStyle,
    index: usize,
) -> String {
    if let Some(id) = child.node.meta().id.as_ref().filter(|id| {
        parent
            .children
            .iter()
            .filter(|sibling| sibling.node.meta().id.as_ref() == Some(*id))
            .count()
            == 1
    }) {
        return id.clone();
    }

    if let ResolvedLayoutNode::Window {
        window_id: Some(window_id),
        ..
    } = &child.node
    {
        return window_id.to_string();
    }

    fallback_branch_id_for_styled_parent(parent, index)
}

fn fallback_branch_id_for_styled_parent(parent: &NodeComputedStyle, index: usize) -> String {
    match parent.node {
        ResolvedLayoutNode::Workspace { .. } => format!("workspace-branch-{index}"),
        ResolvedLayoutNode::Group { .. } => format!("group-branch-{index}"),
        ResolvedLayoutNode::Content { .. } => format!("content-branch-{index}"),
        ResolvedLayoutNode::Window { .. } => format!("window-branch-{index}"),
    }
}

fn flex_partition_axis(
    computed: &crate::css::ComputedStyle,
) -> Option<hypreact_core::resize::PartitionAxis> {
    if computed.display != Some(Display::Flex) {
        return None;
    }

    Some(match computed.flex_direction {
        Some(FlexDirectionValue::Column) | Some(FlexDirectionValue::ColumnReverse) => {
            hypreact_core::resize::PartitionAxis::Vertical
        }
        _ => hypreact_core::resize::PartitionAxis::Horizontal,
    })
}

fn partition_axis(
    computed: &crate::css::ComputedStyle,
) -> Option<hypreact_core::resize::PartitionAxis> {
    flex_partition_axis(computed)
}

fn branch_is_fixed_on_axis(
    computed: &crate::css::ComputedStyle,
    axis: hypreact_core::resize::PartitionAxis,
) -> bool {
    let explicit_main_size = match axis {
        hypreact_core::resize::PartitionAxis::Horizontal => computed.width,
        hypreact_core::resize::PartitionAxis::Vertical => computed.height,
    };

    if matches!(explicit_main_size, Some(SizeValue::LengthPercentage(_))) {
        return true;
    }

    if computed.flex_grow.unwrap_or(0.0) == 0.0 {
        return true;
    }

    computed.flex_shrink.unwrap_or(1.0) == 0.0
}

fn inferred_branch_default_share(
    computed: &crate::css::ComputedStyle,
    axis: PartitionAxis,
) -> Option<u32> {
    if branch_is_fixed_on_axis(computed, axis) {
        return None;
    }

    let grow = computed.flex_grow.unwrap_or(1.0);
    if !grow.is_finite() || grow <= 0.0 {
        return None;
    }

    let scaled = (grow * scale_authored_share_units(1) as f32).round();
    Some(scaled.max(1.0) as u32)
}
