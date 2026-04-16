use std::path::Path;

use tilescript_css::analysis::{
    CssDiagnostic, CssDiagnosticCode, CssDiagnosticSeverity, CssSymbolKind, analyze_stylesheet,
};
use lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range, Url};

use crate::project::ProjectIndex;
use crate::syntax::{position_to_offset, selector_references_in_segment};

pub fn diagnostics_for(uri: &Url, source: &str, project_index: &ProjectIndex) -> Vec<Diagnostic> {
    let analysis = analyze_stylesheet(source);
    let mut diagnostics: Vec<_> = analysis.diagnostics.iter().map(to_lsp_diagnostic).collect();
    diagnostics.extend(project_selector_diagnostics(uri, source, &analysis, project_index));
    diagnostics
}

fn to_lsp_diagnostic(diagnostic: &CssDiagnostic) -> Diagnostic {
    Diagnostic {
        range: Range {
            start: Position {
                line: diagnostic.range.start_line.saturating_sub(1),
                character: diagnostic.range.start_column.saturating_sub(1),
            },
            end: Position {
                line: diagnostic.range.end_line.saturating_sub(1),
                character: diagnostic.range.end_column.saturating_sub(1),
            },
        },
        severity: Some(to_lsp_severity(diagnostic.severity)),
        code: Some(NumberOrString::String(diagnostic_code(diagnostic.code).to_string())),
        source: Some("tilescript-css".to_string()),
        message: diagnostic.message.clone(),
        ..Diagnostic::default()
    }
}

fn to_lsp_severity(severity: CssDiagnosticSeverity) -> DiagnosticSeverity {
    match severity {
        CssDiagnosticSeverity::Error => DiagnosticSeverity::ERROR,
        CssDiagnosticSeverity::Warning => DiagnosticSeverity::WARNING,
        CssDiagnosticSeverity::Information => DiagnosticSeverity::INFORMATION,
    }
}

fn diagnostic_code(code: CssDiagnosticCode) -> &'static str {
    match code {
        CssDiagnosticCode::UnsupportedAtRule => "unsupported-at-rule",
        CssDiagnosticCode::UnsupportedSelector => "unsupported-selector",
        CssDiagnosticCode::UnsupportedProperty => "unsupported-property",
        CssDiagnosticCode::InvalidSyntax => "invalid-syntax",
        CssDiagnosticCode::UnsupportedValue => "unsupported-value",
        CssDiagnosticCode::InapplicableProperty => "inapplicable-property",
        CssDiagnosticCode::UnknownAnimationName => "unknown-animation-name",
        CssDiagnosticCode::UnsupportedAttributeKey => "unsupported-attribute-key",
    }
}

fn project_selector_diagnostics(
    uri: &Url,
    source: &str,
    analysis: &tilescript_css::analysis::CssAnalysis,
    project_index: &ProjectIndex,
) -> Vec<Diagnostic> {
    let Some(path) = crate::uri::path_from_url(uri) else {
        return Vec::new();
    };
    if project_index.is_empty() {
        return Vec::new();
    }

    let mut diagnostics = Vec::new();

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

        diagnostics.extend(selector_reference_diagnostics(
            &path,
            source,
            &source[start..end],
            start,
            project_index,
        ));
    }

    diagnostics
}

fn selector_reference_diagnostics(
    path: &Path,
    source: &str,
    selector: &str,
    selector_offset: usize,
    project_index: &ProjectIndex,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for reference in selector_references_in_segment(selector, selector_offset) {
        match reference.kind {
            crate::syntax::SelectorReferenceKind::Id => {
                if !project_index.has_id_for_path(path, &reference.name) {
                    diagnostics.push(project_selector_diagnostic(
                        source,
                        reference.start,
                        reference.end,
                        "unknown-selector-id",
                        format!(
                            "unknown selector id `#{}` in authored TSX layouts",
                            reference.name
                        ),
                    ));
                }
            }
            crate::syntax::SelectorReferenceKind::Class => {
                if !project_index.has_class_for_path(path, &reference.name) {
                    diagnostics.push(project_selector_diagnostic(
                        source,
                        reference.start,
                        reference.end,
                        "unknown-selector-class",
                        format!(
                            "unknown selector class `.{}` in authored TSX layouts",
                            reference.name
                        ),
                    ));
                }
            }
        }
    }

    diagnostics
}

fn project_selector_diagnostic(
    source: &str,
    start: usize,
    end: usize,
    code: &str,
    message: String,
) -> Diagnostic {
    Diagnostic {
        range: Range {
            start: offset_to_position(source, start),
            end: offset_to_position(source, end),
        },
        severity: Some(DiagnosticSeverity::WARNING),
        code: Some(NumberOrString::String(code.to_string())),
        source: Some("tilescript-css-lsp".to_string()),
        message,
        ..Diagnostic::default()
    }
}

fn offset_to_position(source: &str, offset: usize) -> Position {
    let mut line = 0u32;
    let mut character = 0u32;

    for (index, ch) in source.char_indices() {
        if index == offset {
            return Position { line, character };
        }
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += 1;
        }
    }

    Position { line, character }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn reports_unknown_project_selector_ids_and_classes() {
        let mut project_index = ProjectIndex::default();
        project_index.index_app_scope(
            PathBuf::from("/tmp/layouts/example/index.tsx"),
            vec![(
                PathBuf::from("/tmp/layouts/example/index.tsx"),
                r#"export default function layout() { return <workspace id="root" class="shell" /> }"#
                    .to_string(),
            )],
            vec![(PathBuf::from("/tmp/layouts/example/index.css"), String::new())],
        );

        let diagnostics = diagnostics_for(
            &Url::parse("file:///tmp/layouts/example/index.css").unwrap(),
            "window#missing.shell, group.stack[title=\"#ignored\"] { color: red; }",
            &project_index,
        );

        assert!(diagnostics.iter().any(|diagnostic| diagnostic.code
            == Some(NumberOrString::String("unknown-selector-id".to_string()))));
        assert!(diagnostics.iter().any(|diagnostic| diagnostic.code
            == Some(NumberOrString::String("unknown-selector-class".to_string()))
            && diagnostic.message.contains(".stack")));
        assert!(!diagnostics.iter().any(|diagnostic| diagnostic.message.contains(".shell")));
        assert!(!diagnostics.iter().any(|diagnostic| diagnostic.message.contains("#ignored")));
    }

    #[test]
    fn skips_project_selector_diagnostics_without_layout_index() {
        let diagnostics = diagnostics_for(
            &Url::parse("file:///tmp/index.css").unwrap(),
            "window#missing.shell { color: red; }",
            &ProjectIndex::default(),
        );
        assert!(diagnostics.iter().all(|diagnostic| {
            diagnostic.code != Some(NumberOrString::String("unknown-selector-id".to_string()))
                && diagnostic.code
                    != Some(NumberOrString::String("unknown-selector-class".to_string()))
        }));
    }
}
