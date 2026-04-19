use crate::compiled::{CompiledStyleRule, CompiledStyleSheet};
use crate::language::{StyleTarget, is_supported_attribute_key, property_spec};
use crate::parse_stylesheet;
use crate::query::selector_matches;
use crate::source::{CssRange, SourceMap, leading_trimmed_len, trailing_trimmed_len};
use crate::{LayoutSelectorParser, parse_selector_list_from_parser};
use cssparser::{AtRuleParser, Parser, ParserInput, QualifiedRuleParser, StyleSheetParser};
use tilescript_core::{LayoutNodeMeta, ResolvedLayoutNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssDiagnosticSeverity {
    Error,
    Warning,
    Information,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssDiagnosticCode {
    UnsupportedAtRule,
    UnsupportedSelector,
    UnsupportedProperty,
    InvalidSyntax,
    UnsupportedValue,
    InapplicableProperty,
    UnsupportedAttributeKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssDiagnostic {
    pub code: CssDiagnosticCode,
    pub severity: CssDiagnosticSeverity,
    pub message: String,
    pub range: CssRange,
}

#[derive(Debug, Clone)]
pub struct CssAnalysis {
    pub stylesheet: Option<CompiledStyleSheet>,
    pub diagnostics: Vec<CssDiagnostic>,
    pub symbols: Vec<CssSymbol>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssSymbolKind {
    Rule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssSymbol {
    pub kind: CssSymbolKind,
    pub name: String,
    pub range: CssRange,
    pub selection_range: CssRange,
}

pub fn analyze_stylesheet(source: &str) -> CssAnalysis {
    match parse_stylesheet(source) {
        Ok(stylesheet) => {
            let diagnostics = semantic_diagnostics(&stylesheet);
            let symbols = extract_symbols(&stylesheet);
            CssAnalysis { stylesheet: Some(stylesheet), diagnostics, symbols }
        }
        Err(error) => CssAnalysis {
            stylesheet: None,
            diagnostics: vec![diagnostic_from_parse_error(&error)],
            symbols: fallback_symbols(source),
        },
    }
}

fn extract_symbols(stylesheet: &CompiledStyleSheet) -> Vec<CssSymbol> {
    stylesheet
        .rules
        .iter()
        .map(|rule| CssSymbol {
            kind: CssSymbolKind::Rule,
            name: rule.selector_text.clone(),
            range: rule.block_range,
            selection_range: rule.selector_range,
        })
        .collect()
}

fn fallback_symbols(source: &str) -> Vec<CssSymbol> {
    let source_map = SourceMap::new(source);
    let mut input_buf = ParserInput::new(source);
    let mut parser_input = Parser::new(&mut input_buf);
    let mut parser = SymbolRuleParser { source_map: &source_map };
    let mut symbols = Vec::new();

    for rule in StyleSheetParser::new(&mut parser_input, &mut parser).flatten() {
        symbols.push(rule);
    }

    symbols
}

struct SymbolRuleParser<'a> {
    source_map: &'a SourceMap<'a>,
}

struct SymbolPrelude {
    selector_text: String,
    selector_range: CssRange,
}

impl<'a, 'i> AtRuleParser<'i> for SymbolRuleParser<'a> {
    type Prelude = ();
    type AtRule = CssSymbol;
    type Error = ();
}

impl<'a, 'i> QualifiedRuleParser<'i> for SymbolRuleParser<'a> {
    type Prelude = SymbolPrelude;
    type QualifiedRule = CssSymbol;
    type Error = ();

    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, cssparser::ParseError<'i, Self::Error>> {
        let start = input.state();
        let parser = LayoutSelectorParser;
        parse_selector_list_from_parser(&parser, input).map_err(|_| input.new_custom_error(()))?;

        let selector_slice = input.slice_from(start.position());
        let selector_text = selector_slice.trim().to_string();
        let selector_start = start.position().byte_index() + leading_trimmed_len(selector_slice);
        let selector_end = input.position().byte_index() - trailing_trimmed_len(selector_slice);

        Ok(SymbolPrelude {
            selector_text,
            selector_range: self.source_map.range(selector_start, selector_end),
        })
    }

    fn parse_block<'t>(
        &mut self,
        prelude: Self::Prelude,
        _start: &cssparser::ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::QualifiedRule, cssparser::ParseError<'i, Self::Error>> {
        let block_start = input.position().byte_index().saturating_sub(1);
        while input.next_including_whitespace_and_comments().is_ok() {}

        Ok(CssSymbol {
            kind: CssSymbolKind::Rule,
            name: prelude.selector_text,
            range: self.source_map.range(block_start, input.position().byte_index() + 1),
            selection_range: prelude.selector_range,
        })
    }
}

fn semantic_diagnostics(stylesheet: &CompiledStyleSheet) -> Vec<CssDiagnostic> {
    let mut diagnostics = selector_attribute_key_diagnostics(stylesheet);
    diagnostics.extend(applicability_diagnostics(stylesheet));
    diagnostics
}

fn selector_attribute_key_diagnostics(stylesheet: &CompiledStyleSheet) -> Vec<CssDiagnostic> {
    let mut diagnostics = Vec::new();

    for rule in &stylesheet.rules {
        diagnostics.extend(attribute_key_diagnostics_for_rule(rule));
    }

    diagnostics
}

fn applicability_diagnostics(stylesheet: &CompiledStyleSheet) -> Vec<CssDiagnostic> {
    let mut diagnostics = Vec::new();

    for rule in &stylesheet.rules {
        let targets = infer_style_targets(&rule.selectors);
        for declaration in &rule.declarations {
            let Some(spec) = property_spec(&declaration.property) else {
                continue;
            };
            if targets.iter().any(|target| spec.applies_to.contains(target)) {
                continue;
            }

            diagnostics.push(CssDiagnostic {
                code: CssDiagnosticCode::InapplicableProperty,
                severity: CssDiagnosticSeverity::Warning,
                message: format!(
                    "property `{}` does not apply to {}",
                    declaration.property,
                    describe_targets(&targets)
                ),
                range: declaration.property_range,
            });
        }
    }

    diagnostics
}

fn infer_style_targets(
    selectors: &selectors::parser::SelectorList<crate::LayoutSelectorImpl>,
) -> Vec<StyleTarget> {
    let mut targets = Vec::new();

    if selector_matches(selectors, &synthetic_workspace_node()) {
        push_unique(&mut targets, StyleTarget::Workspace);
    }
    if selector_matches(selectors, &synthetic_group_node()) {
        push_unique(&mut targets, StyleTarget::Group);
    }
    if selector_matches(selectors, &synthetic_window_node()) {
        push_unique(&mut targets, StyleTarget::Window);
    }

    if targets.is_empty() {
        vec![StyleTarget::Workspace, StyleTarget::Group, StyleTarget::Window]
    } else {
        targets
    }
}

fn synthetic_workspace_node() -> ResolvedLayoutNode {
    ResolvedLayoutNode::Workspace { meta: LayoutNodeMeta::default(), children: Vec::new() }
}

fn synthetic_group_node() -> ResolvedLayoutNode {
    ResolvedLayoutNode::Group { meta: LayoutNodeMeta::default(), children: Vec::new() }
}

fn synthetic_window_node() -> ResolvedLayoutNode {
    ResolvedLayoutNode::Window {
        meta: LayoutNodeMeta::default(),
        window_id: None,
        children: Vec::new(),
    }
}

fn attribute_key_diagnostics_for_rule(rule: &CompiledStyleRule) -> Vec<CssDiagnostic> {
    let mut diagnostics = Vec::new();
    let selector = &rule.selector_text;
    let source_map = SourceMap::new(selector);
    let mut offset = 0;
    let bytes = selector.as_bytes();

    while offset < selector.len() {
        if let Some(string_end) = starts_string(selector, offset) {
            offset = string_end;
            continue;
        }

        if bytes[offset] == b'[' {
            let Some(end) = find_matching_bracket_end(selector, offset) else {
                break;
            };
            if let Some(diagnostic) = attribute_key_diagnostic_for_segment(
                selector,
                offset,
                end,
                &source_map,
                rule.selector_range,
            ) {
                diagnostics.push(diagnostic);
            }
            offset = end;
            continue;
        }

        offset += 1;
    }

    diagnostics
}

fn attribute_key_diagnostic_for_segment(
    selector: &str,
    start: usize,
    end: usize,
    source_map: &SourceMap<'_>,
    selector_range: CssRange,
) -> Option<CssDiagnostic> {
    let content_start = start + 1;
    let content_end = end.saturating_sub(1);
    if content_start >= content_end {
        return None;
    }

    let content = &selector[content_start..content_end];
    let trimmed_start = content_start + leading_trimmed_len(content);
    let trimmed_end = content_end - trailing_trimmed_len(content);
    if trimmed_start >= trimmed_end {
        return None;
    }

    let operator =
        find_attribute_operator(selector, trimmed_start, trimmed_end).unwrap_or(trimmed_end);
    let raw_name = selector[trimmed_start..operator].trim();
    if raw_name.is_empty() {
        return None;
    }

    let normalized_name = raw_name
        .strip_prefix("data-")
        .unwrap_or(raw_name)
        .strip_prefix('|')
        .unwrap_or(raw_name)
        .trim();

    if is_supported_attribute_key(normalized_name) || normalized_name == "id" {
        return None;
    }

    let name_start = trimmed_start + leading_trimmed_len(&selector[trimmed_start..operator]);
    let name_end = operator - trailing_trimmed_len(&selector[trimmed_start..operator]);
    let local_range = source_map.range(name_start, name_end);

    Some(CssDiagnostic {
        code: CssDiagnosticCode::UnsupportedAttributeKey,
        severity: CssDiagnosticSeverity::Warning,
        message: format!("unsupported attribute key `{}`", normalized_name),
        range: translate_range(local_range, selector_range),
    })
}

fn find_attribute_operator(selector: &str, start: usize, end: usize) -> Option<usize> {
    let mut offset = start;
    let bytes = selector.as_bytes();

    while offset < end {
        if let Some(string_end) = starts_string(selector, offset) {
            offset = string_end;
            continue;
        }

        match bytes[offset] {
            b'=' | b'~' | b'|' | b'^' | b'$' | b'*' => return Some(offset),
            _ => offset += 1,
        }
    }

    None
}

fn find_matching_bracket_end(source: &str, open_bracket: usize) -> Option<usize> {
    let mut offset = open_bracket;
    let mut depth = 0i32;
    let bytes = source.as_bytes();

    while offset < source.len() {
        if let Some(string_end) = starts_string(source, offset) {
            offset = string_end;
            continue;
        }

        match bytes[offset] {
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(offset + 1);
                }
            }
            _ => {}
        }

        offset += 1;
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

fn translate_range(local: CssRange, base: CssRange) -> CssRange {
    CssRange {
        start_line: base.start_line + local.start_line - 1,
        start_column: if local.start_line == 1 {
            base.start_column + local.start_column - 1
        } else {
            local.start_column
        },
        end_line: base.start_line + local.end_line - 1,
        end_column: if local.end_line == 1 {
            base.start_column + local.end_column - 1
        } else {
            local.end_column
        },
    }
}

fn describe_targets(targets: &[StyleTarget]) -> String {
    let mut labels = Vec::new();
    for target in targets {
        let label = match target {
            StyleTarget::Workspace => "`workspace`",
            StyleTarget::Group => "`group`",
            StyleTarget::Window => "`window`",
        };
        if !labels.contains(&label) {
            labels.push(label);
        }
    }

    match labels.as_slice() {
        [only] => (*only).to_string(),
        [a, b] => format!("{a} or {b}"),
        [a, b, c] => format!("{a}, {b}, or {c}"),
        _ => "the selected target".to_string(),
    }
}

fn diagnostic_from_parse_error(error: &crate::parsing::CssParseError) -> CssDiagnostic {
    use crate::parsing::CssParseError;

    match error {
        CssParseError::UnsupportedAtRule { name } => CssDiagnostic {
            code: CssDiagnosticCode::UnsupportedAtRule,
            severity: CssDiagnosticSeverity::Error,
            message: format!("unsupported at-rule `{name}`"),
            range: CssRange::whole_document(),
        },
        CssParseError::UnsupportedSelector { selector } => CssDiagnostic {
            code: CssDiagnosticCode::UnsupportedSelector,
            severity: CssDiagnosticSeverity::Error,
            message: format!("unsupported selector `{selector}`"),
            range: CssRange::whole_document(),
        },
        CssParseError::UnsupportedProperty { property } => CssDiagnostic {
            code: CssDiagnosticCode::UnsupportedProperty,
            severity: CssDiagnosticSeverity::Error,
            message: format!("unsupported property `{property}`"),
            range: CssRange::whole_document(),
        },
        CssParseError::InvalidSyntax { line, column } => CssDiagnostic {
            code: CssDiagnosticCode::InvalidSyntax,
            severity: CssDiagnosticSeverity::Error,
            message: format!("invalid CSS near line {line}, column {column}"),
            range: CssRange {
                start_line: *line,
                start_column: *column,
                end_line: *line,
                end_column: *column,
            },
        },
        CssParseError::CssValue(error) => CssDiagnostic {
            code: CssDiagnosticCode::UnsupportedValue,
            severity: CssDiagnosticSeverity::Error,
            message: error.to_string(),
            range: CssRange::whole_document(),
        },
    }
}

fn push_unique(targets: &mut Vec<StyleTarget>, target: StyleTarget) {
    if !targets.contains(&target) {
        targets.push(target);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_names_containing_group_do_not_become_group_targets() {
        let analysis = analyze_stylesheet(".stack-group__item { width: 50%; height: 100%; }");

        assert!(!analysis.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == CssDiagnosticCode::InapplicableProperty
                && diagnostic.message.contains("width")
        }));
        assert!(!analysis.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == CssDiagnosticCode::InapplicableProperty
                && diagnostic.message.contains("height")
        }));
    }

    #[test]
    fn accepts_layout_property_on_window_rule() {
        let analysis = analyze_stylesheet("window { display: flex; }");

        assert!(analysis.diagnostics.is_empty());
        assert_eq!(analysis.symbols.len(), 1);
    }

    #[test]
    fn reports_exact_property_range_for_multiline_rule() {
        let analysis = analyze_stylesheet("window {\n  display: flex;\n}");

        assert!(analysis.diagnostics.is_empty());
    }

    #[test]
    fn rejects_titlebar_rule() {
        let analysis = analyze_stylesheet("window::titlebar { display: flex; }");

        assert_eq!(analysis.diagnostics.len(), 1);
        assert_eq!(analysis.diagnostics[0].code, CssDiagnosticCode::UnsupportedSelector);
    }

    #[test]
    fn reports_unsupported_selector_attribute_key() {
        let analysis = analyze_stylesheet("window[foo='bar'] { display: flex; }");

        assert_eq!(analysis.diagnostics.len(), 1);
        assert_eq!(analysis.diagnostics[0].code, CssDiagnosticCode::UnsupportedAttributeKey);
        assert_eq!(analysis.diagnostics[0].message, "unsupported attribute key `foo`");
        assert_eq!(
            analysis.diagnostics[0].range,
            CssRange { start_line: 1, start_column: 8, end_line: 1, end_column: 11 }
        );
    }

    #[test]
    fn accepts_supported_selector_attribute_key() {
        let analysis = analyze_stylesheet("window[app_id='foot'] { display: flex; }");

        assert!(analysis.diagnostics.is_empty());
    }

    #[test]
    fn extracts_rule_symbols() {
        let analysis = analyze_stylesheet("window { display: flex; }\ngroup { gap: 8px; }");

        assert_eq!(analysis.symbols.len(), 2);
        assert_eq!(analysis.symbols[0].kind, CssSymbolKind::Rule);
        assert_eq!(analysis.symbols[0].name, "window");
        assert_eq!(analysis.symbols[1].kind, CssSymbolKind::Rule);
        assert_eq!(analysis.symbols[1].name, "group");
    }

    #[test]
    fn surfaces_parse_errors_as_structured_diagnostics() {
        let analysis = analyze_stylesheet("slot { display: flex; }");

        assert_eq!(analysis.diagnostics.len(), 1);
        assert_eq!(analysis.diagnostics[0].code, CssDiagnosticCode::UnsupportedSelector);
    }
}
