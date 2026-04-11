use crate::css::{
    CssValueError, NodeComputedStyle, StyledLayoutTree, compute_style, map_computed_style_to_taffy,
};
use hypreact_core::ResolvedLayoutNode;

pub fn build_styled_layout_tree_from_sheet(
    root: &ResolvedLayoutNode,
    sheet: &crate::css::CompiledStyleSheet,
) -> Result<StyledLayoutTree, CssValueError> {
    Ok(StyledLayoutTree { root: style_node(root, sheet)? })
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

    Ok(NodeComputedStyle { node: node.clone(), computed, taffy_style, children })
}
