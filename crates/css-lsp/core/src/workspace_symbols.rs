use lsp_types::{Location, SymbolInformation, SymbolKind};

use crate::project::{ProjectIndex, ProjectSelectorKind};
use crate::ranking::similarity_score;

pub fn workspace_symbols_for(query: &str, project_index: &ProjectIndex) -> Vec<SymbolInformation> {
    let mut selectors = project_index.selector_symbols_matching(query);
    selectors.sort_by(|left, right| {
        let left_label = match left.kind {
            ProjectSelectorKind::Id => format!("#{}", left.name),
            ProjectSelectorKind::Class => format!(".{}", left.name),
        };
        let right_label = match right.kind {
            ProjectSelectorKind::Id => format!("#{}", right.name),
            ProjectSelectorKind::Class => format!(".{}", right.name),
        };

        similarity_score(query, &right.name)
            .cmp(&similarity_score(query, &left.name))
            .then_with(|| left_label.cmp(&right_label))
    });

    selectors
        .into_iter()
        .map(|selector| symbol_information(selector.kind, selector.name, selector.location))
        .collect()
}

fn symbol_information(
    kind: ProjectSelectorKind,
    name: String,
    location: Location,
) -> SymbolInformation {
    #[allow(deprecated)]
    SymbolInformation {
        name: match kind {
            ProjectSelectorKind::Id => format!("#{name}"),
            ProjectSelectorKind::Class => format!(".{name}"),
        },
        kind: match kind {
            ProjectSelectorKind::Id => SymbolKind::CONSTANT,
            ProjectSelectorKind::Class => SymbolKind::CLASS,
        },
        tags: None,
        deprecated: None,
        location,
        container_name: Some("authored selector".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::ProjectIndex;
    use std::path::PathBuf;

    #[test]
    fn returns_matching_project_selector_symbols() {
        let mut project_index = ProjectIndex::default();
        project_index.index_app_scope(
            PathBuf::from("/tmp/layouts/example/index.tsx"),
            vec![(
                PathBuf::from("/tmp/layouts/example/index.tsx"),
                r#"export default function layout() { return <workspace id="root" class="shell stack-group" /> }"#
                    .to_string(),
            )],
            vec![(PathBuf::from("/tmp/layouts/example/index.css"), String::new())],
        );

        let symbols = workspace_symbols_for("stack", &project_index);

        assert!(symbols.iter().any(|symbol| symbol.name == ".stack-group"));
        assert!(!symbols.iter().any(|symbol| symbol.name == "#root"));
    }

    #[test]
    fn ranks_workspace_symbols_by_similarity() {
        let mut project_index = ProjectIndex::default();
        project_index.index_app_scope(
            PathBuf::from("/tmp/layouts/example/index.tsx"),
            vec![(
                PathBuf::from("/tmp/layouts/example/index.tsx"),
                r#"export default function layout() { return <workspace class="shell stack-group shell-stack" /> }"#
                    .to_string(),
            )],
            vec![(PathBuf::from("/tmp/layouts/example/index.css"), String::new())],
        );

        let symbols = workspace_symbols_for("stack", &project_index);

        assert_eq!(symbols.first().map(|symbol| symbol.name.as_str()), Some(".stack-group"));
    }
}
