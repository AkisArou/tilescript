use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, Diagnostic, NumberOrString, TextEdit, Url,
    WorkspaceEdit,
};

use crate::project::ProjectIndex;
use crate::ranking::similarity_score;

pub fn code_actions_for(
    uri: &Url,
    project_index: &ProjectIndex,
    diagnostics: &[Diagnostic],
) -> Vec<CodeActionOrCommand> {
    let Some(path) = crate::uri::path_from_url(uri) else {
        return Vec::new();
    };

    let mut actions = Vec::new();

    for diagnostic in diagnostics {
        let Some(NumberOrString::String(code)) = &diagnostic.code else {
            continue;
        };

        match code.as_str() {
            "unknown-selector-id" => {
                let unknown_name = diagnostic
                    .message
                    .split('`')
                    .nth(1)
                    .map(|name| name.trim_start_matches('#'))
                    .unwrap_or_default();
                let mut candidates = project_index.ids_for_path(&path);
                candidates.sort_by_key(|candidate| {
                    std::cmp::Reverse(similarity_score(unknown_name, candidate))
                });

                for candidate in candidates {
                    actions.push(CodeActionOrCommand::CodeAction(replace_selector_action(
                        uri,
                        diagnostic,
                        format!("Replace with `#{candidate}`"),
                        format!("#{candidate}"),
                    )));
                }
            }
            "unknown-selector-class" => {
                let unknown_name = diagnostic
                    .message
                    .split('`')
                    .nth(1)
                    .map(|name| name.trim_start_matches('.'))
                    .unwrap_or_default();
                let mut candidates = project_index.classes_for_path(&path);
                candidates.sort_by_key(|candidate| {
                    std::cmp::Reverse(similarity_score(unknown_name, candidate))
                });

                for candidate in candidates {
                    actions.push(CodeActionOrCommand::CodeAction(replace_selector_action(
                        uri,
                        diagnostic,
                        format!("Replace with `.{candidate}`"),
                        format!(".{candidate}"),
                    )));
                }
            }
            _ => {}
        }
    }

    actions
}

fn replace_selector_action(
    uri: &Url,
    diagnostic: &Diagnostic,
    title: String,
    replacement: String,
) -> CodeAction {
    CodeAction {
        title,
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(
                [(uri.clone(), vec![TextEdit { range: diagnostic.range, new_text: replacement }])]
                    .into_iter()
                    .collect(),
            ),
            document_changes: None,
            change_annotations: None,
        }),
        ..CodeAction::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::diagnostics_for;
    use std::path::PathBuf;

    #[test]
    fn suggests_scoped_replacements_for_unknown_selector_class() {
        let uri = Url::parse("file:///tmp/layouts/example/index.css").unwrap();
        let mut project_index = ProjectIndex::default();
        project_index.index_app_scope(
            PathBuf::from("/tmp/layouts/example/index.tsx"),
            vec![(
                PathBuf::from("/tmp/layouts/example/index.tsx"),
                r#"export default function layout() { return <workspace class="shell stack-group" /> }"#
                    .to_string(),
            )],
            vec![(
                PathBuf::from("/tmp/layouts/example/index.css"),
                "window.missing { color: red; }".to_string(),
            )],
        );

        let diagnostics = diagnostics_for(&uri, "window.missing { color: red; }", &project_index);
        let actions = code_actions_for(&uri, &project_index, &diagnostics);

        assert!(actions.iter().any(|action| matches!(action,
            CodeActionOrCommand::CodeAction(action)
                if action.title == "Replace with `.shell`"
        )));
        assert!(actions.iter().any(|action| matches!(action,
            CodeActionOrCommand::CodeAction(action)
                if action.title == "Replace with `.stack-group`"
        )));
    }

    #[test]
    fn ranks_closer_selector_replacements_first() {
        let uri = Url::parse("file:///tmp/layouts/example/index.css").unwrap();
        let mut project_index = ProjectIndex::default();
        project_index.index_app_scope(
            std::path::PathBuf::from("/tmp/layouts/example/index.tsx"),
            vec![(
                std::path::PathBuf::from("/tmp/layouts/example/index.tsx"),
                r#"export default function layout() { return <workspace class="shell stack-group shell-stack" /> }"#
                    .to_string(),
            )],
            vec![(
                std::path::PathBuf::from("/tmp/layouts/example/index.css"),
                "window.stack { color: red; }".to_string(),
            )],
        );

        let diagnostics = diagnostics_for(&uri, "window.stack { color: red; }", &project_index);
        let actions = code_actions_for(&uri, &project_index, &diagnostics);

        let first_title = actions.into_iter().find_map(|action| match action {
            CodeActionOrCommand::CodeAction(action) => Some(action.title),
            _ => None,
        });
        assert_eq!(first_title.as_deref(), Some("Replace with `.stack-group`"));
    }
}
