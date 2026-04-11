use hypreact_core::ResolvedLayoutNode;

use crate::css::{CompiledStyleRule, CompiledStyleSheet};

pub fn matching_rules<'a>(
    sheet: &'a CompiledStyleSheet,
    node: &ResolvedLayoutNode,
) -> Vec<&'a CompiledStyleRule> {
    hypreact_css::matching_rules(sheet, node)
}

#[cfg(test)]
pub fn selector_matches(
    selector: &selectors::parser::SelectorList<hypreact_css::LayoutSelectorImpl>,
    node: &ResolvedLayoutNode,
) -> bool {
    hypreact_css::selector_matches(selector, node)
}
