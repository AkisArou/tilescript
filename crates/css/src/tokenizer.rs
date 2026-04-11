use cssparser::{Parser, ParserInput, Token};

use crate::parse_values::{
    CssDelimiter, CssDimension, CssFunction, CssSimpleBlock, CssSimpleBlockKind, CssValueToken,
};
use crate::parsing::CssParseError;

pub(super) fn parse_value_tokens(input: &str) -> Result<Vec<CssValueToken>, CssParseError> {
    let mut input_buf = ParserInput::new(input);
    let mut parser = Parser::new(&mut input_buf);
    parse_component_values(&mut parser).map_err(|err| CssParseError::InvalidSyntax {
        line: err.location.line,
        column: err.location.column,
    })
}

pub(super) fn parse_component_values<'i, 't>(
    parser: &mut Parser<'i, 't>,
) -> Result<Vec<CssValueToken>, cssparser::ParseError<'i, CssParseError>> {
    let mut components = Vec::new();
    while let Ok(token) = parser.next_including_whitespace_and_comments() {
        let component = match token.clone() {
            Token::Ident(value) => CssValueToken::Ident(value.to_string()),
            Token::QuotedString(value) => CssValueToken::String(value.to_string()),
            Token::Number { value, int_value, .. } => match int_value {
                Some(int) if int as f32 == value => CssValueToken::Integer(int as i64),
                _ => CssValueToken::Number(value),
            },
            Token::Percentage { unit_value, int_value, .. } => {
                let percent = match int_value {
                    Some(int) => int as f32,
                    None => (unit_value * 100.0 * 1_000_000.0).round() / 1_000_000.0,
                };
                CssValueToken::Percentage(percent)
            }
            Token::Dimension { value, unit, int_value, .. } => {
                let value = match int_value {
                    Some(int) if int as f32 == value => int as f32,
                    _ => value,
                };
                CssValueToken::Dimension(CssDimension { value, unit: unit.to_string() })
            }
            Token::WhiteSpace(_) | Token::Comment(_) => CssValueToken::Whitespace,
            Token::Comma => CssValueToken::Delimiter(CssDelimiter::Comma),
            Token::Semicolon => CssValueToken::Delimiter(CssDelimiter::Semicolon),
            Token::Delim('/') => CssValueToken::Delimiter(CssDelimiter::Solidus),
            Token::Delim(ch) => CssValueToken::Unknown(ch.to_string()),
            Token::Function(name) => {
                let value = parser.parse_nested_block(parse_component_values)?;
                CssValueToken::Function(CssFunction { name: name.to_string(), value })
            }
            Token::ParenthesisBlock => {
                let value = parser.parse_nested_block(parse_component_values)?;
                CssValueToken::SimpleBlock(CssSimpleBlock {
                    kind: CssSimpleBlockKind::Parenthesis,
                    value,
                })
            }
            Token::SquareBracketBlock => {
                let value = parser.parse_nested_block(parse_component_values)?;
                CssValueToken::SimpleBlock(CssSimpleBlock {
                    kind: CssSimpleBlockKind::Bracket,
                    value,
                })
            }
            Token::CurlyBracketBlock => {
                let value = parser.parse_nested_block(parse_component_values)?;
                CssValueToken::SimpleBlock(CssSimpleBlock {
                    kind: CssSimpleBlockKind::Brace,
                    value,
                })
            }
            other => CssValueToken::Unknown(format!("{other:?}")),
        };
        components.push(component);
    }
    Ok(components)
}
