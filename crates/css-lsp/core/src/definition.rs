use hypreact_css::analysis::{CssReferenceKind, CssSymbolKind, analyze_stylesheet};
use lsp_types::{GotoDefinitionResponse, Location, Position, Url};

use crate::project::{ProjectIndex, ProjectSelectorKind};
use crate::syntax::{
    position_to_offset, range_contains, selector_reference_at_offset, to_lsp_range,
};

pub fn definition_for(
    uri: &Url,
    source: &str,
    position: Position,
    project_index: &ProjectIndex,
) -> Option<GotoDefinitionResponse> {
    let analysis = analyze_stylesheet(source);
    let offset = position_to_offset(source, position)?;

    if let Some(reference) = selector_reference_at_offset(source, offset) {
        let path = crate::uri::path_from_url(uri)?;
        let locations = project_index.selector_definitions(
            // fallback kept for non-scoped callers if needed later
            match reference.kind {
                crate::syntax::SelectorReferenceKind::Id => ProjectSelectorKind::Id,
                crate::syntax::SelectorReferenceKind::Class => ProjectSelectorKind::Class,
            },
            &reference.name,
        );
        let locations = if locations.is_empty() {
            project_index.selector_definitions_for_path(
                &path,
                match reference.kind {
                    crate::syntax::SelectorReferenceKind::Id => ProjectSelectorKind::Id,
                    crate::syntax::SelectorReferenceKind::Class => ProjectSelectorKind::Class,
                },
                &reference.name,
            )
        } else {
            project_index.selector_definitions_for_path(
                &path,
                match reference.kind {
                    crate::syntax::SelectorReferenceKind::Id => ProjectSelectorKind::Id,
                    crate::syntax::SelectorReferenceKind::Class => ProjectSelectorKind::Class,
                },
                &reference.name,
            )
        };

        return match locations.as_slice() {
            [] => None,
            [location] => Some(GotoDefinitionResponse::Scalar(location.clone())),
            _ => Some(GotoDefinitionResponse::Array(locations)),
        };
    }

    let reference = analysis.references.iter().find(|reference| {
        reference.kind == CssReferenceKind::AnimationName
            && range_contains(reference.range, offset, source)
    })?;

    let symbol = analysis
        .symbols
        .iter()
        .find(|symbol| symbol.kind == CssSymbolKind::Keyframes && symbol.name == reference.name)?;

    Some(GotoDefinitionResponse::Scalar(Location {
        uri: uri.clone(),
        range: to_lsp_range(symbol.selection_range),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_animation_name_to_keyframes_definition() {
        let uri = Url::parse("file:///test.css").unwrap();
        let source = "@keyframes fade-in { from { opacity: 0; } to { opacity: 1; } }\nwindow { animation-name: fade-in; }";

        let response = definition_for(
            &uri,
            source,
            Position { line: 1, character: 26 },
            &ProjectIndex::default(),
        )
        .unwrap();

        let GotoDefinitionResponse::Scalar(location) = response else {
            panic!("expected scalar definition");
        };
        assert_eq!(location.uri, uri);
        assert_eq!(location.range.start.line, 0);
    }

    #[test]
    fn resolves_selector_id_to_layout_definition() {
        let uri = Url::parse("file:///test.css").unwrap();
        let source = "window#root { color: red; }";
        let mut project_index = ProjectIndex::default();
        project_index.index_app_scope(
            std::path::PathBuf::from("/tmp/layouts/example/index.tsx"),
            vec![(
                std::path::PathBuf::from("/tmp/layouts/example/index.tsx"),
                r#"export default function layout() { return <workspace id="root" /> }"#
                    .to_string(),
            )],
            vec![(std::path::PathBuf::from("/tmp/layouts/example/index.css"), String::new())],
        );

        let response =
            definition_for(&uri, source, Position { line: 0, character: 8 }, &project_index)
                .unwrap();

        let GotoDefinitionResponse::Scalar(location) = response else {
            panic!("expected scalar definition");
        };
        assert_eq!(location.uri.path(), "/tmp/layouts/example/index.tsx");
    }
}
