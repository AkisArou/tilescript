use std::collections::BTreeMap;

use cssparser::{
    AtRuleParser, CowRcStr, DeclarationParser, Parser, ParserInput, QualifiedRuleParser,
    RuleBodyItemParser, RuleBodyParser,
};

use crate::compile::*;
use crate::language::is_supported_property;
use crate::parse_values::*;
use crate::style::*;
use crate::tokenizer::parse_component_values;

#[derive(Default)]
pub(super) struct GridFallbackDeclarationParser;

impl<'i> DeclarationParser<'i> for GridFallbackDeclarationParser {
    type Declaration = CompiledDeclaration;
    type Error = crate::parsing::CssParseError;

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _declaration_start: &cssparser::ParserState,
    ) -> Result<Self::Declaration, cssparser::ParseError<'i, Self::Error>> {
        let property = name.to_ascii_lowercase();
        if !is_supported_property(&property) {
            return Err(input.new_custom_error(
                crate::parsing::CssParseError::UnsupportedProperty { property },
            ));
        }

        let value_start = input.state();
        let components = parse_component_values(input)?;
        let text = input.slice_from(value_start.position()).trim().to_string();
        let parsed = ParsedDeclaration { property, value: CssValue { text, components } };
        compile_declaration(&parsed)
            .map_err(|error| input.new_custom_error(crate::parsing::CssParseError::CssValue(error)))
    }
}

impl<'i> AtRuleParser<'i> for GridFallbackDeclarationParser {
    type Prelude = ();
    type AtRule = CompiledDeclaration;
    type Error = crate::parsing::CssParseError;
}

impl<'i> QualifiedRuleParser<'i> for GridFallbackDeclarationParser {
    type Prelude = ();
    type QualifiedRule = CompiledDeclaration;
    type Error = crate::parsing::CssParseError;
}

impl<'i> RuleBodyItemParser<'i, CompiledDeclaration, crate::parsing::CssParseError>
    for GridFallbackDeclarationParser
{
    fn parse_declarations(&self) -> bool {
        true
    }

    fn parse_qualified(&self) -> bool {
        false
    }
}

pub(super) fn parse_grid_fallback_declarations(
    input: &str,
) -> Result<Vec<CompiledDeclaration>, crate::parsing::CssParseError> {
    let mut input_buf = ParserInput::new(input);
    let mut parser_input = Parser::new(&mut input_buf);
    let mut parser = GridFallbackDeclarationParser;
    let mut body = RuleBodyParser::new(&mut parser_input, &mut parser);
    let mut declarations = Vec::new();
    while let Some(item) = body.next() {
        match item {
            Ok(declaration) => declarations.push(declaration),
            Err((err, _)) => {
                return Err(match err.kind {
                    cssparser::ParseErrorKind::Custom(error) => error,
                    _ => crate::parsing::CssParseError::InvalidSyntax {
                        line: err.location.line,
                        column: err.location.column,
                    },
                });
            }
        }
    }
    Ok(declarations)
}

pub(super) fn parse_grid_tracks(
    property: &str,
    value: &CssValue,
) -> Result<GridTemplate, CssValueError> {
    let components = normalized_components_owned(value);
    if components.is_empty() {
        return Err(invalid_value(property, text_for_value(value)));
    }

    let mut index = 0;
    let mut line_names = Vec::new();
    let mut template_components = Vec::new();
    let mut pending_line_names = Vec::new();

    while index < components.len() {
        match &components[index] {
            CssValueToken::SimpleBlock(block) if block.kind == CssSimpleBlockKind::Bracket => {
                pending_line_names.extend(parse_line_name_block(property, block)?);
                index += 1;
            }
            token => {
                let component = match token {
                    CssValueToken::Function(function) if function.name == "repeat" => {
                        GridTemplateComponent::Repeat(parse_grid_track_repeat(property, function)?)
                    }
                    _ => GridTemplateComponent::Single(parse_grid_track(
                        property,
                        &CssValue { text: component_text(token), components: vec![token.clone()] },
                    )?),
                };

                line_names.push(std::mem::take(&mut pending_line_names));
                template_components.push(component);
                index += 1;
            }
        }
    }

    line_names.push(pending_line_names);

    Ok(GridTemplate { components: template_components, line_names })
}

pub(super) fn parse_grid_auto_tracks(
    property: &str,
    value: &CssValue,
) -> Result<Vec<GridTrackValue>, CssValueError> {
    let template = parse_grid_tracks(property, value)?;
    if template.line_names.iter().any(|names| !names.is_empty()) {
        return Err(invalid_value(property, text_for_value(value)));
    }

    template
        .components
        .into_iter()
        .map(|component| match component {
            GridTemplateComponent::Single(track) => Ok(track),
            GridTemplateComponent::Repeat(_) => Err(invalid_value(property, text_for_value(value))),
        })
        .collect()
}

pub(super) fn parse_grid_track(
    property: &str,
    value: &CssValue,
) -> Result<GridTrackValue, CssValueError> {
    let components = normalized_components(value);
    match components.as_slice() {
        [CssValueToken::Ident(ident)] if ident == "auto" => Ok(GridTrackValue::Auto),
        [CssValueToken::Ident(ident)] if ident == "min-content" => Ok(GridTrackValue::MinContent),
        [CssValueToken::Ident(ident)] if ident == "max-content" => Ok(GridTrackValue::MaxContent),
        [CssValueToken::Dimension(dimension)] if dimension.unit.eq_ignore_ascii_case("fr") => {
            Ok(GridTrackValue::Fraction(dimension.value))
        }
        [CssValueToken::Dimension(_)] | [CssValueToken::Percentage(_)] => {
            parse_length_percentage(property, value).map(GridTrackValue::LengthPercentage)
        }
        [CssValueToken::Function(function)] if function.name == "fit-content" => {
            parse_length_percentage(property, &function_args_value(function))
                .map(GridTrackValue::FitContent)
        }
        [CssValueToken::Function(function)] if function.name == "minmax" => {
            let args = split_function_args(function);
            if args.len() != 2 {
                return Err(invalid_value(property, text_for_value(value)));
            }
            Ok(GridTrackValue::MinMax(
                parse_grid_track_min(property, &args[0])?,
                parse_grid_track_max(property, &args[1])?,
            ))
        }
        _ => Err(invalid_value(property, text_for_value(value))),
    }
}

pub(super) fn parse_line_name_block(
    property: &str,
    block: &CssSimpleBlock,
) -> Result<Vec<String>, CssValueError> {
    let names = block
        .value
        .iter()
        .filter_map(|component| match component {
            CssValueToken::Whitespace => None,
            CssValueToken::Ident(name) => Some(Ok(name.clone())),
            _ => Some(Err(invalid_value(property, &components_to_text(&block.value)))),
        })
        .collect::<Result<Vec<_>, _>>()?;

    if names.is_empty() {
        return Err(invalid_value(property, &components_to_text(&block.value)));
    }

    Ok(names)
}

pub(super) fn parse_grid_track_repeat(
    property: &str,
    function: &CssFunction,
) -> Result<GridTrackRepeat, CssValueError> {
    let args = split_function_args(function);
    if args.len() < 2 {
        return Err(invalid_value(
            property,
            &format!("{}({})", function.name, components_to_text(&function.value)),
        ));
    }

    let count = parse_grid_repetition_count(property, &args[0])?;
    let tracks = parse_grid_tracks(
        property,
        &CssValue {
            text: args[1..].iter().map(|arg| arg.text.as_str()).collect::<Vec<_>>().join(" "),
            components: args[1..]
                .iter()
                .flat_map(|arg| {
                    let mut parts = arg.components.clone();
                    parts.push(CssValueToken::Whitespace);
                    parts
                })
                .collect(),
        },
    )?;

    Ok(GridTrackRepeat {
        count,
        tracks: tracks
            .components
            .into_iter()
            .map(|component| match component {
                GridTemplateComponent::Single(track) => Ok(track),
                GridTemplateComponent::Repeat(_) => {
                    Err(invalid_value(property, &components_to_text(&function.value)))
                }
            })
            .collect::<Result<Vec<_>, _>>()?,
        line_names: tracks.line_names,
    })
}

pub(super) fn parse_grid_repetition_count(
    property: &str,
    value: &CssValue,
) -> Result<GridRepetitionCount, CssValueError> {
    let components = normalized_components(value);
    match components.as_slice() {
        [CssValueToken::Ident(ident)] if ident == "auto-fill" => Ok(GridRepetitionCount::AutoFill),
        [CssValueToken::Ident(ident)] if ident == "auto-fit" => Ok(GridRepetitionCount::AutoFit),
        [CssValueToken::Integer(count)] => u16::try_from(*count)
            .map(GridRepetitionCount::Count)
            .map_err(|_| invalid_value(property, text_for_value(value))),
        [CssValueToken::Number(count)] if count.fract() == 0.0 => u16::try_from(*count as i64)
            .map(GridRepetitionCount::Count)
            .map_err(|_| invalid_value(property, text_for_value(value))),
        _ => Err(invalid_value(property, text_for_value(value))),
    }
}

pub(super) fn parse_grid_track_min(
    property: &str,
    value: &CssValue,
) -> Result<GridTrackMinValue, CssValueError> {
    match parse_grid_track(property, value)? {
        GridTrackValue::Auto => Ok(GridTrackMinValue::Auto),
        GridTrackValue::MinContent => Ok(GridTrackMinValue::MinContent),
        GridTrackValue::MaxContent => Ok(GridTrackMinValue::MaxContent),
        GridTrackValue::LengthPercentage(value) => Ok(GridTrackMinValue::LengthPercentage(value)),
        _ => Err(invalid_value(property, text_for_value(value))),
    }
}

pub(super) fn parse_grid_track_max(
    property: &str,
    value: &CssValue,
) -> Result<GridTrackMaxValue, CssValueError> {
    match parse_grid_track(property, value)? {
        GridTrackValue::Auto => Ok(GridTrackMaxValue::Auto),
        GridTrackValue::MinContent => Ok(GridTrackMaxValue::MinContent),
        GridTrackValue::MaxContent => Ok(GridTrackMaxValue::MaxContent),
        GridTrackValue::LengthPercentage(value) => Ok(GridTrackMaxValue::LengthPercentage(value)),
        GridTrackValue::Fraction(value) => Ok(GridTrackMaxValue::Fraction(value)),
        GridTrackValue::FitContent(value) => Ok(GridTrackMaxValue::FitContent(value)),
        GridTrackValue::MinMax(_, _) => Err(invalid_value(property, text_for_value(value))),
    }
}

pub(super) fn parse_grid_line_shorthand(
    property: &str,
    value: &CssValue,
) -> Result<Line<GridPlacementValue>, CssValueError> {
    let components = normalized_components(value);
    let slash = components
        .iter()
        .position(|component| matches!(component, CssValueToken::Delimiter(CssDelimiter::Solidus)));

    match slash {
        Some(index) => Ok(Line {
            start: parse_grid_placement(property, &slice_to_value(&components[..index]))?,
            end: parse_grid_placement(property, &slice_to_value(&components[index + 1..]))?,
        }),
        None => Ok(Line {
            start: parse_grid_placement(property, &slice_to_value(&components))?,
            end: GridPlacementValue::Auto,
        }),
    }
}

pub(super) fn parse_grid_line_side(
    property: &str,
    value: &CssValue,
) -> Result<Line<GridPlacementValue>, CssValueError> {
    let placement = parse_grid_placement(property, value)?;
    Ok(match property {
        "grid-row-start" | "grid-column-start" => {
            Line { start: placement, end: GridPlacementValue::Auto }
        }
        "grid-row-end" | "grid-column-end" => {
            Line { start: GridPlacementValue::Auto, end: placement }
        }
        _ => return Err(invalid_value(property, text_for_value(value))),
    })
}

pub(super) fn parse_grid_placement(
    property: &str,
    value: &CssValue,
) -> Result<GridPlacementValue, CssValueError> {
    let components = normalized_components(value);
    match components.as_slice() {
        [CssValueToken::Ident(ident)] if ident == "auto" => Ok(GridPlacementValue::Auto),
        [CssValueToken::Ident(span), CssValueToken::Integer(number)] if span == "span" => {
            u16::try_from(*number)
                .map(GridPlacementValue::Span)
                .map_err(|_| invalid_value(property, text_for_value(value)))
        }
        [CssValueToken::Ident(span), CssValueToken::Ident(name)] if span == "span" => {
            Ok(GridPlacementValue::NamedSpan(name.clone(), 1))
        }
        [
            CssValueToken::Ident(span),
            CssValueToken::Integer(number),
            CssValueToken::Ident(name),
        ] if span == "span" => u16::try_from(*number)
            .map(|count| GridPlacementValue::NamedSpan(name.clone(), count))
            .map_err(|_| invalid_value(property, text_for_value(value))),
        [CssValueToken::Ident(span), CssValueToken::Number(number)]
            if span == "span" && number.fract() == 0.0 =>
        {
            u16::try_from(*number as i64)
                .map(GridPlacementValue::Span)
                .map_err(|_| invalid_value(property, text_for_value(value)))
        }
        [CssValueToken::Ident(span), CssValueToken::Number(number), CssValueToken::Ident(name)]
            if span == "span" && number.fract() == 0.0 =>
        {
            u16::try_from(*number as i64)
                .map(|count| GridPlacementValue::NamedSpan(name.clone(), count))
                .map_err(|_| invalid_value(property, text_for_value(value)))
        }
        [CssValueToken::Ident(name)] => Ok(GridPlacementValue::NamedLine(name.clone(), 1)),
        [CssValueToken::Ident(name), CssValueToken::Integer(index)] => i16::try_from(*index)
            .map(|line_index| GridPlacementValue::NamedLine(name.clone(), line_index))
            .map_err(|_| invalid_value(property, text_for_value(value))),
        [CssValueToken::Integer(index), CssValueToken::Ident(name)] => i16::try_from(*index)
            .map(|line_index| GridPlacementValue::NamedLine(name.clone(), line_index))
            .map_err(|_| invalid_value(property, text_for_value(value))),
        [CssValueToken::Integer(number)] => i16::try_from(*number)
            .map(GridPlacementValue::Line)
            .map_err(|_| invalid_value(property, text_for_value(value))),
        [CssValueToken::Number(number)] if number.fract() == 0.0 => i16::try_from(*number as i64)
            .map(GridPlacementValue::Line)
            .map_err(|_| invalid_value(property, text_for_value(value))),
        _ => Err(invalid_value(property, text_for_value(value))),
    }
}

pub(super) fn parse_grid_template_areas(
    property: &str,
    value: &CssValue,
) -> Result<Vec<GridTemplateArea>, CssValueError> {
    let rows = normalized_components(value)
        .into_iter()
        .map(|component| match component {
            CssValueToken::String(row) => Ok(row),
            _ => Err(invalid_value(property, text_for_value(value))),
        })
        .collect::<Result<Vec<_>, _>>()?;

    if rows.is_empty() {
        return Err(invalid_value(property, text_for_value(value)));
    }

    let mut cells = BTreeMap::<String, Vec<(u16, u16)>>::new();
    let mut columns_per_row = None;

    for (row_index, row) in rows.iter().enumerate() {
        let columns = row.split_whitespace().collect::<Vec<_>>();
        if columns.is_empty() {
            return Err(invalid_value(property, text_for_value(value)));
        }

        match columns_per_row {
            Some(expected) if expected != columns.len() => {
                return Err(invalid_value(property, text_for_value(value)));
            }
            None => columns_per_row = Some(columns.len()),
            _ => {}
        }

        for (column_index, name) in columns.into_iter().enumerate() {
            if name == "." {
                continue;
            }

            cells
                .entry(name.to_owned())
                .or_default()
                .push((row_index as u16 + 1, column_index as u16 + 1));
        }
    }

    cells
        .into_iter()
        .map(|(name, cells)| {
            let row_start = cells.iter().map(|(row, _)| *row).min().unwrap();
            let row_end = cells.iter().map(|(row, _)| *row).max().unwrap() + 1;
            let column_start = cells.iter().map(|(_, column)| *column).min().unwrap();
            let column_end = cells.iter().map(|(_, column)| *column).max().unwrap() + 1;

            let expected =
                ((row_end - row_start) as usize) * ((column_end - column_start) as usize);
            if expected != cells.len() {
                return Err(invalid_value(property, text_for_value(value)));
            }

            for row in row_start..row_end {
                for column in column_start..column_end {
                    if !cells.contains(&(row, column)) {
                        return Err(invalid_value(property, text_for_value(value)));
                    }
                }
            }

            Ok(GridTemplateArea { name, row_start, row_end, column_start, column_end })
        })
        .collect()
}
