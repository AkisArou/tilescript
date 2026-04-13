use lsp_types::{Location, Position, Url};
use hypreact_css::analysis::{CssReferenceKind, CssSymbolKind, analyze_stylesheet};

use crate::project::{ProjectIndex, ProjectSelectorKind};
use crate::syntax::{
    keyframes_name_at_offset, position_to_offset, range_for, selector_reference_at_offset,
    selector_references_in_segment, to_lsp_range,
};

pub fn references_for(
    uri: &Url,
    source: &str,
    position: Position,
    include_declaration: bool,
    project_index: &ProjectIndex,
    documents: &[(Url, String)],
) -> Vec<Location> {
    let analysis = analyze_stylesheet(source);
    let offset = match position_to_offset(source, position) {
        Some(offset) => offset,
        None => return Vec::new(),
    };
    let path = crate::uri::path_from_url(uri);

    if let Some(path) = path.as_deref()
        && let Some(selector) = project_index.selector_at(path, offset)
    {
        return selector_reference_locations(
            path,
            selector.kind,
            &selector.name,
            include_declaration,
            project_index,
            documents,
        );
    }

    if let Some(selector) = selector_reference_at_offset(source, offset) {
        let Some(path) = path.as_deref() else {
            return Vec::new();
        };
        return selector_reference_locations(
            path,
            match selector.kind {
                crate::syntax::SelectorReferenceKind::Id => ProjectSelectorKind::Id,
                crate::syntax::SelectorReferenceKind::Class => ProjectSelectorKind::Class,
            },
            &selector.name,
            include_declaration,
            project_index,
            documents,
        );
    }

    let Some(name) = keyframes_name_at_offset(&analysis, source, offset) else {
        return Vec::new();
    };

    let mut locations = Vec::new();

    if include_declaration {
        locations.extend(
            analysis
                .symbols
                .iter()
                .filter(|symbol| symbol.kind == CssSymbolKind::Keyframes && symbol.name == name)
                .map(|symbol| Location {
                    uri: uri.clone(),
                    range: to_lsp_range(symbol.selection_range),
                }),
        );
    }

    locations.extend(
        analysis
            .references
            .iter()
            .filter(|reference| {
                reference.kind == CssReferenceKind::AnimationName && reference.name == name
            })
            .map(|reference| Location { uri: uri.clone(), range: to_lsp_range(reference.range) }),
    );

    locations
}

fn selector_reference_locations(
    path: &std::path::Path,
    kind: ProjectSelectorKind,
    name: &str,
    include_declaration: bool,
    project_index: &ProjectIndex,
    documents: &[(Url, String)],
) -> Vec<Location> {
    let mut locations = Vec::new();
    let scoped_documents = project_index.stylesheet_documents_for_path(path);

    if include_declaration {
        locations.extend(project_index.selector_definitions_for_path(path, kind, name));
    }

    let document_source: Vec<(Url, String)> =
        if scoped_documents.is_empty() { documents.to_vec() } else { scoped_documents };

    for (uri, source) in &document_source {
        if !uri.path().ends_with(".css") {
            continue;
        }
        locations.extend(selector_locations_in_stylesheet(uri, source, kind, name));
    }

    dedupe_locations(locations)
}

fn selector_locations_in_stylesheet(
    uri: &Url,
    source: &str,
    kind: ProjectSelectorKind,
    name: &str,
) -> Vec<Location> {
    let analysis = analyze_stylesheet(source);
    let mut locations = Vec::new();

    for symbol in &analysis.symbols {
        if symbol.kind != CssSymbolKind::Rule {
            continue;
        }

        let start = position_to_offset(
            source,
            Position {
                line: symbol.selection_range.start_line.saturating_sub(1),
                character: symbol.selection_range.start_column.saturating_sub(1),
            },
        );
        let end = position_to_offset(
            source,
            Position {
                line: symbol.selection_range.end_line.saturating_sub(1),
                character: symbol.selection_range.end_column.saturating_sub(1),
            },
        );

        let (Some(start), Some(end)) = (start, end) else {
            continue;
        };

        for selector in selector_references_in_segment(&source[start..end], start) {
            let selector_kind = match selector.kind {
                crate::syntax::SelectorReferenceKind::Id => ProjectSelectorKind::Id,
                crate::syntax::SelectorReferenceKind::Class => ProjectSelectorKind::Class,
            };
            if selector_kind != kind || selector.name != name {
                continue;
            }

            let Some(range) = range_for(source, selector.start, selector.end) else {
                continue;
            };
            locations.push(Location { uri: uri.clone(), range });
        }
    }

    locations
}

fn dedupe_locations(locations: Vec<Location>) -> Vec<Location> {
    let mut seen = std::collections::BTreeSet::new();
    let mut deduped = Vec::new();

    for location in locations {
        let key = (
            location.uri.to_string(),
            location.range.start.line,
            location.range.start.character,
            location.range.end.line,
            location.range.end.character,
        );
        if seen.insert(key) {
            deduped.push(location);
        }
    }

    deduped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_references_from_animation_name_use() {
        let uri = Url::parse("file:///test.css").unwrap();
        let source = "@keyframes fade-in { from { opacity: 0; } to { opacity: 1; } }\nwindow { animation-name: fade-in; }\ngroup { animation-name: fade-in; }";

        let references = references_for(
            &uri,
            source,
            Position { line: 1, character: 26 },
            true,
            &ProjectIndex::default(),
            &[(uri.clone(), source.to_string())],
        );

        assert_eq!(references.len(), 3);
    }

    #[test]
    fn finds_references_from_keyframes_definition() {
        let uri = Url::parse("file:///test.css").unwrap();
        let source = "@keyframes fade-in { from { opacity: 0; } to { opacity: 1; } }\nwindow { animation-name: fade-in; }";

        let references = references_for(
            &uri,
            source,
            Position { line: 0, character: 12 },
            false,
            &ProjectIndex::default(),
            &[(uri.clone(), source.to_string())],
        );

        assert_eq!(references.len(), 1);
        assert_eq!(references[0].range.start.line, 1);
    }

    #[test]
    fn finds_css_and_layout_references_for_selector_id() {
        let css_uri = Url::parse("file:///tmp/layouts/example/index.css").unwrap();
        let other_css_uri = Url::parse("file:///tmp/layouts/example/extra.css").unwrap();
        let css_source = "window#root { color: red; }";
        let other_css_source = "group#root { color: blue; }";
        let tsx_source = r#"export default function layout() { return <workspace id="root" /> }"#;
        let mut project_index = ProjectIndex::default();
        project_index.index_app_scope(
            std::path::PathBuf::from("/tmp/layouts/example/index.tsx"),
            vec![(
                std::path::PathBuf::from("/tmp/layouts/example/index.tsx"),
                tsx_source.to_string(),
            )],
            vec![
                (
                    std::path::PathBuf::from("/tmp/layouts/example/index.css"),
                    css_source.to_string(),
                ),
                (
                    std::path::PathBuf::from("/tmp/layouts/example/extra.css"),
                    other_css_source.to_string(),
                ),
            ],
        );

        let references = references_for(
            &css_uri,
            css_source,
            Position { line: 0, character: 8 },
            true,
            &project_index,
            &[],
        );

        assert_eq!(references.len(), 3);
        assert!(references.iter().any(|location| location.uri.path().ends_with("index.tsx")));
        assert!(references.iter().any(|location| location.uri == css_uri));
        assert!(references.iter().any(|location| location.uri == other_css_uri));
    }

    #[test]
    fn finds_css_references_from_layout_selector_definition() {
        let tsx_uri = Url::parse("file:///tmp/layouts/example/index.tsx").unwrap();
        let css_uri = Url::parse("file:///tmp/layouts/example/index.css").unwrap();
        let tsx_source =
            r#"export default function layout() { return <workspace id="root" class="shell" /> }"#;
        let css_source = "window#root.shell { color: red; }";
        let mut project_index = ProjectIndex::default();
        project_index.index_app_scope(
            std::path::PathBuf::from("/tmp/layouts/example/index.tsx"),
            vec![(
                std::path::PathBuf::from("/tmp/layouts/example/index.tsx"),
                tsx_source.to_string(),
            )],
            vec![(
                std::path::PathBuf::from("/tmp/layouts/example/index.css"),
                css_source.to_string(),
            )],
        );

        let references = references_for(
            &tsx_uri,
            tsx_source,
            Position { line: 0, character: 60 },
            false,
            &project_index,
            &[],
        );

        assert_eq!(references.len(), 1);
        assert_eq!(references[0].uri, css_uri);
    }

    #[test]
    fn excludes_other_layout_scope_css_references() {
        let css_uri = Url::parse("file:///tmp/layouts/example/index.css").unwrap();
        let other_layout_css_uri = Url::parse("file:///tmp/layouts/other/index.css").unwrap();
        let mut project_index = ProjectIndex::default();
        project_index.index_app_scope(
            std::path::PathBuf::from("/tmp/layouts/example/index.tsx"),
            vec![(
                std::path::PathBuf::from("/tmp/layouts/example/index.tsx"),
                r#"export default function layout() { return <workspace id="root" /> }"#
                    .to_string(),
            )],
            vec![(
                std::path::PathBuf::from("/tmp/layouts/example/index.css"),
                "window#root { color: red; }".to_string(),
            )],
        );
        project_index.index_app_scope(
            std::path::PathBuf::from("/tmp/layouts/other/index.tsx"),
            vec![(
                std::path::PathBuf::from("/tmp/layouts/other/index.tsx"),
                r#"export default function layout() { return <workspace id="root" /> }"#
                    .to_string(),
            )],
            vec![(
                std::path::PathBuf::from("/tmp/layouts/other/index.css"),
                "group#root { color: blue; }".to_string(),
            )],
        );

        let references = references_for(
            &css_uri,
            "window#root { color: red; }",
            Position { line: 0, character: 8 },
            true,
            &project_index,
            &[(other_layout_css_uri, "group#root { color: blue; }".to_string())],
        );

        assert_eq!(references.len(), 2);
    }
}
