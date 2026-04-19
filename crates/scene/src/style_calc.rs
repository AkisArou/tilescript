use tilescript_core::ResolvedLayoutNode;

use crate::css::apply::ApplyCompiledDeclaration;
use crate::css::{CompiledStyleSheet, ComputedStyle, CssValueError, LayoutDomTree};

pub fn compute_style(
    sheet: &CompiledStyleSheet,
    node: &ResolvedLayoutNode,
) -> Result<ComputedStyle, CssValueError> {
    let tree = LayoutDomTree::from_resolved_root(node);
    compute_style_in_tree(sheet, &tree, node)
}

pub fn compute_style_in_tree(
    sheet: &CompiledStyleSheet,
    tree: &LayoutDomTree,
    node: &ResolvedLayoutNode,
) -> Result<ComputedStyle, CssValueError> {
    let mut style = ComputedStyle::default();

    for rule in crate::css_matching::matching_rules_in_tree(sheet, tree, node) {
        for declaration in &rule.declarations {
            style.apply(&declaration.declaration);
        }
    }

    Ok(style)
}
