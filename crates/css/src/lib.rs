pub mod analysis;
pub mod compile;
pub mod compiled;
pub mod grid;
pub mod language;
pub mod parse_values;
pub mod parsing;
mod query;
pub mod source;
pub mod style;
pub mod tokenizer;

mod stylo_adapter;

pub use compile::{BoxSide, CompiledDeclaration, CssValueError};
pub use compiled::{CompiledDeclarationEntry, CompiledStyleRule, CompiledStyleSheet};
pub use parsing::{CssParseError, parse_stylesheet};
pub use query::{matching_rules, matching_rules_for_element, selector_matches};
pub use source::CssRange;
pub use style::*;
pub use stylo_adapter::{
    LayoutDomTree, LayoutElement, LayoutSelectorImpl, LayoutSelectorParser, StyloAdapterError,
    selector_matches_element,
};

#[doc(hidden)]
pub use stylo_adapter::{
    parse_selector_list, parse_selector_list_from_parser, parse_selector_list_from_parser_relative,
};
