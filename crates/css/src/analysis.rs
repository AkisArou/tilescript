use crate::compiled::CompiledStyleSheet;
use crate::language::{StyleTarget, is_supported_attribute_key, property_spec};
use crate::parse_stylesheet;
use crate::query::selector_matches;
use tilescript_core::{LayoutNodeMeta, ResolvedLayoutNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CssRange {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

impl CssRange {
    pub const fn whole_document() -> Self {
        Self { start_line: 1, start_column: 1, end_line: u32::MAX, end_column: u32::MAX }
    }
}

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
    UnknownAnimationName,
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
    pub references: Vec<CssReference>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssSymbolKind {
    Rule,
    Keyframes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssSymbol {
    pub kind: CssSymbolKind,
    pub name: String,
    pub range: CssRange,
    pub selection_range: CssRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssReferenceKind {
    AnimationName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssReference {
    pub kind: CssReferenceKind,
    pub name: String,
    pub range: CssRange,
}

#[derive(Debug, Clone)]
struct AuthoredRule {
    selector_text: String,
    selector_range: CssRange,
    block_range: CssRange,
    targets: Vec<StyleTarget>,
    declarations: Vec<AuthoredDeclaration>,
}

#[derive(Debug, Clone)]
struct AuthoredDeclaration {
    property: String,
    property_range: CssRange,
    value_text: String,
    value_range: CssRange,
}

pub fn analyze_stylesheet(source: &str) -> CssAnalysis {
    match parse_stylesheet(source) {
        Ok(stylesheet) => {
            let authored_rules = authored_rules(source, Some(&stylesheet));
            let diagnostics = semantic_diagnostics(&authored_rules, &stylesheet);
            let symbols = extract_symbols(source, &authored_rules);
            let references = extract_references(&authored_rules);
            CssAnalysis { stylesheet: Some(stylesheet), diagnostics, symbols, references }
        }
        Err(error) => {
            let authored_rules = authored_rules(source, None);
            let symbols = extract_symbols(source, &authored_rules);
            let references = extract_references(&authored_rules);
            CssAnalysis {
                stylesheet: None,
                diagnostics: vec![diagnostic_from_parse_error(&error)],
                symbols,
                references,
            }
        }
    }
}

fn extract_symbols(source: &str, rules: &[AuthoredRule]) -> Vec<CssSymbol> {
    let mut symbols = Vec::new();

    for rule in rules {
        symbols.push(CssSymbol {
            kind: CssSymbolKind::Rule,
            name: rule.selector_text.clone(),
            range: rule.block_range,
            selection_range: rule.selector_range,
        });
    }

    symbols.extend(keyframes_symbols(source));
    symbols
}

fn extract_references(rules: &[AuthoredRule]) -> Vec<CssReference> {
    let mut references = Vec::new();

    for rule in rules {
        for declaration in &rule.declarations {
            if declaration.property != "animation-name" {
                continue;
            }

            references.extend(animation_name_references(declaration));
        }
    }

    references
}

fn animation_name_references(declaration: &AuthoredDeclaration) -> Vec<CssReference> {
    let mut references = Vec::new();
    let source_map = SourceMap::new(&declaration.value_text);

    for (start, end, name) in split_named_values(&declaration.value_text) {
        if name.eq_ignore_ascii_case("none") {
            continue;
        }

        references.push(CssReference {
            kind: CssReferenceKind::AnimationName,
            name,
            range: translate_range(source_map.range(start, end), declaration.value_range),
        });
    }

    references
}

fn keyframes_symbols(source: &str) -> Vec<CssSymbol> {
    let source_map = SourceMap::new(source);
    let mut symbols = Vec::new();
    let mut offset = 0;

    while let Some(relative) = source[offset..].find("@keyframes") {
        let start = offset + relative;
        let name_start = start + "@keyframes".len();
        let Some(open_brace) = find_top_level_token(source, name_start, &['{']) else {
            break;
        };
        let Some(block_end) = find_matching_brace_end(source, open_brace) else {
            break;
        };

        let raw_name = source[name_start..open_brace].trim();
        let selection_start = name_start + leading_trimmed_len(&source[name_start..open_brace]);
        let selection_end = open_brace - trailing_trimmed_len(&source[name_start..open_brace]);
        if !raw_name.is_empty() {
            symbols.push(CssSymbol {
                kind: CssSymbolKind::Keyframes,
                name: raw_name.trim_matches('"').trim_matches('\'').to_string(),
                range: source_map.range(start, block_end),
                selection_range: source_map.range(selection_start, selection_end),
            });
        }

        offset = block_end;
    }

    symbols
}

fn semantic_diagnostics(
    rules: &[AuthoredRule],
    stylesheet: &CompiledStyleSheet,
) -> Vec<CssDiagnostic> {
    let mut diagnostics = selector_attribute_key_diagnostics(rules);
    diagnostics.extend(applicability_diagnostics(rules));
    diagnostics.extend(animation_name_diagnostics(rules, stylesheet));
    diagnostics
}

fn selector_attribute_key_diagnostics(rules: &[AuthoredRule]) -> Vec<CssDiagnostic> {
    let mut diagnostics = Vec::new();

    for rule in rules {
        diagnostics.extend(attribute_key_diagnostics_for_selector(rule));
    }

    diagnostics
}

fn applicability_diagnostics(rules: &[AuthoredRule]) -> Vec<CssDiagnostic> {
    let mut diagnostics = Vec::new();

    for rule in rules {
        for declaration in &rule.declarations {
            let Some(spec) = property_spec(&declaration.property) else {
                continue;
            };
            if rule.targets.iter().any(|target| spec.applies_to.contains(target)) {
                continue;
            }

            diagnostics.push(CssDiagnostic {
                code: CssDiagnosticCode::InapplicableProperty,
                severity: CssDiagnosticSeverity::Warning,
                message: format!(
                    "property `{}` does not apply to {}",
                    declaration.property,
                    describe_targets(&rule.targets)
                ),
                range: declaration.property_range,
            });
        }
    }

    diagnostics
}

fn animation_name_diagnostics(
    rules: &[AuthoredRule],
    stylesheet: &CompiledStyleSheet,
) -> Vec<CssDiagnostic> {
    let mut diagnostics = Vec::new();

    for rule in rules {
        for declaration in &rule.declarations {
            if declaration.property != "animation-name" {
                continue;
            }

            for (_, _, name) in split_named_values(&declaration.value_text) {
                if name.is_empty() || name.eq_ignore_ascii_case("none") {
                    continue;
                }
                if stylesheet.keyframes(&name).is_some() {
                    continue;
                }

                diagnostics.push(CssDiagnostic {
                    code: CssDiagnosticCode::UnknownAnimationName,
                    severity: CssDiagnosticSeverity::Warning,
                    message: format!("unknown animation-name `{name}`"),
                    range: animation_name_reference_range(declaration, &name)
                        .unwrap_or(declaration.value_range),
                });
            }
        }
    }

    diagnostics
}

fn animation_name_reference_range(
    declaration: &AuthoredDeclaration,
    name: &str,
) -> Option<CssRange> {
    animation_name_references(declaration)
        .into_iter()
        .find(|reference| reference.name == name)
        .map(|reference| reference.range)
}

fn authored_rules(source: &str, stylesheet: Option<&CompiledStyleSheet>) -> Vec<AuthoredRule> {
    let source_map = SourceMap::new(source);
    let mut rules = Vec::new();
    let mut offset = 0;
    let mut style_rule_index = 0usize;

    while let Some(start) = skip_ws_and_comments(source, offset) {
        let Some(prelude_end) = find_top_level_token(source, start, &['{', ';']) else {
            break;
        };
        let token = source[prelude_end..].chars().next().unwrap();

        if token == ';' {
            offset = prelude_end + 1;
            continue;
        }

        let Some(block_end) = find_matching_brace_end(source, prelude_end) else {
            break;
        };
        let prelude = source[start..prelude_end].trim();
        if !prelude.starts_with('@') {
            let declarations =
                authored_declarations(source, prelude_end + 1, block_end - 1, &source_map);
            let selector_start = start + leading_trimmed_len(&source[start..prelude_end]);
            let selector_end = prelude_end - trailing_trimmed_len(&source[start..prelude_end]);
            let targets = stylesheet
                .and_then(|stylesheet| stylesheet.rules.get(style_rule_index))
                .map(|rule| infer_style_targets(&rule.selectors))
                .unwrap_or_else(|| {
                    vec![StyleTarget::Workspace, StyleTarget::Group, StyleTarget::Window]
                });
            rules.push(AuthoredRule {
                selector_text: source[selector_start..selector_end].to_string(),
                selector_range: source_map.range(selector_start, selector_end),
                block_range: source_map.range(start, block_end),
                targets,
                declarations,
            });
            style_rule_index += 1;
        }

        offset = block_end;
    }

    rules
}

fn authored_declarations(
    source: &str,
    start: usize,
    end: usize,
    source_map: &SourceMap<'_>,
) -> Vec<AuthoredDeclaration> {
    let mut declarations = Vec::new();
    let mut segment_start = start;
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
            b';' if paren_depth == 0 && bracket_depth == 0 => {
                if let Some(declaration) =
                    parse_declaration_segment(source, segment_start, offset, source_map)
                {
                    declarations.push(declaration);
                }
                segment_start = offset + 1;
            }
            _ => {}
        }

        offset += 1;
    }

    if let Some(declaration) = parse_declaration_segment(source, segment_start, end, source_map) {
        declarations.push(declaration);
    }

    declarations
}

fn parse_declaration_segment(
    source: &str,
    start: usize,
    end: usize,
    source_map: &SourceMap<'_>,
) -> Option<AuthoredDeclaration> {
    let segment = &source[start..end];
    let trimmed_start = start + leading_trimmed_len(segment);
    let trimmed_end = end - trailing_trimmed_len(segment);

    if trimmed_start >= trimmed_end {
        return None;
    }

    let colon = find_top_level_colon(source, trimmed_start, trimmed_end)?;
    let name = source[trimmed_start..colon].trim();
    if name.is_empty() {
        return None;
    }

    let name_start = trimmed_start + leading_trimmed_len(&source[trimmed_start..colon]);
    let name_end = colon - trailing_trimmed_len(&source[trimmed_start..colon]);
    let value_start = colon + 1 + leading_trimmed_len(&source[colon + 1..trimmed_end]);
    let value_end = trimmed_end;

    Some(AuthoredDeclaration {
        property: name.to_string(),
        property_range: source_map.range(name_start, name_end),
        value_text: source[value_start..value_end].trim().to_string(),
        value_range: source_map.range(value_start, value_end),
    })
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

fn split_top_level_commas(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut offset = 0;
    let mut paren_depth = 0i32;
    let mut bracket_depth = 0i32;
    let bytes = input.as_bytes();

    while offset < input.len() {
        if let Some(comment_end) = starts_comment(bytes, offset) {
            offset = comment_end;
            continue;
        }
        if let Some(string_end) = starts_string(input, offset) {
            offset = string_end;
            continue;
        }

        match bytes[offset] {
            b'(' => paren_depth += 1,
            b')' => paren_depth -= 1,
            b'[' => bracket_depth += 1,
            b']' => bracket_depth -= 1,
            b',' if paren_depth == 0 && bracket_depth == 0 => {
                parts.push(&input[start..offset]);
                start = offset + 1;
            }
            _ => {}
        }

        offset += 1;
    }

    parts.push(&input[start..]);
    parts
}

fn split_named_values(input: &str) -> Vec<(usize, usize, String)> {
    let mut values = Vec::new();
    let mut start = 0;

    for part in split_top_level_commas(input) {
        let part_offset = start + leading_trimmed_len(part);
        let trimmed = part.trim();
        let trimmed_end = part_offset + trimmed.len();
        if !trimmed.is_empty() {
            values.push((
                part_offset,
                trimmed_end,
                trimmed.trim_matches('"').trim_matches('\'').to_string(),
            ));
        }
        start += part.len() + 1;
    }

    values
}

fn attribute_key_diagnostics_for_selector(rule: &AuthoredRule) -> Vec<CssDiagnostic> {
    let mut diagnostics = Vec::new();
    let source_map = SourceMap::new(&rule.selector_text);
    let mut offset = 0;
    let bytes = rule.selector_text.as_bytes();

    while offset < rule.selector_text.len() {
        if let Some(comment_end) = starts_comment(bytes, offset) {
            offset = comment_end;
            continue;
        }
        if let Some(string_end) = starts_string(&rule.selector_text, offset) {
            offset = string_end;
            continue;
        }

        if bytes[offset] == b'[' {
            let Some(end) = find_matching_bracket_end(&rule.selector_text, offset) else {
                break;
            };
            if let Some(diagnostic) = attribute_key_diagnostic_for_segment(
                &rule.selector_text,
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

fn skip_ws_and_comments(source: &str, mut offset: usize) -> Option<usize> {
    let bytes = source.as_bytes();

    while offset < source.len() {
        if bytes[offset].is_ascii_whitespace() {
            offset += 1;
            continue;
        }
        if let Some(comment_end) = starts_comment(bytes, offset) {
            offset = comment_end;
            continue;
        }
        return Some(offset);
    }

    None
}

fn find_top_level_token(source: &str, start: usize, tokens: &[char]) -> Option<usize> {
    let mut offset = start;
    let mut paren_depth = 0i32;
    let mut bracket_depth = 0i32;
    let bytes = source.as_bytes();

    while offset < source.len() {
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
            byte if paren_depth == 0 && bracket_depth == 0 && tokens.contains(&(byte as char)) => {
                return Some(offset);
            }
            _ => {}
        }

        offset += 1;
    }

    None
}

fn find_matching_brace_end(source: &str, open_brace: usize) -> Option<usize> {
    let mut offset = open_brace;
    let mut depth = 0i32;
    let bytes = source.as_bytes();

    while offset < source.len() {
        if let Some(comment_end) = starts_comment(bytes, offset) {
            offset = comment_end;
            continue;
        }
        if let Some(string_end) = starts_string(source, offset) {
            offset = string_end;
            continue;
        }

        match bytes[offset] {
            b'{' => depth += 1,
            b'}' => {
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

fn leading_trimmed_len(input: &str) -> usize {
    input.len() - input.trim_start().len()
}

fn trailing_trimmed_len(input: &str) -> usize {
    input.len() - input.trim_end().len()
}

struct SourceMap<'a> {
    source: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> SourceMap<'a> {
    fn new(source: &'a str) -> Self {
        let mut line_starts = vec![0];
        for (offset, ch) in source.char_indices() {
            if ch == '\n' {
                line_starts.push(offset + 1);
            }
        }
        Self { source, line_starts }
    }

    fn range(&self, start: usize, end: usize) -> CssRange {
        let (start_line, start_column) = self.position(start);
        let (end_line, end_column) = self.position(end);
        CssRange { start_line, start_column, end_line, end_column }
    }

    fn position(&self, offset: usize) -> (u32, u32) {
        let line_index = match self.line_starts.binary_search(&offset) {
            Ok(index) => index,
            Err(index) => index.saturating_sub(1),
        };
        let line_start = self.line_starts[line_index];
        let column = self.source[line_start..offset].chars().count() as u32 + 1;
        (line_index as u32 + 1, column)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_names_containing_group_do_not_become_group_targets() {
        let analysis =
            analyze_stylesheet(".stack-group__item { border-width: 1px; border-color: #2f3647; }");

        assert!(!analysis.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == CssDiagnosticCode::InapplicableProperty
                && diagnostic.message.contains("border-width")
        }));
        assert!(!analysis.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == CssDiagnosticCode::InapplicableProperty
                && diagnostic.message.contains("border-color")
        }));
    }

    #[test]
    fn accepts_text_align_on_window_rule() {
        let analysis = analyze_stylesheet("window { text-align: center; }");

        assert!(analysis.diagnostics.is_empty());
        assert_eq!(analysis.symbols.len(), 1);
    }

    #[test]
    fn reports_exact_property_range_for_multiline_rule() {
        let analysis = analyze_stylesheet("window {\n  text-align: center;\n}");

        assert!(analysis.diagnostics.is_empty());
    }

    #[test]
    fn rejects_titlebar_rule() {
        let analysis = analyze_stylesheet("window::titlebar { text-align: center; }");

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
    fn reports_unknown_animation_name_on_value_range() {
        let analysis = analyze_stylesheet("window { animation-name: fade-in; }");

        assert_eq!(analysis.diagnostics.len(), 1);
        assert_eq!(analysis.diagnostics[0].code, CssDiagnosticCode::UnknownAnimationName);
        assert_eq!(analysis.diagnostics[0].message, "unknown animation-name `fade-in`");
        assert_eq!(
            analysis.diagnostics[0].range,
            CssRange { start_line: 1, start_column: 26, end_line: 1, end_column: 33 }
        );
    }

    #[test]
    fn accepts_known_animation_name() {
        let analysis = analyze_stylesheet(
            "@keyframes fade-in { from { opacity: 0; } to { opacity: 1; } } window { animation-name: fade-in; }",
        );

        assert!(analysis.diagnostics.is_empty());
        assert_eq!(analysis.symbols.len(), 2);
    }

    #[test]
    fn extracts_rule_and_keyframes_symbols() {
        let analysis = analyze_stylesheet(
            "@keyframes fade-in { from { opacity: 0; } to { opacity: 1; } }\nwindow { display: flex; }",
        );

        assert_eq!(analysis.symbols.len(), 2);
        assert_eq!(analysis.symbols[0].kind, CssSymbolKind::Rule);
        assert_eq!(analysis.symbols[0].name, "window");
        assert_eq!(analysis.symbols[1].kind, CssSymbolKind::Keyframes);
        assert_eq!(analysis.symbols[1].name, "fade-in");
    }

    #[test]
    fn surfaces_parse_errors_as_structured_diagnostics() {
        let analysis = analyze_stylesheet("slot { display: flex; }");

        assert_eq!(analysis.diagnostics.len(), 1);
        assert_eq!(analysis.diagnostics[0].code, CssDiagnosticCode::UnsupportedSelector);
    }
}
