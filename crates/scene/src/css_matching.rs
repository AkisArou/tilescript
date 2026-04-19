use tilescript_core::ResolvedLayoutNode;

use crate::css::{CompiledStyleRule, CompiledStyleSheet, LayoutDomTree};

#[cfg(test)]
pub fn matching_rules<'a>(
    sheet: &'a CompiledStyleSheet,
    node: &ResolvedLayoutNode,
) -> Vec<&'a CompiledStyleRule> {
    tilescript_css::matching_rules(sheet, node)
}

pub fn matching_rules_in_tree<'a>(
    sheet: &'a CompiledStyleSheet,
    tree: &'a LayoutDomTree,
    node: &ResolvedLayoutNode,
) -> Vec<&'a CompiledStyleRule> {
    tree.find_element(node)
        .map(|element| tilescript_css::matching_rules_for_element(sheet, element))
        .unwrap_or_default()
}

#[cfg(test)]
pub fn selector_matches(
    selector: &selectors::parser::SelectorList<tilescript_css::LayoutSelectorImpl>,
    node: &ResolvedLayoutNode,
) -> bool {
    tilescript_css::selector_matches(selector, node)
}
