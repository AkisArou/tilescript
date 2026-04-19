use lsp_types::{GotoDefinitionResponse, Position, Url};

use crate::project::{ProjectIndex, ProjectSelectorKind};
use crate::syntax::position_to_offset;
use crate::syntax::selector_reference_at_offset;

pub fn definition_for(
    uri: &Url,
    source: &str,
    position: Position,
    project_index: &ProjectIndex,
) -> Option<GotoDefinitionResponse> {
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

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_selector_id_to_layout_definition() {
        let uri = Url::parse("file:///test.css").unwrap();
        let source = "window#root { display: flex; }";
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
