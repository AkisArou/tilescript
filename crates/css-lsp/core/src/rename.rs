use lsp_types::{Position, TextEdit, Url, WorkspaceEdit};

use crate::project::{ProjectIndex, ProjectSelectorKind};
use crate::references::references_for;
use crate::syntax::{position_to_offset, selector_reference_at_offset};

pub fn rename_for(
    uri: &Url,
    source: &str,
    position: Position,
    new_name: &str,
    project_index: &ProjectIndex,
    documents: &[(Url, String)],
) -> Option<WorkspaceEdit> {
    let offset = position_to_offset(source, position)?;
    let path = crate::uri::path_from_url(uri);

    if let Some(path) = path.as_deref()
        && let Some(selector) = project_index.selector_at(path, offset)
    {
        return rename_project_selector(
            uri,
            source,
            position,
            new_name,
            path,
            selector.kind,
            &selector.name,
            project_index,
            documents,
        );
    }

    if let Some(selector) = selector_reference_at_offset(source, offset) {
        let path = path.as_deref()?;
        return rename_project_selector(
            uri,
            source,
            position,
            new_name,
            path,
            match selector.kind {
                crate::syntax::SelectorReferenceKind::Id => ProjectSelectorKind::Id,
                crate::syntax::SelectorReferenceKind::Class => ProjectSelectorKind::Class,
            },
            &selector.name,
            project_index,
            documents,
        );
    }

    None
}

fn rename_project_selector(
    uri: &Url,
    source: &str,
    position: Position,
    new_name: &str,
    path: &std::path::Path,
    kind: ProjectSelectorKind,
    name: &str,
    project_index: &ProjectIndex,
    documents: &[(Url, String)],
) -> Option<WorkspaceEdit> {
    let references = references_for(uri, source, position, true, project_index, documents);
    let definition_edits = project_index.selector_rename_locations_for_path(path, kind, name);

    let mut changes: std::collections::HashMap<Url, Vec<TextEdit>> =
        std::collections::HashMap::new();

    for location in definition_edits.into_iter().chain(references) {
        changes
            .entry(location.uri)
            .or_default()
            .push(TextEdit { range: location.range, new_text: new_name.to_string() });
    }

    if changes.is_empty() {
        return None;
    }

    Some(WorkspaceEdit { changes: Some(changes), document_changes: None, change_annotations: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renames_selector_id_across_css_and_tsx_scope() {
        let css_uri = Url::parse("file:///tmp/layouts/example/index.css").unwrap();
        let css_source = "window#root { display: flex; }\ngroup#root { gap: 8px; }";
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
                css_source.to_string(),
            )],
        );

        let edit = rename_for(
            &css_uri,
            css_source,
            Position { line: 0, character: 8 },
            "main",
            &project_index,
            &[],
        )
        .unwrap();

        let changes = edit.changes.unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes.get(&css_uri).unwrap().len(), 2);
        assert!(
            changes.values().flat_map(|edits| edits.iter()).all(|edit| edit.new_text == "main")
        );
    }

    #[test]
    fn renames_selector_id_from_tsx_definition_across_scope() {
        let tsx_uri = Url::parse("file:///tmp/layouts/example/index.tsx").unwrap();
        let css_source = "window#root { display: flex; }\ngroup#root { gap: 8px; }";
        let tsx_source = r#"export default function layout() { return <workspace id="root" /> }"#;
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

        let edit = rename_for(
            &tsx_uri,
            tsx_source,
            Position { line: 0, character: 60 },
            "main",
            &project_index,
            &[],
        )
        .unwrap();

        let changes = edit.changes.unwrap();
        assert_eq!(changes.len(), 2);
        assert!(
            changes.values().flat_map(|edits| edits.iter()).all(|edit| edit.new_text == "main")
        );
    }
}
