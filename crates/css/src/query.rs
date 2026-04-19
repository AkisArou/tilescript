use tilescript_core::ResolvedLayoutNode;

use crate::{
    CompiledStyleRule, CompiledStyleSheet, LayoutDomTree, LayoutElement, LayoutSelectorImpl,
    selector_matches_element,
};

pub fn matching_rules<'a>(
    sheet: &'a CompiledStyleSheet,
    node: &ResolvedLayoutNode,
) -> Vec<&'a CompiledStyleRule> {
    let tree = LayoutDomTree::from_resolved_root(node);
    matching_rules_for_element(sheet, tree.root_element())
}

pub fn matching_rules_for_element<'a>(
    sheet: &'a CompiledStyleSheet,
    element: LayoutElement<'_>,
) -> Vec<&'a CompiledStyleRule> {
    let mut matches = Vec::new();
    for rule in &sheet.rules {
        collect_matching_rules(rule, element, &mut matches);
    }
    matches
}

pub fn selector_matches(
    selector: &selectors::parser::SelectorList<LayoutSelectorImpl>,
    node: &ResolvedLayoutNode,
) -> bool {
    let tree = LayoutDomTree::from_resolved_root(node);
    selector_matches_element(selector, tree.root_element())
}

fn collect_matching_rules<'a>(
    rule: &'a CompiledStyleRule,
    element: LayoutElement<'_>,
    matches: &mut Vec<&'a CompiledStyleRule>,
) {
    if selector_matches_element(&rule.selectors, element) {
        matches.push(rule);
    }

    for child in &rule.children {
        collect_matching_rules(child, element, matches);
    }
}
