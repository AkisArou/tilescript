use std::collections::BTreeMap;

use crate::compile::*;
use crate::parse_values::*;
use crate::style::*;

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

pub(super) fn parse_grid_template_shorthand(
    property: &str,
    value: &CssValue,
) -> Result<(Option<GridTemplate>, Option<GridTemplate>, Option<Vec<GridTemplateArea>>), CssValueError>
{
    let components = normalized_components(value);

    match components.as_slice() {
        [CssValueToken::Ident(ident)] if ident == "none" => return Ok((None, None, None)),
        [] => return Err(invalid_value(property, text_for_value(value))),
        _ => {}
    }

    if components.iter().any(|component| matches!(component, CssValueToken::String(_))) {
        return parse_grid_template_area_shorthand(property, &components);
    }

    let slash_positions = slash_positions(&components);
    if slash_positions.len() != 1 {
        return Err(invalid_value(property, text_for_value(value)));
    }

    let slash = slash_positions[0];
    if slash == 0 || slash + 1 >= components.len() {
        return Err(invalid_value(property, text_for_value(value)));
    }

    Ok((
        Some(parse_grid_tracks(property, &slice_to_value(&components[..slash]))?),
        Some(parse_grid_tracks(property, &slice_to_value(&components[slash + 1..]))?),
        None,
    ))
}

pub(super) fn parse_grid_shorthand(
    property: &str,
    value: &CssValue,
) -> Result<
    (
        Option<GridTemplate>,
        Option<GridTemplate>,
        Option<Vec<GridTemplateArea>>,
        Option<GridAutoFlow>,
        Option<Vec<GridTrackValue>>,
        Option<Vec<GridTrackValue>>,
    ),
    CssValueError,
> {
    let components = normalized_components(value);

    match components.as_slice() {
        [CssValueToken::Ident(ident)] if ident == "none" => {
            return Ok((None, None, None, None, None, None));
        }
        [] => return Err(invalid_value(property, text_for_value(value))),
        _ => {}
    }

    if !components.iter().any(|component| matches!(component, CssValueToken::Ident(ident) if ident == "auto-flow")) {
        let (rows, columns, areas) = parse_grid_template_shorthand(property, value)?;
        return Ok((rows, columns, areas, None, None, None));
    }

    let slash_positions = slash_positions(&components);
    if slash_positions.len() != 1 {
        return Err(invalid_value(property, text_for_value(value)));
    }

    let slash = slash_positions[0];
    if slash == 0 || slash + 1 >= components.len() {
        return Err(invalid_value(property, text_for_value(value)));
    }

    let left = &components[..slash];
    let right = &components[slash + 1..];

    let left_has_auto_flow = left
        .iter()
        .any(|component| matches!(component, CssValueToken::Ident(ident) if ident == "auto-flow"));
    let right_has_auto_flow = right
        .iter()
        .any(|component| matches!(component, CssValueToken::Ident(ident) if ident == "auto-flow"));

    match (left_has_auto_flow, right_has_auto_flow) {
        (true, false) => {
            let (auto_flow, auto_rows) =
                parse_grid_auto_flow_tracks(property, left, GridAutoFlow::Row)?;
            Ok((
                None,
                Some(parse_grid_tracks(property, &slice_to_value(right))?),
                None,
                Some(auto_flow),
                Some(auto_rows),
                None,
            ))
        }
        (false, true) => {
            let (auto_flow, auto_columns) =
                parse_grid_auto_flow_tracks(property, right, GridAutoFlow::Column)?;
            Ok((
                Some(parse_grid_tracks(property, &slice_to_value(left))?),
                None,
                None,
                Some(auto_flow),
                None,
                Some(auto_columns),
            ))
        }
        _ => Err(invalid_value(property, text_for_value(value))),
    }
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

pub(super) fn parse_grid_area_shorthand(
    property: &str,
    value: &CssValue,
) -> Result<(Line<GridPlacementValue>, Line<GridPlacementValue>), CssValueError> {
    let components = normalized_components(value);
    let slash_positions = slash_positions(&components);

    if slash_positions.len() > 3 {
        return Err(invalid_value(property, text_for_value(value)));
    }

    let mut parts = Vec::with_capacity(slash_positions.len() + 1);
    let mut start = 0;
    for slash in slash_positions {
        if slash == start {
            return Err(invalid_value(property, text_for_value(value)));
        }
        parts.push(&components[start..slash]);
        start = slash + 1;
    }
    if start == components.len() {
        return Err(invalid_value(property, text_for_value(value)));
    }
    parts.push(&components[start..]);

    let row_start = parse_grid_placement(property, &slice_to_value(parts[0]))?;
    let default_from_first = implied_grid_area_placement(property, parts[0])?;

    let (column_start, default_from_second, row_end, column_end) = match parts.as_slice() {
        [_first] => {
            let implied = default_from_first.clone().unwrap_or(GridPlacementValue::Auto);
            (
                implied.clone(),
                default_from_first,
                implied.clone(),
                implied,
            )
        }
        [_, second] => {
            let column_start = parse_grid_placement(property, &slice_to_value(second))?;
            let default_from_second = implied_grid_area_placement(property, second)?;
            (
                column_start,
                default_from_second.clone(),
                default_from_first.unwrap_or(GridPlacementValue::Auto),
                default_from_second.unwrap_or(GridPlacementValue::Auto),
            )
        }
        [_, second, third] => {
            let column_start = parse_grid_placement(property, &slice_to_value(second))?;
            let default_from_second = implied_grid_area_placement(property, second)?;
            (
                column_start,
                default_from_second.clone(),
                parse_grid_placement(property, &slice_to_value(third))?,
                default_from_second.unwrap_or(GridPlacementValue::Auto),
            )
        }
        [_, second, third, fourth] => (
            parse_grid_placement(property, &slice_to_value(second))?,
            None,
            parse_grid_placement(property, &slice_to_value(third))?,
            parse_grid_placement(property, &slice_to_value(fourth))?,
        ),
        _ => return Err(invalid_value(property, text_for_value(value))),
    };

    let _ = default_from_second;

    Ok((
        Line { start: row_start, end: row_end },
        Line { start: column_start, end: column_end },
    ))
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

fn implied_grid_area_placement(
    property: &str,
    components: &[&CssValueToken],
) -> Result<Option<GridPlacementValue>, CssValueError> {
    match components {
        [CssValueToken::Ident(ident)] if ident != "auto" => {
            parse_grid_placement(property, &slice_to_value(components)).map(Some)
        }
        _ => Ok(None),
    }
}

fn parse_grid_template_area_shorthand(
    property: &str,
    components: &[&CssValueToken],
) -> Result<(Option<GridTemplate>, Option<GridTemplate>, Option<Vec<GridTemplateArea>>), CssValueError>
{
    let slash_positions = slash_positions(components);
    if slash_positions.len() > 1 {
        return Err(invalid_value(property, &components_to_text_refs(components)));
    }

    let (row_components, column_components) = match slash_positions.first().copied() {
        Some(slash) => {
            if slash == 0 || slash + 1 >= components.len() {
                return Err(invalid_value(property, &components_to_text_refs(components)));
            }
            (&components[..slash], Some(&components[slash + 1..]))
        }
        None => (components, None),
    };

    let mut index = 0;
    let mut pending_line_names = Vec::new();
    let mut line_names = Vec::new();
    let mut tracks = Vec::new();
    let mut area_rows = Vec::new();

    while index < row_components.len() {
        while let CssValueToken::SimpleBlock(block) = row_components[index] {
            if block.kind != CssSimpleBlockKind::Bracket {
                break;
            }
            pending_line_names.extend(parse_line_name_block(property, block)?);
            index += 1;
            if index >= row_components.len() {
                line_names.push(pending_line_names);
                return Ok((
                    Some(GridTemplate { components: tracks, line_names }),
                    column_components
                        .map(|components| parse_grid_tracks(property, &slice_to_value(components)))
                        .transpose()?,
                    Some(parse_grid_template_areas(property, &grid_area_rows_value(&area_rows))?),
                ));
            }
        }

        let CssValueToken::String(row) = row_components[index] else {
            return Err(invalid_value(property, &components_to_text_refs(components)));
        };
        area_rows.push(row.clone());
        line_names.push(std::mem::take(&mut pending_line_names));
        index += 1;

        let track = if index < row_components.len()
            && !matches!(row_components[index], CssValueToken::SimpleBlock(_) | CssValueToken::String(_))
        {
            let parsed = parse_grid_track(property, &slice_to_value(&row_components[index..index + 1]))?;
            index += 1;
            parsed
        } else {
            GridTrackValue::Auto
        };
        tracks.push(GridTemplateComponent::Single(track));

        while index < row_components.len() {
            let CssValueToken::SimpleBlock(block) = row_components[index] else {
                break;
            };
            if block.kind != CssSimpleBlockKind::Bracket {
                break;
            }
            pending_line_names.extend(parse_line_name_block(property, block)?);
            index += 1;
        }
    }

    if area_rows.is_empty() {
        return Err(invalid_value(property, &components_to_text_refs(components)));
    }

    line_names.push(pending_line_names);

    Ok((
        Some(GridTemplate { components: tracks, line_names }),
        column_components
            .map(|components| parse_grid_tracks(property, &slice_to_value(components)))
            .transpose()?,
        Some(parse_grid_template_areas(property, &grid_area_rows_value(&area_rows))?),
    ))
}

fn grid_area_rows_value(rows: &[String]) -> CssValue {
    let mut components = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            components.push(CssValueToken::Whitespace);
        }
        components.push(CssValueToken::String(row.clone()));
    }

    CssValue { text: rows.join(" "), components }
}

fn slash_positions(components: &[&CssValueToken]) -> Vec<usize> {
    components
        .iter()
        .enumerate()
        .filter_map(|(index, component)| {
            matches!(component, CssValueToken::Delimiter(CssDelimiter::Solidus)).then_some(index)
        })
        .collect()
}

fn components_to_text_refs(components: &[&CssValueToken]) -> String {
    let owned = components.iter().map(|component| (*component).clone()).collect::<Vec<_>>();
    components_to_text(&owned)
}

fn parse_grid_auto_flow_tracks(
    property: &str,
    components: &[&CssValueToken],
    default_axis: GridAutoFlow,
) -> Result<(GridAutoFlow, Vec<GridTrackValue>), CssValueError> {
    let mut flow_components = Vec::new();
    let mut track_components = Vec::new();

    let mut auto_flow_seen = false;
    let mut parsing_flow = true;

    for component in components.iter().copied() {
        match component {
            CssValueToken::Ident(ident) if ident == "auto-flow" => {
                if auto_flow_seen || !parsing_flow {
                    return Err(invalid_value(property, &components_to_text_refs(components)));
                }
                auto_flow_seen = true;
            }
            CssValueToken::Ident(ident)
                if parsing_flow && matches!(ident.as_str(), "row" | "column" | "dense") =>
            {
                flow_components.push(component);
            }
            _ => {
                parsing_flow = false;
                track_components.push(component);
            }
        }
    }

    if !auto_flow_seen || track_components.is_empty() {
        return Err(invalid_value(property, &components_to_text_refs(components)));
    }

    let auto_flow = match (default_axis, flow_components.as_slice()) {
        (GridAutoFlow::Row, []) => GridAutoFlow::Row,
        (GridAutoFlow::Column, []) => GridAutoFlow::Column,
        _ => parse_grid_auto_flow_direct(property, &slice_to_value(&flow_components))?,
    };

    Ok((
        auto_flow,
        parse_grid_auto_tracks(property, &slice_to_value(&track_components))?,
    ))
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
