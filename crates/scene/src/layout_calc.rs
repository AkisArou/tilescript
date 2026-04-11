use crate::css::{NodeComputedStyle, StyledLayoutTree};
use crate::scene::{LayoutSnapshotNode, SceneNodeStyle};
use hypreact_core::{LayoutRect, ResolvedLayoutNode};
use taffy::prelude::{AvailableSpace, Size as TaffyAvailableSize, TaffyTree};
use taffy::tree::{Layout as TaffyLayout, NodeId as TaffyNodeId};

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum LayoutCalcError {
    #[error("taffy tree construction failed")]
    BuildTree,
    #[error("taffy layout computation failed")]
    ComputeLayout,
    #[error("taffy layout collection failed")]
    CollectLayout,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LaidOutNode {
    pub node: ResolvedLayoutNode,
    pub computed: crate::css::ComputedStyle,
    pub taffy_style: taffy::style::Style,
    pub geometry: LayoutRect,
    pub children: Vec<LaidOutNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LaidOutTree {
    pub root: LaidOutNode,
}

impl LaidOutTree {
    pub fn snapshot(&self) -> LayoutSnapshotNode {
        self.root.snapshot()
    }
}

impl LaidOutNode {
    pub fn snapshot(&self) -> LayoutSnapshotNode {
        let styles = scene_style_from_node(self);

        match &self.node {
            ResolvedLayoutNode::Workspace { meta, .. } => LayoutSnapshotNode::Workspace {
                meta: meta.clone(),
                rect: self.geometry,
                styles,
                children: self.children.iter().map(Self::snapshot).collect(),
            },
            ResolvedLayoutNode::Group { meta, .. } => LayoutSnapshotNode::Group {
                meta: meta.clone(),
                rect: self.geometry,
                styles,
                children: self.children.iter().map(Self::snapshot).collect(),
            },
            ResolvedLayoutNode::Content { meta, text, .. } => LayoutSnapshotNode::Content {
                meta: meta.clone(),
                rect: self.geometry,
                styles,
                text: text.clone(),
                children: self.children.iter().map(Self::snapshot).collect(),
            },
            ResolvedLayoutNode::Window { meta, window_id, .. } => LayoutSnapshotNode::Window {
                meta: meta.clone(),
                rect: self.geometry,
                styles,
                window_id: window_id.clone(),
                children: self.children.iter().map(Self::snapshot).collect(),
            },
        }
    }
}

pub fn compute_layout_from_styled(
    styled: &StyledLayoutTree,
    width: f32,
    height: f32,
) -> Result<LaidOutTree, LayoutCalcError> {
    let root = root_node_with_space(&styled.root, width, height);
    let mut taffy = TaffyTree::new();
    let root_id = build_taffy_tree(&mut taffy, &root).map_err(|_| LayoutCalcError::BuildTree)?;
    taffy
        .compute_layout(
            root_id,
            TaffyAvailableSize {
                width: AvailableSpace::Definite(width),
                height: AvailableSpace::Definite(height),
            },
        )
        .map_err(|_| LayoutCalcError::ComputeLayout)?;

    Ok(LaidOutTree {
        root: collect_layout(&taffy, root_id, &root, 0.0, 0.0)
            .map_err(|_| LayoutCalcError::CollectLayout)?,
    })
}

fn scene_style_from_node(node: &LaidOutNode) -> Option<SceneNodeStyle> {
    (node.computed != crate::css::ComputedStyle::default())
        .then(|| SceneNodeStyle { layout: node.computed.clone() })
}

fn root_node_with_space(node: &NodeComputedStyle, width: f32, height: f32) -> NodeComputedStyle {
    let mut root = node.clone();
    root.taffy_style.size = taffy::geometry::Size {
        width: taffy::style::Dimension::length(width),
        height: taffy::style::Dimension::length(height),
    };
    root
}

fn build_taffy_tree(
    taffy: &mut TaffyTree<()>,
    node: &NodeComputedStyle,
) -> Result<TaffyNodeId, taffy::tree::TaffyError> {
    let child_ids = node
        .children
        .iter()
        .map(|child| build_taffy_tree(taffy, child))
        .collect::<Result<Vec<_>, _>>()?;

    if child_ids.is_empty() {
        taffy.new_leaf(node.taffy_style.clone())
    } else {
        taffy.new_with_children(node.taffy_style.clone(), &child_ids)
    }
}

fn collect_layout(
    taffy: &TaffyTree<()>,
    node_id: TaffyNodeId,
    node: &NodeComputedStyle,
    parent_x: f32,
    parent_y: f32,
) -> Result<LaidOutNode, taffy::tree::TaffyError> {
    let layout = *taffy.layout(node_id)?;
    let geometry = geometry_from_layout(layout, parent_x, parent_y);
    let child_ids = taffy.children(node_id)?;
    let children = node
        .children
        .iter()
        .zip(child_ids.iter())
        .map(|(child, child_id)| collect_layout(taffy, *child_id, child, geometry.x, geometry.y))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(LaidOutNode {
        node: node.node.clone(),
        computed: node.computed.clone(),
        taffy_style: node.taffy_style.clone(),
        geometry,
        children,
    })
}

fn geometry_from_layout(layout: TaffyLayout, parent_x: f32, parent_y: f32) -> LayoutRect {
    LayoutRect {
        x: parent_x + layout.location.x,
        y: parent_y + layout.location.y,
        width: layout.size.width,
        height: layout.size.height,
    }
}
