use cssparser::{
    AtRuleParser, CowRcStr, Parser, ParserInput, QualifiedRuleParser, StyleSheetParser,
};

use crate::selector_matches;
use crate::stylo_adapter::{
    LayoutSelectorImpl, LayoutSelectorParser, parse_selector_list_from_parser,
};

use crate::compile::{CssValueError, compile_declaration, compile_declaration_from_value};
use crate::compiled::{
    CompiledKeyframeStep, CompiledKeyframesRule, CompiledStyleRule, CompiledStyleSheet,
};
use crate::grid::parse_grid_fallback_declarations;
use crate::language::is_supported_property;
use crate::parse_values::{CssValue, ParsedDeclaration};
use crate::tokenizer::parse_value_tokens;
use style::parser::ParserContext;
use style::properties::declaration_block::parse_property_declaration_list;
use style::stylesheets::{CssRuleType, Origin, UrlExtraData};
use style_traits::ParsingMode;
use style_traits::values::ToCss;

struct ParsedSelectorPrelude {
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

#[derive(Default)]
struct LayoutCssRuleParser;

pub fn parse_stylesheet(input: &str) -> Result<CompiledStyleSheet, CssParseError> {
    let (sanitized, keyframes) = extract_keyframes_and_strip(input)?;
    let mut input_buf = ParserInput::new(&sanitized);
    let mut parser_input = Parser::new(&mut input_buf);
    let mut parser = LayoutCssRuleParser;
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

    Ok(CompiledStyleSheet { rules, keyframes })
}

impl<'i> AtRuleParser<'i> for LayoutCssRuleParser {
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

impl<'i> QualifiedRuleParser<'i> for LayoutCssRuleParser {
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

        let selector = input.slice_from(start.position()).trim().to_string();

        if selector_matches_slot(&parsed) {
            return Err(input.new_custom_error(CssParseError::UnsupportedSelector { selector }));
        }

        Ok(ParsedSelectorPrelude { selectors: parsed })
    }

    fn parse_block<'t>(
        &mut self,
        prelude: Self::Prelude,
        _start: &cssparser::ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::QualifiedRule, cssparser::ParseError<'i, Self::Error>> {
        let url_data = UrlExtraData(url::Url::parse("about:blank").unwrap().into());
        let context = ParserContext::new(
            Origin::Author,
            &url_data,
            Some(CssRuleType::Style),
            ParsingMode::DEFAULT,
            style::context::QuirksMode::NoQuirks,
            Default::default(),
            None,
            None,
        );
        let block_start = input.state();
        let _ = parse_property_declaration_list(&context, input, &[]);
        let raw_block = input.slice_from(block_start.position()).trim().to_string();
        let declarations = compile_declarations_from_raw_block(&raw_block)
            .map_err(|error| input.new_custom_error(error))?;

        Ok(CompiledStyleRule { selectors: prelude.selectors, declarations })
    }
}

fn extract_keyframes_and_strip(
    input: &str,
) -> Result<(String, Vec<CompiledKeyframesRule>), CssParseError> {
    let mut result = String::with_capacity(input.len());
    let mut keyframes = Vec::new();
    let mut index = 0;

    while let Some(relative) = input[index..].find("@keyframes") {
        let start = index + relative;
        result.push_str(&input[index..start]);

        let open_brace_offset =
            input[start..].find('{').ok_or(CssParseError::InvalidSyntax { line: 1, column: 1 })?;
        let open_brace = start + open_brace_offset;
        let end = matching_brace_end(input, open_brace)
            .ok_or(CssParseError::InvalidSyntax { line: 1, column: 1 })?;
        let name = input[start + "@keyframes".len()..open_brace]
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        if name.is_empty() {
            return Err(CssParseError::InvalidSyntax { line: 1, column: 1 });
        }
        let body = &input[open_brace + 1..end - 1];
        keyframes.push(parse_keyframes_rule(name, body)?);
        index = end;
    }

    result.push_str(&input[index..]);

    Ok((result, keyframes))
}

fn compile_declarations_from_raw_block(
    raw_block: &str,
) -> Result<Vec<crate::compile::CompiledDeclaration>, CssParseError> {
    let url_data = UrlExtraData(url::Url::parse("about:blank").unwrap().into());
    let context = ParserContext::new(
        Origin::Author,
        &url_data,
        Some(CssRuleType::Style),
        ParsingMode::DEFAULT,
        style::context::QuirksMode::NoQuirks,
        Default::default(),
        None,
        None,
    );

    let mut input_buf = ParserInput::new(raw_block);
    let mut parser = Parser::new(&mut input_buf);
    let block = parse_property_declaration_list(&context, &mut parser, &[]);
    let mut declarations = Vec::new();
    for declaration in block.normal_declaration_iter() {
        let property = declaration.id().to_css_string();
        if !is_supported_property(&property) {
            if is_ignored_background_expansion(&property) {
                continue;
            }
            return Err(CssParseError::UnsupportedProperty { property });
        }

        if let Some(compiled) = crate::stylo_compile::compile_stylo_declaration(declaration)? {
            declarations.push(compiled);
            continue;
        }

        let mut value = String::new();
        declaration
            .to_css(&mut value)
            .map_err(|_| CssParseError::InvalidSyntax { line: 1, column: 1 })?;

        let parsed = ParsedDeclaration {
            property,
            value: CssValue { text: value.clone(), components: parse_value_tokens(&value)? },
        };
        let compiled = compile_declaration(&parsed).map_err(CssParseError::CssValue)?;
        declarations.push(compiled);
    }

    let fallback_declarations = fallback_declarations(raw_block)?;

    if fallback_declarations.iter().any(|declaration| declaration.property == "appearance")
        && !declarations.iter().any(|declaration| {
            matches!(declaration, crate::compile::CompiledDeclaration::Appearance(_))
        })
    {
        let fallback = parse_grid_fallback_declarations(raw_block)?;
        declarations.extend(fallback.into_iter().filter(|declaration| {
            matches!(declaration, crate::compile::CompiledDeclaration::Appearance(_))
        }));
    }

    if fallback_declarations.iter().any(|declaration| declaration.property == "background")
        && !declarations.iter().any(|declaration| {
            matches!(declaration, crate::compile::CompiledDeclaration::Background(_))
        })
    {
        let fallback = parse_grid_fallback_declarations(raw_block)?;
        declarations.extend(fallback.into_iter().filter(|declaration| {
            matches!(declaration, crate::compile::CompiledDeclaration::Background(_))
        }));
    }

    if fallback_declarations.iter().any(|declaration| declaration.property == "border-color")
        && !declarations.iter().any(|declaration| {
            matches!(declaration, crate::compile::CompiledDeclaration::BorderColor(_))
        })
    {
        append_fallback_declarations(&fallback_declarations, &mut declarations, &["border-color"])?;
    }

    append_fallback_declarations(
        &fallback_declarations,
        &mut declarations,
        &["border-radius", "box-shadow"],
    )?;

    if declarations.is_empty() && needs_grid_fallback(&fallback_declarations) {
        declarations = parse_grid_fallback_declarations(raw_block)?;
    }

    append_custom_tilescript_declarations(&fallback_declarations, &mut declarations)?;

    Ok(declarations)
}

fn append_custom_tilescript_declarations(
    fallback_declarations: &[ParsedDeclaration],
    declarations: &mut Vec<crate::compile::CompiledDeclaration>,
) -> Result<(), CssParseError> {
    for declaration in fallback_declarations {
        if !declaration.property.starts_with("-tilescript-") {
            continue;
        }

        if !is_supported_property(&declaration.property) {
            return Err(CssParseError::UnsupportedProperty {
                property: declaration.property.clone(),
            });
        }

        let already_present = declarations.iter().any(|compiled| {
            compiled.canonical_property_name() == Some(declaration.property.as_str())
        });
        if already_present {
            continue;
        }

        declarations.push(
            compile_declaration_from_value(&declaration.property, &declaration.value)
                .map_err(CssParseError::CssValue)?,
        );
    }

    Ok(())
}

fn parse_keyframes_rule(name: String, body: &str) -> Result<CompiledKeyframesRule, CssParseError> {
    let mut steps = Vec::new();
    let mut index = 0;

    while let Some(relative) = body[index..].find('{') {
        let block_start = index + relative;
        let selector_text = body[index..block_start].trim();
        let block_end = matching_brace_end(body, block_start)
            .ok_or(CssParseError::InvalidSyntax { line: 1, column: 1 })?;
        let declarations =
            compile_declarations_from_raw_block(&body[block_start + 1..block_end - 1])?;

        for selector in
            selector_text.split(',').map(str::trim).filter(|selector| !selector.is_empty())
        {
            steps.push(CompiledKeyframeStep {
                offset: parse_keyframe_offset(selector)?,
                declarations: declarations.clone(),
            });
        }

        index = block_end;
    }

    steps.sort_by(|left, right| {
        left.offset.partial_cmp(&right.offset).unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(CompiledKeyframesRule { name, steps })
}

fn parse_keyframe_offset(selector: &str) -> Result<f32, CssParseError> {
    match selector {
        "from" => Ok(0.0),
        "to" => Ok(1.0),
        _ => selector
            .strip_suffix('%')
            .and_then(|value| value.trim().parse::<f32>().ok())
            .map(|value| (value / 100.0).clamp(0.0, 1.0))
            .ok_or(CssParseError::UnsupportedSelector { selector: selector.to_string() }),
    }
}

fn matching_brace_end(input: &str, open_brace: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (offset, ch) in input[open_brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open_brace + offset + ch.len_utf8());
                }
            }
            _ => {}
        }
    }
    None
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

fn is_ignored_background_expansion(property: &str) -> bool {
    matches!(
        property,
        "background-position-x"
            | "background-position-y"
            | "background-repeat"
            | "background-attachment"
            | "background-image"
            | "background-size"
            | "background-origin"
            | "background-clip"
            | "border-image-source"
            | "border-image-slice"
            | "border-image-width"
            | "border-image-outset"
            | "border-image-repeat"
            | "border-top-left-radius"
            | "border-top-right-radius"
            | "border-bottom-right-radius"
            | "border-bottom-left-radius"
    )
}

fn append_fallback_declarations(
    fallback_declarations: &[ParsedDeclaration],
    declarations: &mut Vec<crate::compile::CompiledDeclaration>,
    properties: &[&str],
) -> Result<(), CssParseError> {
    for property in properties {
        let property_name = *property;
        let already_present =
            declarations.iter().any(|declaration| match (property_name, declaration) {
                ("border-radius", crate::compile::CompiledDeclaration::BorderRadius(_)) => true,
                ("border-color", crate::compile::CompiledDeclaration::BorderColor(_)) => true,
                ("box-shadow", crate::compile::CompiledDeclaration::BoxShadow(_)) => true,
                _ => false,
            });
        if already_present {
            continue;
        }

        if let Some(value) = fallback_declarations
            .iter()
            .find(|declaration| declaration.property == property_name)
            .map(|declaration| declaration.value.clone())
        {
            let compiled = compile_declaration_from_value(property_name, &value)
                .map_err(CssParseError::CssValue)?;
            declarations.push(compiled);
        }
    }

    Ok(())
}

fn needs_grid_fallback(fallback_declarations: &[ParsedDeclaration]) -> bool {
    fallback_declarations.iter().any(|declaration| {
        matches!(
            declaration.property.as_str(),
            "grid-template-rows"
                | "grid-template-columns"
                | "grid-template-areas"
                | "grid-row"
                | "grid-column"
                | "grid-row-start"
                | "grid-row-end"
                | "grid-column-start"
                | "grid-column-end"
                | "grid-auto-rows"
                | "grid-auto-columns"
                | "grid-auto-flow"
        )
    })
}

fn fallback_declarations(raw_block: &str) -> Result<Vec<ParsedDeclaration>, CssParseError> {
    let mut declarations = Vec::new();
    let mut start = 0usize;
    let mut offset = 0usize;
    let mut paren_depth = 0i32;
    let mut bracket_depth = 0i32;
    let bytes = raw_block.as_bytes();

    while offset < raw_block.len() {
        if let Some(comment_end) = starts_comment(bytes, offset) {
            offset = comment_end;
            continue;
        }
        if let Some(string_end) = starts_string(raw_block, offset) {
            offset = string_end;
            continue;
        }

        match bytes[offset] {
            b'(' => paren_depth += 1,
            b')' => paren_depth -= 1,
            b'[' => bracket_depth += 1,
            b']' => bracket_depth -= 1,
            b';' if paren_depth == 0 && bracket_depth == 0 => {
                if let Some(declaration) =
                    parse_fallback_declaration_segment(raw_block, start, offset)?
                {
                    declarations.push(declaration);
                }
                start = offset + 1;
            }
            _ => {}
        }

        offset += 1;
    }

    if let Some(declaration) =
        parse_fallback_declaration_segment(raw_block, start, raw_block.len())?
    {
        declarations.push(declaration);
    }

    Ok(declarations)
}

fn parse_fallback_declaration_segment(
    raw_block: &str,
    start: usize,
    end: usize,
) -> Result<Option<ParsedDeclaration>, CssParseError> {
    let Some(trimmed_start) = skip_ws_and_comments(raw_block, start, end) else {
        return Ok(None);
    };
    let segment = &raw_block[trimmed_start..end];
    let trimmed_end = end - trailing_trimmed_len(segment);
    if trimmed_start >= trimmed_end {
        return Ok(None);
    }

    let Some(colon) = find_top_level_colon(raw_block, trimmed_start, trimmed_end) else {
        return Ok(None);
    };
    let property = raw_block[trimmed_start..colon].trim().to_ascii_lowercase();
    if property.is_empty() {
        return Ok(None);
    }

    let value_text = raw_block[colon + 1..trimmed_end].trim().to_string();
    let components = parse_value_tokens(&value_text)?;

    Ok(Some(ParsedDeclaration { property, value: CssValue { text: value_text, components } }))
}

fn find_top_level_colon(source: &str, start: usize, end: usize) -> Option<usize> {
    let mut offset = start;
    let mut paren_depth = 0i32;
    let mut bracket_depth = 0i32;
    let bytes = source.as_bytes();

    while offset < end {
        if let Some(comment_end) = starts_comment(bytes, offset) {
            offset = comment_end;
            continue;
        }
        if let Some(string_end) = starts_string(source, offset) {
            offset = string_end;
            continue;
        }

        match bytes[offset] {
            b'(' => paren_depth += 1,
            b')' => paren_depth -= 1,
            b'[' => bracket_depth += 1,
            b']' => bracket_depth -= 1,
            b':' if paren_depth == 0 && bracket_depth == 0 => return Some(offset),
            _ => {}
        }

        offset += 1;
    }

    None
}

fn starts_comment(bytes: &[u8], offset: usize) -> Option<usize> {
    if bytes.get(offset) == Some(&b'/') && bytes.get(offset + 1) == Some(&b'*') {
        let mut end = offset + 2;
        while end + 1 < bytes.len() {
            if bytes[end] == b'*' && bytes[end + 1] == b'/' {
                return Some(end + 2);
            }
            end += 1;
        }
        return Some(bytes.len());
    }

    None
}

fn starts_string(source: &str, offset: usize) -> Option<usize> {
    let quote = match source.as_bytes().get(offset) {
        Some(b'\'') => b'\'',
        Some(b'"') => b'"',
        _ => return None,
    };

    let mut escaped = false;
    let mut index = offset + 1;
    let bytes = source.as_bytes();

    while index < bytes.len() {
        let byte = bytes[index];
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == quote {
            return Some(index + 1);
        }
        index += 1;
    }

    Some(bytes.len())
}

fn trailing_trimmed_len(input: &str) -> usize {
    input.len() - input.trim_end().len()
}

fn skip_ws_and_comments(source: &str, mut offset: usize, end: usize) -> Option<usize> {
    let bytes = source.as_bytes();

    while offset < end {
        if bytes[offset].is_ascii_whitespace() {
            offset += 1;
            continue;
        }
        if let Some(comment_end) = starts_comment(bytes, offset) {
            offset = comment_end.min(end);
            continue;
        }
        return Some(offset);
    }

    None
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
        let parsed = parse_stylesheet("window::titlebar { text-align: center; }");
        assert!(matches!(parsed, Err(CssParseError::UnsupportedSelector { .. })));
    }

    #[test]
    fn fallback_property_scan_ignores_comments_and_strings() {
        let parsed = fallback_declarations(
            "color: red; /* border-color: hotpink; */ background: rgb(1, 2, 3);",
        )
        .unwrap();

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].property, "color");
        assert_eq!(parsed[1].property, "background");
    }
}
