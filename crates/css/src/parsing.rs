use cssparser::{
    AtRuleParser, CowRcStr, DeclarationParser, Parser, ParserInput, QualifiedRuleParser,
    RuleBodyItemParser, RuleBodyParser, StyleSheetParser,
};

use crate::selector_matches;
use crate::stylo_adapter::{
    LayoutSelectorImpl, LayoutSelectorParser, parse_selector_list_from_parser,
};

use crate::compile::{CssValueError, compile_declaration, components_to_text};
use crate::compiled::{CompiledDeclarationEntry, CompiledStyleRule, CompiledStyleSheet};
use crate::language::property_spec;
use crate::parse_values::{CssValue, ParsedDeclaration};
use crate::source::{CssRange, SourceMap, leading_trimmed_len, trailing_trimmed_len};
use crate::tokenizer::parse_component_values;

struct ParsedSelectorPrelude {
    selector_text: String,
    selector_range: CssRange,
    selectors: selectors::parser::SelectorList<LayoutSelectorImpl>,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum CssParseError {
    #[error("unsupported at-rule `{name}`")]
    UnsupportedAtRule { name: String },
    #[error("unsupported selector `{selector}`")]
    UnsupportedSelector { selector: String },
    #[error("unsupported property `{property}`")]
    UnsupportedProperty { property: String },
    #[error("invalid CSS near line {line}, column {column}")]
    InvalidSyntax { line: u32, column: u32 },
    #[error(transparent)]
    CssValue(#[from] CssValueError),
}

struct LayoutCssRuleParser<'a> {
    source_map: &'a SourceMap<'a>,
}

struct LayoutDeclarationParser<'a> {
    source_map: &'a SourceMap<'a>,
}

pub fn parse_stylesheet(input: &str) -> Result<CompiledStyleSheet, CssParseError> {
    let source_map = SourceMap::new(input);
    let mut input_buf = ParserInput::new(input);
    let mut parser_input = Parser::new(&mut input_buf);
    let mut parser = LayoutCssRuleParser { source_map: &source_map };
    let mut rules = Vec::new();

    for item in StyleSheetParser::new(&mut parser_input, &mut parser) {
        match item {
            Ok(rule) => rules.push(rule),
            Err((err, _slice)) => {
                let location = err.location;
                return Err(match err.kind {
                    cssparser::ParseErrorKind::Custom(error) => error,
                    _ => CssParseError::InvalidSyntax {
                        line: location.line,
                        column: location.column,
                    },
                });
            }
        }
    }

    Ok(CompiledStyleSheet { rules })
}

impl<'a, 'i> AtRuleParser<'i> for LayoutCssRuleParser<'a> {
    type Prelude = ();
    type AtRule = CompiledStyleRule;
    type Error = CssParseError;

    fn parse_prelude<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, cssparser::ParseError<'i, Self::Error>> {
        Err(input.new_custom_error(CssParseError::UnsupportedAtRule { name: name.to_string() }))
    }
}

impl<'a, 'i> QualifiedRuleParser<'i> for LayoutCssRuleParser<'a> {
    type Prelude = ParsedSelectorPrelude;
    type QualifiedRule = CompiledStyleRule;
    type Error = CssParseError;

    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, cssparser::ParseError<'i, Self::Error>> {
        let start = input.state();
        let parser = LayoutSelectorParser;
        let parsed = parse_selector_list_from_parser(&parser, input).map_err(|_| {
            let selector = input.slice_from(start.position()).trim().to_string();
            input.new_custom_error(CssParseError::UnsupportedSelector { selector })
        })?;

        let selector_slice = input.slice_from(start.position());
        let selector = selector_slice.trim().to_string();
        let selector_start = start.position().byte_index() + leading_trimmed_len(selector_slice);
        let selector_end = input.position().byte_index() - trailing_trimmed_len(selector_slice);

        if selector_matches_slot(&parsed) {
            return Err(input.new_custom_error(CssParseError::UnsupportedSelector { selector }));
        }

        Ok(ParsedSelectorPrelude {
            selector_text: selector,
            selector_range: self.source_map.range(selector_start, selector_end),
            selectors: parsed,
        })
    }

    fn parse_block<'t>(
        &mut self,
        prelude: Self::Prelude,
        _start: &cssparser::ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::QualifiedRule, cssparser::ParseError<'i, Self::Error>> {
        let block_start = input.position().byte_index().saturating_sub(1);
        let mut declaration_parser = LayoutDeclarationParser { source_map: self.source_map };
        let mut declarations = Vec::new();

        for item in RuleBodyParser::new(input, &mut declaration_parser) {
            match item {
                Ok(declaration) => declarations.push(declaration),
                Err((err, _slice)) => {
                    return Err(input.new_custom_error(match err.kind {
                        cssparser::ParseErrorKind::Custom(error) => error,
                        _ => CssParseError::InvalidSyntax {
                            line: err.location.line,
                            column: err.location.column,
                        },
                    }));
                }
            }
        }

        Ok(CompiledStyleRule {
            selector_text: prelude.selector_text,
            selector_range: prelude.selector_range,
            block_range: self.source_map.range(block_start, input.position().byte_index() + 1),
            selectors: prelude.selectors,
            declarations,
        })
    }
}

impl<'a, 'i> DeclarationParser<'i> for LayoutDeclarationParser<'a> {
    type Declaration = CompiledDeclarationEntry;
    type Error = CssParseError;

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        declaration_start: &cssparser::ParserState,
    ) -> Result<Self::Declaration, cssparser::ParseError<'i, Self::Error>> {
        let property = name.to_string().to_ascii_lowercase();
        if property_spec(&property).is_none() {
            return Err(input.new_custom_error(CssParseError::UnsupportedProperty { property }));
        }

        let property_start = declaration_start.position().byte_index();
        let property_end = property_start + name.len();
        let components = parse_component_values(input)?;
        let value_text = components_to_text(&components);
        let parsed = ParsedDeclaration {
            property: property.clone(),
            value: CssValue { text: value_text, components },
        };

        let declaration = compile_declaration(&parsed)
            .map_err(|error| input.new_custom_error(CssParseError::CssValue(error)))?;

        Ok(CompiledDeclarationEntry {
            property,
            property_range: self.source_map.range(property_start, property_end),
            declaration,
        })
    }
}

impl<'a, 'i> AtRuleParser<'i> for LayoutDeclarationParser<'a> {
    type Prelude = ();
    type AtRule = CompiledDeclarationEntry;
    type Error = CssParseError;
}

impl<'a, 'i> QualifiedRuleParser<'i> for LayoutDeclarationParser<'a> {
    type Prelude = ();
    type QualifiedRule = CompiledDeclarationEntry;
    type Error = CssParseError;
}

impl<'a, 'i> RuleBodyItemParser<'i, CompiledDeclarationEntry, CssParseError>
    for LayoutDeclarationParser<'a>
{
    fn parse_declarations(&self) -> bool {
        true
    }

    fn parse_qualified(&self) -> bool {
        false
    }
}

fn selector_matches_slot(selectors: &selectors::parser::SelectorList<LayoutSelectorImpl>) -> bool {
    selector_matches(selectors, &synthetic_slot_node())
}

fn synthetic_slot_node() -> tilescript_core::ResolvedLayoutNode {
    tilescript_core::ResolvedLayoutNode::Content {
        meta: tilescript_core::LayoutNodeMeta {
            name: Some("slot".to_string()),
            ..tilescript_core::LayoutNodeMeta::default()
        },
        kind: tilescript_core::RuntimeContentKind::Container,
        text: None,
        children: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_real_slot_selectors() {
        let parsed = parse_stylesheet("slot { display: flex; }");
        assert!(matches!(parsed, Err(CssParseError::UnsupportedSelector { .. })));
    }

    #[test]
    fn does_not_reject_class_names_containing_slot() {
        let parsed = parse_stylesheet(".slot-item { display: flex; }");
        assert!(parsed.is_ok());
    }

    #[test]
    fn rejects_window_titlebar_selectors() {
        let parsed = parse_stylesheet("window::titlebar { display: flex; }");
        assert!(matches!(parsed, Err(CssParseError::UnsupportedSelector { .. })));
    }
}
