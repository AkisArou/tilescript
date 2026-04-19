use selectors::parser::SelectorList;

use crate::stylo_adapter::LayoutSelectorImpl;

use crate::compile::CompiledDeclaration;
use crate::source::CssRange;

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledStyleSheet {
    pub rules: Vec<CompiledStyleRule>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledStyleRule {
    pub selector_text: String,
    pub selector_range: CssRange,
    pub block_range: CssRange,
    pub selectors: SelectorList<LayoutSelectorImpl>,
    pub declarations: Vec<CompiledDeclarationEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledDeclarationEntry {
    pub property: String,
    pub property_range: CssRange,
    pub declaration: CompiledDeclaration,
}
