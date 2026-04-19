use lsp_types::{DocumentSymbol, Position, Range, SymbolKind};
use tilescript_css::analysis::{CssRange, CssSymbolKind, analyze_stylesheet};

pub fn document_symbols_for(source: &str) -> Vec<DocumentSymbol> {
    let analysis = analyze_stylesheet(source);
    analysis.symbols.iter().map(to_document_symbol).collect()
}

fn to_document_symbol(symbol: &tilescript_css::analysis::CssSymbol) -> DocumentSymbol {
    #[allow(deprecated)]
    DocumentSymbol {
        name: symbol.name.clone(),
        detail: None,
        kind: match symbol.kind {
            CssSymbolKind::Rule => SymbolKind::OBJECT,
        },
        tags: None,
        deprecated: None,
        range: to_lsp_range(symbol.range),
        selection_range: to_lsp_range(symbol.selection_range),
        children: None,
    }
}

fn to_lsp_range(range: CssRange) -> Range {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_rule_symbols() {
        let symbols = document_symbols_for("window { display: flex; }\ngroup { gap: 8px; }");

        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "window");
        assert_eq!(symbols[0].kind, SymbolKind::OBJECT);
        assert_eq!(symbols[1].name, "group");
        assert_eq!(symbols[1].kind, SymbolKind::OBJECT);
    }
}
