use cssparser::{Parser, ParserInput, Token};
use lsp_types::{Position, Range};

use hypreact_css::analysis::{CssAnalysis, CssRange, CssReferenceKind, CssSymbolKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorContext {
    PropertyName,
    PropertyValue,
    SelectorId,
    SelectorClass,
    PseudoClass,
    PseudoElement,
    AttributeKey,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectorReferenceKind {
    Id,
    Class,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectorReference {
    pub kind: SelectorReferenceKind,
    pub name: String,
    pub start: usize,
    pub end: usize,
}

pub fn cursor_context(source: &str, position: Position) -> Option<(CursorContext, usize, usize)> {
    let offset = position_to_offset(source, position)?;
    let token_start = token_start(source, offset);

    let context = if is_property_value_context(source, token_start) {
        CursorContext::PropertyValue
    } else if is_property_name_context(source, token_start) {
        CursorContext::PropertyName
    } else if let Some(selector_context) = selector_context(source, offset, token_start) {
        selector_context
    } else {
        CursorContext::None
    };

    Some((context, offset, token_start))
}

pub fn position_to_offset(source: &str, position: Position) -> Option<usize> {
    let mut line = 0u32;
    let mut column = 0u32;

    for (offset, ch) in source.char_indices() {
        if line == position.line && column == position.character {
            return Some(offset);
        }

        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }

    (line == position.line && column == position.character).then_some(source.len())
}

pub fn offset_to_position(source: &str, offset: usize) -> Option<Position> {
    if offset > source.len() {
        return None;
    }

    let mut line = 0u32;
    let mut column = 0u32;
    for (index, ch) in source.char_indices() {
        if index == offset {
            return Some(Position { line, character: column });
        }
        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }

    Some(Position { line, character: column })
}

pub fn range_for(source: &str, start: usize, end: usize) -> Option<Range> {
    Some(Range { start: offset_to_position(source, start)?, end: offset_to_position(source, end)? })
}

pub fn selector_reference_at_offset(source: &str, offset: usize) -> Option<SelectorReference> {
    let selector = selector_segment_at_offset(source, offset)?;

    selector_references_in_segment(selector.text, selector.start)
        .into_iter()
        .find(|reference| reference.start <= offset && offset <= reference.end)
}

pub fn to_lsp_range(range: CssRange) -> Range {
    Range {
        start: Position {
            line: range.start_line.saturating_sub(1),
            character: range.start_column.saturating_sub(1),
        },
        end: Position {
            line: range.end_line.saturating_sub(1),
            character: range.end_column.saturating_sub(1),
        },
    }
}

pub fn range_contains(range: CssRange, offset: usize, source: &str) -> bool {
    let start = position_to_offset(
        source,
        Position {
            line: range.start_line.saturating_sub(1),
            character: range.start_column.saturating_sub(1),
        },
    );
    let end = position_to_offset(
        source,
        Position {
            line: range.end_line.saturating_sub(1),
            character: range.end_column.saturating_sub(1),
        },
    );

    matches!((start, end), (Some(start), Some(end)) if start <= offset && offset <= end)
}

pub fn keyframes_name_at_offset(
    analysis: &CssAnalysis,
    source: &str,
    offset: usize,
) -> Option<String> {
    if let Some(symbol) = analysis.symbols.iter().find(|symbol| {
        symbol.kind == CssSymbolKind::Keyframes
            && range_contains(symbol.selection_range, offset, source)
    }) {
        return Some(symbol.name.clone());
    }

    analysis
        .references
        .iter()
        .find(|reference| {
            reference.kind == CssReferenceKind::AnimationName
                && range_contains(reference.range, offset, source)
        })
        .map(|reference| reference.name.clone())
}

pub fn identifier_bounds(source: &str, offset: usize) -> Option<(usize, usize)> {
    if source.is_empty() {
        return None;
    }

    let mut probe = offset.min(source.len().saturating_sub(1));
    if !is_identifier_byte(*source.as_bytes().get(probe)?) {
        if probe == 0 || !is_identifier_byte(*source.as_bytes().get(probe.saturating_sub(1))?) {
            return None;
        }
        probe = probe.saturating_sub(1);
    }

    let bytes = source.as_bytes();
    let mut start = probe;
    while start > 0 && is_identifier_byte(bytes[start - 1]) {
        start -= 1;
    }

    let mut end = probe + 1;
    while end < source.len() && is_identifier_byte(bytes[end]) {
        end += 1;
    }

    Some((start, end))
}

pub fn enclosing_property_name(source: &str, offset: usize) -> Option<String> {
    let DeclarationContext::Value { property_name, .. } = declaration_context(source, offset)?
    else {
        return None;
    };
    Some(property_name)
}

pub fn has_prefix(source: &str, start: usize, prefix: &str) -> bool {
    start >= prefix.len() && &source[start - prefix.len()..start] == prefix
}

pub fn inside_square_brackets(source: &str, offset: usize) -> bool {
    enclosing_square_bracket_start(source, offset)
        .zip(enclosing_square_bracket_end(source, offset))
        .is_some_and(|(start, end)| start < offset && offset <= end)
}

pub fn enclosing_square_bracket_start(source: &str, offset: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut index = offset;
    while index > 0 {
        index -= 1;
        match bytes[index] {
            b'[' => return Some(index),
            b']' | b'{' | b';' => return None,
            _ => {}
        }
    }
    None
}

pub fn enclosing_square_bracket_end(source: &str, offset: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut index = offset;
    while index < bytes.len() {
        match bytes[index] {
            b']' => return Some(index),
            b'[' | b'{' | b';' => return None,
            _ => index += 1,
        }
    }
    None
}

pub fn token_start(source: &str, offset: usize) -> usize {
    let bytes = source.as_bytes();
    let mut start = offset.min(source.len());

    while start > 0 {
        let byte = bytes[start - 1];
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_') {
            start -= 1;
            continue;
        }
        break;
    }

    start
}

pub fn next_non_whitespace_byte(source: &str, mut offset: usize) -> Option<u8> {
    let bytes = source.as_bytes();
    while offset < bytes.len() {
        if !bytes[offset].is_ascii_whitespace() {
            return Some(bytes[offset]);
        }
        offset += 1;
    }
    None
}

pub fn next_non_whitespace_byte_in_range(
    source: &str,
    mut offset: usize,
    end: usize,
) -> Option<u8> {
    let bytes = source.as_bytes();
    while offset < end {
        if !bytes[offset].is_ascii_whitespace() {
            return Some(bytes[offset]);
        }
        offset += 1;
    }
    None
}

pub fn previous_non_whitespace_byte_in_range(
    source: &str,
    offset: usize,
    lower_bound: usize,
) -> Option<u8> {
    let bytes = source.as_bytes();
    let mut index = offset;
    while index > lower_bound {
        index -= 1;
        if !bytes[index].is_ascii_whitespace() {
            return Some(bytes[index]);
        }
    }
    None
}

fn is_property_name_context(source: &str, token_start: usize) -> bool {
    if inside_square_brackets(source, token_start) {
        return false;
    }

    matches!(declaration_context(source, token_start), Some(DeclarationContext::Name))
}

fn is_property_value_context(source: &str, token_start: usize) -> bool {
    matches!(declaration_context(source, token_start), Some(DeclarationContext::Value { .. }))
}

fn is_attribute_key_context(source: &str, offset: usize, token_start: usize) -> bool {
    let Some(bracket_start) = enclosing_square_bracket_start(source, token_start) else {
        return false;
    };

    let content = &source[bracket_start + 1..offset];
    !content.contains('=')
        && !content.contains('~')
        && !content.contains('|')
        && !content.contains('^')
        && !content.contains('$')
        && !content.contains('*')
}

fn selector_context(source: &str, offset: usize, token_start: usize) -> Option<CursorContext> {
    let context = selector_context_from_parser(source, offset, token_start)
        .or_else(|| selector_context_fallback(source, offset, token_start));

    match context {
        Some(
            CursorContext::AttributeKey
            | CursorContext::SelectorId
            | CursorContext::SelectorClass
            | CursorContext::PseudoClass
            | CursorContext::PseudoElement,
        ) => context,
        _ => None,
    }
}

fn selector_context_from_parser(
    source: &str,
    offset: usize,
    token_start: usize,
) -> Option<CursorContext> {
    let selector = current_selector_segment(source, offset)?;
    let relative_token_start = token_start.checked_sub(selector.start)?;
    let mut input = ParserInput::new(selector.text);
    let mut parser = Parser::new(&mut input);
    let mut last_colon_offset = None;
    let mut last_bracket_start = None;

    while parser.position().byte_index() < relative_token_start {
        let token = parser.next_including_whitespace_and_comments().ok()?;
        match token {
            Token::Colon => last_colon_offset = Some(parser.position().byte_index()),
            Token::SquareBracketBlock => {
                let end = parser.position().byte_index();
                let start = end.saturating_sub(1);
                if start <= relative_token_start && relative_token_start <= end {
                    return Some(CursorContext::AttributeKey);
                }
                last_bracket_start = Some(start);
            }
            _ => {}
        }
    }

    if let Some(bracket_start) = last_bracket_start {
        if bracket_start < relative_token_start {
            return Some(CursorContext::AttributeKey);
        }
    }

    if has_prefix(selector.text, relative_token_start, "#") {
        return Some(CursorContext::SelectorId);
    }

    if has_prefix(selector.text, relative_token_start, ".") {
        return Some(CursorContext::SelectorClass);
    }

    if let Some(colon_offset) = last_colon_offset {
        let colon_count = selector
            .text
            .as_bytes()
            .get(colon_offset.saturating_sub(2)..colon_offset)
            .is_some_and(|bytes| bytes == b"::");
        return Some(if colon_count {
            CursorContext::PseudoElement
        } else {
            CursorContext::PseudoClass
        });
    }

    None
}

fn selector_context_fallback(
    source: &str,
    offset: usize,
    token_start: usize,
) -> Option<CursorContext> {
    if is_attribute_key_context(source, offset, token_start) {
        return Some(CursorContext::AttributeKey);
    }
    if has_prefix(source, token_start, "::") {
        return Some(CursorContext::PseudoElement);
    }
    if has_prefix(source, token_start, "#") {
        return Some(CursorContext::SelectorId);
    }
    if has_prefix(source, token_start, ".") {
        return Some(CursorContext::SelectorClass);
    }
    if has_prefix(source, token_start, ":") {
        return Some(CursorContext::PseudoClass);
    }
    None
}

pub fn selector_references_in_segment(
    selector: &str,
    selector_offset: usize,
) -> Vec<SelectorReference> {
    let mut references = Vec::new();
    let bytes = selector.as_bytes();
    let mut offset = 0;
    let mut bracket_depth = 0u32;

    while offset < selector.len() {
        if let Some(comment_end) = starts_comment(bytes, offset) {
            offset = comment_end;
            continue;
        }
        if let Some(string_end) = starts_string(selector, offset) {
            offset = string_end;
            continue;
        }

        match bytes[offset] {
            b'[' => {
                bracket_depth += 1;
                offset += 1;
            }
            b']' => {
                bracket_depth = bracket_depth.saturating_sub(1);
                offset += 1;
            }
            b'#' if bracket_depth == 0 => {
                let name_start = offset + 1;
                let name_end = identifier_end(selector, name_start);
                if name_start < name_end {
                    references.push(SelectorReference {
                        kind: SelectorReferenceKind::Id,
                        name: selector[name_start..name_end].to_string(),
                        start: selector_offset + offset,
                        end: selector_offset + name_end,
                    });
                    offset = name_end;
                } else {
                    offset += 1;
                }
            }
            b'.' if bracket_depth == 0 => {
                let name_start = offset + 1;
                let name_end = identifier_end(selector, name_start);
                if name_start < name_end {
                    references.push(SelectorReference {
                        kind: SelectorReferenceKind::Class,
                        name: selector[name_start..name_end].to_string(),
                        start: selector_offset + offset,
                        end: selector_offset + name_end,
                    });
                    offset = name_end;
                } else {
                    offset += 1;
                }
            }
            _ => offset += 1,
        }
    }

    references
}

struct SelectorSegment<'a> {
    start: usize,
    text: &'a str,
}

fn selector_segment_at_offset(source: &str, offset: usize) -> Option<SelectorSegment<'_>> {
    let rule_start = source[..offset].rfind(['{', '}']).map(|index| index + 1).unwrap_or(0);
    let selector_start =
        source[..offset].rfind([',', '{', '}']).map(|index| index + 1).unwrap_or(0);
    let selector_end =
        source[offset..].find([',', '{', '}']).map(|index| offset + index).unwrap_or(source.len());

    if source[rule_start..offset].contains(':') {
        return None;
    }

    let selector = &source[selector_start..selector_end];
    (!selector.trim().is_empty())
        .then_some(SelectorSegment { start: selector_start, text: selector })
}

fn current_selector_segment(source: &str, offset: usize) -> Option<SelectorSegment<'_>> {
    let rule_start = source[..offset].rfind(['{', '}']).map(|index| index + 1).unwrap_or(0);
    if source[rule_start..offset].contains(':') {
        return None;
    }

    let selector_start =
        source[..offset].rfind([',', '{', '}']).map(|index| index + 1).unwrap_or(0);
    let selector = &source[selector_start..offset];
    (!selector.trim().is_empty())
        .then_some(SelectorSegment { start: selector_start, text: selector })
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')
}

fn identifier_end(selector: &str, mut offset: usize) -> usize {
    let bytes = selector.as_bytes();
    while offset < bytes.len() && is_identifier_byte(bytes[offset]) {
        offset += 1;
    }
    offset
}

fn starts_comment(bytes: &[u8], offset: usize) -> Option<usize> {
    if bytes.get(offset) != Some(&b'/') || bytes.get(offset + 1) != Some(&b'*') {
        return None;
    }

    let mut index = offset + 2;
    while index + 1 < bytes.len() {
        if bytes[index] == b'*' && bytes[index + 1] == b'/' {
            return Some(index + 2);
        }
        index += 1;
    }

    Some(bytes.len())
}

fn starts_string(source: &str, offset: usize) -> Option<usize> {
    let quote = *source.as_bytes().get(offset)?;
    if !matches!(quote, b'\'' | b'"') {
        return None;
    }

    let bytes = source.as_bytes();
    let mut index = offset + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            current if current == quote => return Some(index + 1),
            _ => index += 1,
        }
    }

    Some(bytes.len())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DeclarationContext {
    Name,
    Value { property_name: String },
}

fn declaration_context(source: &str, offset: usize) -> Option<DeclarationContext> {
    let last_open = source[..offset].rfind('{');
    let last_close = source[..offset].rfind('}');
    if last_open.is_none() || last_close.is_some_and(|close| close > last_open.unwrap()) {
        return None;
    }

    let segment_start = source[..offset].rfind(['{', ';', '}']).map(|index| index + 1).unwrap_or(0);
    let segment = &source[segment_start..offset];
    if segment.trim().is_empty() {
        return Some(DeclarationContext::Name);
    }
    if inside_square_brackets(source, offset) {
        return None;
    }

    declaration_context_from_parser(segment).or_else(|| declaration_context_fallback(segment))
}

fn declaration_context_from_parser(segment: &str) -> Option<DeclarationContext> {
    let mut input = ParserInput::new(segment);
    let mut parser = Parser::new(&mut input);
    let mut property_name = String::new();
    let mut saw_colon = false;

    while let Ok(token) = parser.next_including_whitespace_and_comments() {
        match token {
            Token::Ident(value) if !saw_colon && property_name.is_empty() => {
                property_name = value.to_string();
            }
            Token::Colon if !property_name.is_empty() => {
                saw_colon = true;
            }
            Token::WhiteSpace(_) | Token::Comment(_) => {}
            _ if !saw_colon && property_name.is_empty() => return None,
            _ => {}
        }
    }

    if property_name.is_empty() {
        return Some(DeclarationContext::Name);
    }
    if saw_colon {
        Some(DeclarationContext::Value { property_name })
    } else {
        Some(DeclarationContext::Name)
    }
}

fn declaration_context_fallback(segment: &str) -> Option<DeclarationContext> {
    let Some(colon) = segment.find(':') else {
        return Some(DeclarationContext::Name);
    };

    let property_name = segment[..colon].trim();
    if property_name.is_empty() {
        return None;
    }

    Some(DeclarationContext::Value { property_name: property_name.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_pseudo_class_context() {
        let (context, _, _) =
            cursor_context("window:fo", Position { line: 0, character: 8 }).unwrap();
        assert_eq!(context, CursorContext::PseudoClass);
    }

    #[test]
    fn detects_selector_id_context() {
        let (context, _, _) =
            cursor_context("window#ro", Position { line: 0, character: 8 }).unwrap();
        assert_eq!(context, CursorContext::SelectorId);
    }

    #[test]
    fn detects_selector_class_context() {
        let (context, _, _) =
            cursor_context("window.sh", Position { line: 0, character: 8 }).unwrap();
        assert_eq!(context, CursorContext::SelectorClass);
    }

    #[test]
    fn detects_property_value_context() {
        let (context, _, _) =
            cursor_context("window { text-align: ce }", Position { line: 0, character: 22 })
                .unwrap();
        assert_eq!(context, CursorContext::PropertyValue);
    }

    #[test]
    fn finds_selector_reference_at_offset() {
        let reference =
            selector_reference_at_offset("window#root.stack[title=\".ignored\"]", 9).unwrap();
        assert_eq!(reference.kind, SelectorReferenceKind::Id);
        assert_eq!(reference.name, "root");
    }
}
