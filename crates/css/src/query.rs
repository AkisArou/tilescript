use hypreact_core::ResolvedLayoutNode;

use crate::{
    CompiledStyleRule, CompiledStyleSheet, LayoutDomTree, LayoutSelectorImpl,
    selector_matches_element,
};

pub fn matching_rules<'a>(
    sheet: &'a CompiledStyleSheet,
    node: &ResolvedLayoutNode,
) -> Vec<&'a CompiledStyleRule> {
    sheet.rules.iter().filter(|rule| selector_matches(&rule.selectors, node)).collect()
}

pub fn selector_matches(
    selector: &selectors::parser::SelectorList<LayoutSelectorImpl>,
    node: &ResolvedLayoutNode,
) -> bool {
    let tree = LayoutDomTree::from_resolved_root(node);
    selector_matches_element(selector, tree.root_element())
}
