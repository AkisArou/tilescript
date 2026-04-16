use tilescript_core::ResolvedLayoutNode;

use crate::css::apply::ApplyCompiledDeclaration;
use crate::css::{CompiledStyleSheet, ComputedStyle, CssValueError};

pub fn compute_style(
    sheet: &CompiledStyleSheet,
    node: &ResolvedLayoutNode,
) -> Result<ComputedStyle, CssValueError> {
    let mut style = ComputedStyle::default();

    for rule in crate::css_matching::matching_rules(sheet, node) {
        for declaration in &rule.declarations {
            style.apply(declaration.clone());
        }
    }

    Ok(style)
}
