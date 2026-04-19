use lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position, Range};
use tilescript_css::language::{
    SelectorTarget, StyleTarget, SupportStatus, attribute_key_spec, property_spec,
    pseudo_class_spec, pseudo_element_spec,
};

use crate::project::{ProjectIndex, ProjectSelectorKind};
use crate::syntax::{
    CursorContext, cursor_context, enclosing_square_bracket_end, enclosing_square_bracket_start,
    has_prefix, identifier_bounds, inside_square_brackets, next_non_whitespace_byte,
    next_non_whitespace_byte_in_range, previous_non_whitespace_byte_in_range, range_for,
    selector_reference_at_offset,
};

pub fn hover_for(
    uri: &lsp_types::Url,
    source: &str,
    position: Position,
    project_index: &ProjectIndex,
) -> Option<Hover> {
    let (context, offset, _) = cursor_context(source, position)?;
    let (start, end) = identifier_bounds(source, offset)?;
    let token = &source[start..end];

    if let Some(spec) = project_selector_hover(uri, source, offset, project_index) {
        return Some(hover(spec, range_for(source, start, end)?));
    }
    if let Some(spec) = property_hover(source, start, end, token, context) {
        return Some(hover(spec, range_for(source, start, end)?));
    }
    if let Some(spec) = pseudo_element_hover(source, start, end, token, context) {
        return Some(hover(spec, range_for(source, start, end)?));
    }
    if let Some(spec) = pseudo_class_hover(source, start, end, token, context) {
        return Some(hover(spec, range_for(source, start, end)?));
    }
    if let Some(spec) = attribute_key_hover(source, start, end, token, context) {
        return Some(hover(spec, range_for(source, start, end)?));
    }

    None
}

fn project_selector_hover(
    uri: &lsp_types::Url,
    source: &str,
    offset: usize,
    project_index: &ProjectIndex,
) -> Option<String> {
    let path = crate::uri::path_from_url(uri)?;
    let reference = selector_reference_at_offset(source, offset)?;

    let kind = match reference.kind {
        crate::syntax::SelectorReferenceKind::Id => ProjectSelectorKind::Id,
        crate::syntax::SelectorReferenceKind::Class => ProjectSelectorKind::Class,
    };
    let definitions = project_index.selector_definitions_for_path(&path, kind, &reference.name);
    if definitions.is_empty() {
        return None;
    }

    let label = match kind {
        ProjectSelectorKind::Id => format!("`#{}`", reference.name),
        ProjectSelectorKind::Class => format!("`.{}`", reference.name),
    };
    let definition_count = definitions.len();
    let source_label = if definition_count == 1 { "definition" } else { "definitions" };

    Some(format!(
        "{}\n\nProject-backed selector from authored TSX in the current app scope.\n\n{} matching {} found.",
        label, definition_count, source_label
    ))
}

fn property_hover(
    source: &str,
    start: usize,
    end: usize,
    token: &str,
    context: CursorContext,
) -> Option<String> {
    let spec = property_spec(token)?;
    if context != CursorContext::PropertyName || !looks_like_property_name(source, start, end) {
        return None;
    }

    Some(format!(
        "`{}`\n\n{}\n\nStatus: {}\n\nApplies to: {}",
        spec.name,
        spec.hover,
        support_status_label(spec.status),
        style_targets_label(spec.applies_to),
    ))
}

fn pseudo_element_hover(
    source: &str,
    start: usize,
    _end: usize,
    token: &str,
    context: CursorContext,
) -> Option<String> {
    let spec = pseudo_element_spec(token)?;
    if context != CursorContext::PseudoElement || !has_prefix(source, start, "::") {
        return None;
    }

    Some(format!(
        "`::{}`\n\n{}\n\nApplies to: {}",
        spec.name,
        spec.hover,
        selector_targets_label(spec.targets),
    ))
}

fn pseudo_class_hover(
    source: &str,
    start: usize,
    _end: usize,
    token: &str,
    context: CursorContext,
) -> Option<String> {
    let spec = pseudo_class_spec(token)?;
    if context != CursorContext::PseudoClass
        || !has_prefix(source, start, ":")
        || has_prefix(source, start, "::")
    {
        return None;
    }

    Some(format!("`:{}`\n\n{}", spec.name, spec.hover))
}

fn attribute_key_hover(
    source: &str,
    start: usize,
    end: usize,
    token: &str,
    context: CursorContext,
) -> Option<String> {
    let spec = attribute_key_spec(token)?;
    if context != CursorContext::AttributeKey || !looks_like_attribute_key(source, start, end) {
        return None;
    }

    Some(format!(
        "`[{}]`\n\n{}\n\nApplies to: {}",
        spec.name,
        spec.hover,
        selector_targets_label(spec.targets),
    ))
}

fn hover(contents: String, range: Range) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: contents,
        }),
        range: Some(range),
    }
}

fn looks_like_property_name(source: &str, start: usize, end: usize) -> bool {
    let Some(next) = next_non_whitespace_byte(source, end) else {
        return false;
    };
    if next != b':' {
        return false;
    }

    !inside_square_brackets(source, start)
}

fn looks_like_attribute_key(source: &str, start: usize, end: usize) -> bool {
    let Some(bracket_start) = enclosing_square_bracket_start(source, start) else {
        return false;
    };
    let Some(bracket_end) = enclosing_square_bracket_end(source, end) else {
        return false;
    };

    let next = next_non_whitespace_byte_in_range(source, end, bracket_end).unwrap_or(b']');
    let prev = previous_non_whitespace_byte_in_range(source, start, bracket_start).unwrap_or(b'[');

    prev == b'[' && matches!(next, b']' | b'=' | b'~' | b'|' | b'^' | b'$' | b'*')
}

fn support_status_label(status: SupportStatus) -> &'static str {
    match status {
        SupportStatus::Full => "full",
        SupportStatus::Partial => "partial",
        SupportStatus::Planned => "planned",
    }
}

fn style_targets_label(targets: &[StyleTarget]) -> String {
    targets
        .iter()
        .map(|target| match target {
            StyleTarget::Workspace => "`workspace`",
            StyleTarget::Group => "`group`",
            StyleTarget::Window => "`window`",
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn selector_targets_label(targets: &[SelectorTarget]) -> String {
    targets
        .iter()
        .map(|target| match target {
            SelectorTarget::Workspace => "`workspace`",
            SelectorTarget::Group => "`group`",
            SelectorTarget::Window => "`window`",
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::Url;

    #[test]
    fn hovers_property_name() {
        let hover = hover_for(
            &Url::parse("file:///test.css").unwrap(),
            "window { display: flex; }",
            Position { line: 0, character: 10 },
            &ProjectIndex::default(),
        )
        .unwrap();

        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup hover");
        };
        assert!(markup.value.contains("`display`"));
        assert!(markup.value.contains("Applies to: `workspace`, `group`, `window`"));
    }

    #[test]
    fn hovers_pseudo_class() {
        let hover = hover_for(
            &Url::parse("file:///test.css").unwrap(),
            "window:focused { display: flex; }",
            Position { line: 0, character: 8 },
            &ProjectIndex::default(),
        )
        .unwrap();

        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup hover");
        };
        assert!(markup.value.contains("`:focused`"));
    }

    #[test]
    fn hovers_attribute_key() {
        let hover = hover_for(
            &Url::parse("file:///test.css").unwrap(),
            "window[app_id='foot'] { display: flex; }",
            Position { line: 0, character: 8 },
            &ProjectIndex::default(),
        )
        .unwrap();

        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup hover");
        };
        assert!(markup.value.contains("`[app_id]`"));
    }

    #[test]
    fn hovers_project_backed_selector_class() {
        let uri = Url::parse("file:///tmp/layouts/example/index.css").unwrap();
        let mut project_index = ProjectIndex::default();
        project_index.index_app_scope(
            std::path::PathBuf::from("/tmp/layouts/example/index.tsx"),
            vec![(
                std::path::PathBuf::from("/tmp/layouts/example/index.tsx"),
                r#"export default function layout() { return <workspace class={joinClasses("shell", growClass(1))} /> }"#
                    .to_string(),
            )],
            vec![(
                std::path::PathBuf::from("/tmp/layouts/example/index.css"),
                "window.shell { display: flex; }".to_string(),
            )],
        );

        let hover = hover_for(
            &uri,
            "window.shell { display: flex; }",
            Position { line: 0, character: 8 },
            &project_index,
        )
        .unwrap();

        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup hover");
        };
        assert!(markup.value.contains("`.shell`"));
        assert!(markup.value.contains("Project-backed selector"));
    }
}
