use tilescript_css::analysis::{CssSymbolKind, analyze_stylesheet};
use tilescript_css::language::{
    attribute_key_specs, property_spec, property_specs, pseudo_class_specs, pseudo_element_specs,
};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionResponse, Documentation, InsertTextFormat,
    MarkupContent, MarkupKind, Position, Url,
};

use crate::project::ProjectIndex;
use crate::syntax::{CursorContext, cursor_context, enclosing_property_name};

pub fn completions_for(
    uri: &Url,
    source: &str,
    position: Position,
    project_index: &ProjectIndex,
) -> Option<CompletionResponse> {
    let (context, offset, _) = cursor_context(source, position)?;

    let items = match context {
        CursorContext::PropertyName => property_items(),
        CursorContext::PropertyValue => property_value_items(source, offset),
        CursorContext::SelectorId => selector_id_items(uri, project_index),
        CursorContext::SelectorClass => selector_class_items(uri, project_index),
        CursorContext::PseudoClass => pseudo_class_items(),
        CursorContext::PseudoElement => pseudo_element_items(),
        CursorContext::AttributeKey => attribute_key_items(),
        CursorContext::None => return None,
    };

    Some(CompletionResponse::Array(items))
}

fn property_items() -> Vec<CompletionItem> {
    property_specs()
        .iter()
        .map(|spec| CompletionItem {
            label: spec.name.to_string(),
            kind: Some(CompletionItemKind::PROPERTY),
            documentation: Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: spec.hover.to_string(),
            })),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            ..CompletionItem::default()
        })
        .collect()
}

fn property_value_items(source: &str, offset: usize) -> Vec<CompletionItem> {
    let Some(property_name) = enclosing_property_name(source, offset) else {
        return Vec::new();
    };

    if property_name == "animation-name" {
        let analysis = analyze_stylesheet(source);
        return analysis
            .symbols
            .iter()
            .filter(|symbol| symbol.kind == CssSymbolKind::Keyframes)
            .map(|symbol| CompletionItem {
                label: symbol.name.clone(),
                kind: Some(CompletionItemKind::REFERENCE),
                documentation: Some(Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: "Known `@keyframes` in the current document.".to_string(),
                })),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                ..CompletionItem::default()
            })
            .collect();
    }

    property_spec(&property_name)
        .map(|spec| {
            spec.value_keywords
                .iter()
                .map(|value| CompletionItem {
                    label: (*value).to_string(),
                    kind: Some(CompletionItemKind::VALUE),
                    documentation: Some(Documentation::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: format!("Keyword value for `{}`.", spec.name),
                    })),
                    insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                    ..CompletionItem::default()
                })
                .collect()
        })
        .unwrap_or_default()
}

fn pseudo_class_items() -> Vec<CompletionItem> {
    pseudo_class_specs()
        .iter()
        .map(|spec| CompletionItem {
            label: spec.name.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            documentation: Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: spec.hover.to_string(),
            })),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            ..CompletionItem::default()
        })
        .collect()
}

fn selector_id_items(uri: &Url, project_index: &ProjectIndex) -> Vec<CompletionItem> {
    let ids = crate::uri::path_from_url(uri)
        .map(|path| project_index.ids_for_path(&path))
        .unwrap_or_else(|| project_index.ids().cloned().collect());

    ids.iter()
        .map(|id| CompletionItem {
            label: format!("#{id}"),
            kind: Some(CompletionItemKind::REFERENCE),
            documentation: Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: "Known layout `id` from authored TSX layouts.".to_string(),
            })),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            ..CompletionItem::default()
        })
        .collect()
}

fn selector_class_items(uri: &Url, project_index: &ProjectIndex) -> Vec<CompletionItem> {
    let classes = crate::uri::path_from_url(uri)
        .map(|path| project_index.classes_for_path(&path))
        .unwrap_or_else(|| project_index.classes().cloned().collect());

    classes
        .iter()
        .map(|class_name| CompletionItem {
            label: format!(".{class_name}"),
            kind: Some(CompletionItemKind::REFERENCE),
            documentation: Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: "Known layout `class` from authored TSX layouts.".to_string(),
            })),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            ..CompletionItem::default()
        })
        .collect()
}

fn pseudo_element_items() -> Vec<CompletionItem> {
    pseudo_element_specs()
        .iter()
        .map(|spec| CompletionItem {
            label: spec.name.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            documentation: Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: spec.hover.to_string(),
            })),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            ..CompletionItem::default()
        })
        .collect()
}

fn attribute_key_items() -> Vec<CompletionItem> {
    attribute_key_specs()
        .iter()
        .map(|spec| CompletionItem {
            label: spec.name.to_string(),
            kind: Some(CompletionItemKind::FIELD),
            documentation: Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: spec.hover.to_string(),
            })),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            ..CompletionItem::default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completes_property_names() {
        let response = completions_for(
            &Url::parse("file:///test.css").unwrap(),
            "window { tex }",
            Position { line: 0, character: 11 },
            &ProjectIndex::default(),
        )
        .unwrap();

        let CompletionResponse::Array(items) = response else {
            panic!("expected array response");
        };
        assert!(items.iter().any(|item| item.label == "text-align"));
    }

    #[test]
    fn completes_pseudo_classes() {
        let response = completions_for(
            &Url::parse("file:///test.css").unwrap(),
            "window:fo",
            Position { line: 0, character: 8 },
            &ProjectIndex::default(),
        )
        .unwrap();

        let CompletionResponse::Array(items) = response else {
            panic!("expected array response");
        };
        assert!(items.iter().any(|item| item.label == "focused"));
    }

    #[test]
    fn completes_attribute_keys() {
        let response = completions_for(
            &Url::parse("file:///test.css").unwrap(),
            "window[ap",
            Position { line: 0, character: 8 },
            &ProjectIndex::default(),
        )
        .unwrap();

        let CompletionResponse::Array(items) = response else {
            panic!("expected array response");
        };
        assert!(items.iter().any(|item| item.label == "app_id"));
    }

    #[test]
    fn completes_animation_names_from_known_keyframes() {
        let response = completions_for(
            &Url::parse("file:///test.css").unwrap(),
            "@keyframes fade-in { from { opacity: 0; } to { opacity: 1; } }\nwindow { animation-name: fa }",
            Position { line: 1, character: 27 },
            &ProjectIndex::default(),
        )
        .unwrap();

        let CompletionResponse::Array(items) = response else {
            panic!("expected array response");
        };
        assert!(items.iter().any(|item| item.label == "fade-in"));
    }

    #[test]
    fn completes_property_value_keywords() {
        let response = completions_for(
            &Url::parse("file:///test.css").unwrap(),
            "window { text-align: ce }",
            Position { line: 0, character: 22 },
            &ProjectIndex::default(),
        )
        .unwrap();

        let CompletionResponse::Array(items) = response else {
            panic!("expected array response");
        };
        assert!(items.iter().any(|item| item.label == "center"));
    }

    #[test]
    fn completes_known_layout_ids_and_classes() {
        let mut project_index = ProjectIndex::default();
        project_index.index_app_scope(
            std::path::PathBuf::from("/tmp/layouts/example/index.tsx"),
            vec![(
                std::path::PathBuf::from("/tmp/layouts/example/index.tsx"),
                r#"export default function layout() { return <workspace id="root" class="shell main" /> }"#
                    .to_string(),
            )],
            vec![(
                std::path::PathBuf::from("/tmp/layouts/example/index.css"),
                String::new(),
            )],
        );

        let id_response = completions_for(
            &Url::parse("file:///tmp/layouts/example/index.css").unwrap(),
            "window#",
            Position { line: 0, character: 7 },
            &project_index,
        )
        .unwrap();
        let class_response = completions_for(
            &Url::parse("file:///tmp/layouts/example/index.css").unwrap(),
            "window.",
            Position { line: 0, character: 7 },
            &project_index,
        )
        .unwrap();

        let CompletionResponse::Array(id_items) = id_response else {
            panic!("expected array response");
        };
        let CompletionResponse::Array(class_items) = class_response else {
            panic!("expected array response");
        };

        assert!(id_items.iter().any(|item| item.label == "#root"));
        assert!(!id_items.iter().any(|item| item.label == ".shell"));
        assert!(class_items.iter().any(|item| item.label == ".shell"));
        assert!(!class_items.iter().any(|item| item.label == "#root"));
    }
}
