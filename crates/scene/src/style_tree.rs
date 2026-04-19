use crate::css::{
    CssValueError, NodeComputedStyle, StyledLayoutTree, compute_style, map_computed_style_to_taffy,
};
use taffy::style::Dimension as TaffyDimension;
use tilescript_core::ResolvedLayoutNode;
use tilescript_core::resize::{
    PartitionAxis, PartitionId, PartitionNodeKind, WorkspaceResizeState, reconciled_branch_shares,
    scale_authored_share_units, structural_partition_id,
};
use tilescript_css::{Display, FlexDirectionValue, SizeValue};

pub fn build_styled_layout_tree_from_sheet(
    root: &ResolvedLayoutNode,
    sheet: &crate::css::CompiledStyleSheet,
) -> Result<StyledLayoutTree, CssValueError> {
    Ok(StyledLayoutTree { root: style_node(root, sheet)? })
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
        | ResolvedLayoutNode::Content { children, .. } => {
            children.iter().map(|child| style_node(child, sheet)).collect::<Result<Vec<_>, _>>()?
        }
    };

    let mut children = children;
    children.sort_by_key(|child| child.computed.order.unwrap_or(0));

    Ok(NodeComputedStyle { node: node.clone(), computed, taffy_style, children })
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
    let Some(adjustment) =
        resize_state.adjustments_by_partition_id.get(&PartitionId(partition_id.clone()))
    else {
        return;
    };
    let Some(axis) = partition_axis(&node.computed) else {
        return;
    };
    if node.children.len() < 2 {
        return;
    }
    if node.children.iter().any(|child| branch_is_fixed_on_axis(&child.computed, axis)) {
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

    node.node.meta().id.clone().or_else(|| partition_structural_id(node, path, is_root))
}

fn partition_structural_id(
    node: &NodeComputedStyle,
    path: &[String],
    is_root: bool,
) -> Option<String> {
    if !is_root
        && !matches!(
            node.node,
            ResolvedLayoutNode::Group { .. } | ResolvedLayoutNode::Content { .. }
        )
    {
        return None;
    }

    Some(structural_partition_id(partition_node_kind(&node.node), path))
}

fn partition_node_kind(node: &ResolvedLayoutNode) -> PartitionNodeKind {
    match node {
        ResolvedLayoutNode::Workspace { .. } => PartitionNodeKind::Workspace,
        ResolvedLayoutNode::Group { .. } => PartitionNodeKind::Group,
        ResolvedLayoutNode::Content { .. } => PartitionNodeKind::Content,
        ResolvedLayoutNode::Window { .. } => PartitionNodeKind::Window,
    }
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

    if let ResolvedLayoutNode::Window { window_id: Some(window_id), .. } = &child.node {
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
) -> Option<tilescript_core::resize::PartitionAxis> {
    if computed.display != Some(Display::Flex) {
        return None;
    }

    Some(match computed.flex_direction {
        Some(FlexDirectionValue::Column) | Some(FlexDirectionValue::ColumnReverse) => {
            tilescript_core::resize::PartitionAxis::Vertical
        }
        _ => tilescript_core::resize::PartitionAxis::Horizontal,
    })
}

fn partition_axis(
    computed: &crate::css::ComputedStyle,
) -> Option<tilescript_core::resize::PartitionAxis> {
    flex_partition_axis(computed)
}

fn branch_is_fixed_on_axis(
    computed: &crate::css::ComputedStyle,
    axis: tilescript_core::resize::PartitionAxis,
) -> bool {
    let explicit_main_size = match axis {
        tilescript_core::resize::PartitionAxis::Horizontal => computed.width,
        tilescript_core::resize::PartitionAxis::Vertical => computed.height,
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

#[cfg(test)]
mod tests {
    use crate::pipeline::{compile_stylesheet, compute_layout_from_request_with_sheet};
    use crate::scene::SceneRequest;
    use crate::style_tree::build_styled_layout_tree_from_sheet;
    use tilescript_core::resize::{PartitionAdjustment, PartitionId, WorkspaceResizeState};
    use tilescript_core::runtime::prepared_layout::{PreparedStylesheet, PreparedStylesheets};
    use tilescript_core::{LayoutNodeMeta, LayoutSpace, ResolvedLayoutNode, WindowId, WorkspaceId};

    fn dwindle_root() -> ResolvedLayoutNode {
        ResolvedLayoutNode::Workspace {
            meta: LayoutNodeMeta { id: Some("frame".into()), ..LayoutNodeMeta::default() },
            children: vec![
                ResolvedLayoutNode::Window {
                    meta: LayoutNodeMeta { id: Some("master".into()), ..LayoutNodeMeta::default() },
                    window_id: Some(WindowId::from("w1")),
                    children: vec![],
                },
                ResolvedLayoutNode::Group {
                    meta: LayoutNodeMeta {
                        id: Some("stack-col".into()),
                        ..LayoutNodeMeta::default()
                    },
                    children: vec![
                        ResolvedLayoutNode::Window {
                            meta: LayoutNodeMeta::default(),
                            window_id: Some(WindowId::from("w2")),
                            children: vec![],
                        },
                        ResolvedLayoutNode::Group {
                            meta: LayoutNodeMeta {
                                id: Some("stack-row".into()),
                                ..LayoutNodeMeta::default()
                            },
                            children: vec![
                                ResolvedLayoutNode::Window {
                                    meta: LayoutNodeMeta::default(),
                                    window_id: Some(WindowId::from("w3")),
                                    children: vec![],
                                },
                                ResolvedLayoutNode::Window {
                                    meta: LayoutNodeMeta::default(),
                                    window_id: Some(WindowId::from("w4")),
                                    children: vec![],
                                },
                            ],
                        },
                    ],
                },
            ],
        }
    }

    fn dwindle_stylesheet() -> &'static str {
        "#frame { display: flex; flex-direction: row; width: 100%; height: 100%; }\
         #master { flex-grow: 1; flex-basis: 0px; min-width: 0px; min-height: 0px; }\
         #stack-col { display: flex; flex-direction: column; flex-grow: 1; flex-basis: 0px; min-width: 0px; min-height: 0px; }\
         #stack-row { display: flex; flex-direction: row; flex-grow: 1; flex-basis: 0px; min-width: 0px; min-height: 0px; }\
         window { flex-grow: 1; flex-basis: 0px; min-width: 0px; min-height: 0px; }"
    }

    #[test]
    fn nested_vertical_resize_does_not_change_nested_horizontal_width() {
        let root = dwindle_root();
        let sheet = compile_stylesheet(dwindle_stylesheet()).expect("compiled stylesheet");

        let base_request = SceneRequest {
            workspace_id: WorkspaceId::from("1"),
            output_id: None,
            layout_name: Some("dwindle".into()),
            root: root.clone(),
            stylesheets: PreparedStylesheets {
                global: None,
                layout: Some(PreparedStylesheet {
                    path: "layouts/dwindle/index.css".into(),
                    source: dwindle_stylesheet().into(),
                }),
            },
            space: LayoutSpace { width: 1600.0, height: 1000.0 },
            resize_state: WorkspaceResizeState::default(),
        };

        let before =
            compute_layout_from_request_with_sheet(&base_request, &sheet).expect("base layout");
        let before_w3 =
            before.root.find_by_window_id(&WindowId::from("w3")).expect("w3 before").rect();
        let before_w4 =
            before.root.find_by_window_id(&WindowId::from("w4")).expect("w4 before").rect();

        let adjusted_request = SceneRequest {
            resize_state: WorkspaceResizeState {
                adjustments_by_partition_id: [(
                    PartitionId::new("stack-col"),
                    PartitionAdjustment {
                        branch_ids: vec!["w2".into(), "stack-row".into()],
                        branch_shares: vec![12, 24],
                    },
                )]
                .into_iter()
                .collect(),
                sibling_order_by_container_id: Default::default(),
            },
            ..base_request
        };

        let after = compute_layout_from_request_with_sheet(&adjusted_request, &sheet)
            .expect("adjusted layout");
        let after_w3 =
            after.root.find_by_window_id(&WindowId::from("w3")).expect("w3 after").rect();
        let after_w4 =
            after.root.find_by_window_id(&WindowId::from("w4")).expect("w4 after").rect();

        assert_eq!(before_w3.x, after_w3.x);
        assert_eq!(before_w4.x, after_w4.x);
        assert_eq!(before_w3.width, after_w3.width);
        assert_eq!(before_w4.width, after_w4.width);
        assert_eq!(before_w3.height, before_w4.height);
        assert_eq!(after_w3.height, after_w4.height);
        assert!(after_w4.height > before_w4.height);
    }

    #[test]
    fn order_reorders_siblings_before_layout() {
        let root = ResolvedLayoutNode::Workspace {
            meta: LayoutNodeMeta::default(),
            children: vec![
                ResolvedLayoutNode::Window {
                    meta: LayoutNodeMeta { id: Some("first".into()), ..LayoutNodeMeta::default() },
                    window_id: Some(WindowId::from("w1")),
                    children: vec![],
                },
                ResolvedLayoutNode::Window {
                    meta: LayoutNodeMeta { id: Some("second".into()), ..LayoutNodeMeta::default() },
                    window_id: Some(WindowId::from("w2")),
                    children: vec![],
                },
            ],
        };

        let sheet = compile_stylesheet(
            r#"
            workspace { display: flex; flex-direction: row; }
            #first { order: 2; width: 100px; }
            #second { order: 1; width: 200px; }
            "#,
        )
        .unwrap();

        let styled = build_styled_layout_tree_from_sheet(&root, &sheet).unwrap();

        assert_eq!(styled.root.children[0].node.meta().id.as_deref(), Some("second"));
        assert_eq!(styled.root.children[1].node.meta().id.as_deref(), Some("first"));
    }
}
