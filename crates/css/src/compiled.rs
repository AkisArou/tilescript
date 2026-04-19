use selectors::parser::SelectorList;

use crate::stylo_adapter::LayoutSelectorImpl;

use crate::compile::CompiledDeclaration;

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledStyleSheet {
    pub rules: Vec<CompiledStyleRule>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledStyleRule {
    pub selectors: SelectorList<LayoutSelectorImpl>,
    pub declarations: Vec<CompiledDeclaration>,
}
