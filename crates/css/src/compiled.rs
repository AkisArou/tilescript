use selectors::parser::SelectorList;

use crate::stylo_adapter::LayoutSelectorImpl;

use crate::compile::CompiledDeclaration;

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledStyleSheet {
    pub rules: Vec<CompiledStyleRule>,
    pub keyframes: Vec<CompiledKeyframesRule>,
}

impl CompiledStyleSheet {
    pub fn keyframes(&self, name: &str) -> Option<&CompiledKeyframesRule> {
        self.keyframes.iter().find(|rule| rule.name == name)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledStyleRule {
    pub selectors: SelectorList<LayoutSelectorImpl>,
    pub declarations: Vec<CompiledDeclaration>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledKeyframesRule {
    pub name: String,
    pub steps: Vec<CompiledKeyframeStep>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledKeyframeStep {
    pub offset: f32,
    pub declarations: Vec<CompiledDeclaration>,
}
