use thiserror::Error;

use crate::grid::*;
use crate::parse_values::*;

use crate::style::{
    AlignmentValue, BoxEdges, BoxSizingValue, ContentAlignmentValue, Display, FlexDirectionValue,
    FlexWrapValue, GridAutoFlow, GridPlacementValue, GridTemplate, GridTemplateArea,
    GridTrackValue, LengthPercentage, Line, OverflowValue, PositionValue, Size2, SizeValue,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxSide {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Debug, Error, PartialEq)]
pub enum CssValueError {
    #[error("unsupported value `{value}` for property `{property}`")]
    UnsupportedValue { property: String, value: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompiledDeclaration {
    Ignored,
    Display(Display),
    BoxSizing(BoxSizingValue),
    AspectRatio(f32),
    Order(i32),
    FlexDirection(FlexDirectionValue),
    FlexWrap(FlexWrapValue),
    FlexGrow(f32),
    FlexShrink(f32),
    FlexBasis(SizeValue),
    Position(PositionValue),
    Inset(BoxEdges<SizeValue>),
    InsetSide(BoxSide, SizeValue),
    Overflow(OverflowValue, OverflowValue),
    OverflowX(OverflowValue),
    OverflowY(OverflowValue),
    Width(SizeValue),
    Height(SizeValue),
    MinWidth(SizeValue),
    MinHeight(SizeValue),
    MaxWidth(SizeValue),
    MaxHeight(SizeValue),
    AlignItems(AlignmentValue),
    AlignSelf(AlignmentValue),
    JustifyItems(AlignmentValue),
    JustifySelf(AlignmentValue),
    AlignContent(ContentAlignmentValue),
    JustifyContent(ContentAlignmentValue),
    Gap(Size2<LengthPercentage>),
    GridTemplateRows(GridTemplate),
    GridTemplateColumns(GridTemplate),
    GridAutoRows(Vec<GridTrackValue>),
    GridAutoColumns(Vec<GridTrackValue>),
    GridAutoFlow(GridAutoFlow),
    GridTemplateAreas(Vec<GridTemplateArea>),
    GridRow(Line<GridPlacementValue>),
    GridColumn(Line<GridPlacementValue>),
    Padding(BoxEdges<LengthPercentage>),
    PaddingSide(BoxSide, LengthPercentage),
    Margin(BoxEdges<SizeValue>),
    MarginSide(BoxSide, SizeValue),
}

impl CompiledDeclaration {
    pub fn canonical_property_name(&self) -> Option<&'static str> {
        match self {
            Self::Ignored => None,
            Self::Display(_) => Some("display"),
            Self::BoxSizing(_) => Some("box-sizing"),
            Self::AspectRatio(_) => Some("aspect-ratio"),
            Self::Order(_) => Some("order"),
            Self::FlexDirection(_) => Some("flex-direction"),
            Self::FlexWrap(_) => Some("flex-wrap"),
            Self::FlexGrow(_) => Some("flex-grow"),
            Self::FlexShrink(_) => Some("flex-shrink"),
            Self::FlexBasis(_) => Some("flex-basis"),
            Self::Position(_) => Some("position"),
            Self::Inset(_) => Some("inset"),
            Self::InsetSide(side, _) => Some(match side {
                BoxSide::Top => "top",
                BoxSide::Right => "right",
                BoxSide::Bottom => "bottom",
                BoxSide::Left => "left",
            }),
            Self::Overflow(_, _) => Some("overflow"),
            Self::OverflowX(_) => Some("overflow-x"),
            Self::OverflowY(_) => Some("overflow-y"),
            Self::Width(_) => Some("width"),
            Self::Height(_) => Some("height"),
            Self::MinWidth(_) => Some("min-width"),
            Self::MinHeight(_) => Some("min-height"),
            Self::MaxWidth(_) => Some("max-width"),
            Self::MaxHeight(_) => Some("max-height"),
            Self::AlignItems(_) => Some("align-items"),
            Self::AlignSelf(_) => Some("align-self"),
            Self::JustifyItems(_) => Some("justify-items"),
            Self::JustifySelf(_) => Some("justify-self"),
            Self::AlignContent(_) => Some("align-content"),
            Self::JustifyContent(_) => Some("justify-content"),
            Self::Gap(_) => Some("gap"),
            Self::GridTemplateRows(_) => Some("grid-template-rows"),
            Self::GridTemplateColumns(_) => Some("grid-template-columns"),
            Self::GridAutoRows(_) => Some("grid-auto-rows"),
            Self::GridAutoColumns(_) => Some("grid-auto-columns"),
            Self::GridAutoFlow(_) => Some("grid-auto-flow"),
            Self::GridTemplateAreas(_) => Some("grid-template-areas"),
            Self::GridRow(_) => Some("grid-row"),
            Self::GridColumn(_) => Some("grid-column"),
            Self::Padding(_) => Some("padding"),
            Self::PaddingSide(side, _) => Some(match side {
                BoxSide::Top => "padding-top",
                BoxSide::Right => "padding-right",
                BoxSide::Bottom => "padding-bottom",
                BoxSide::Left => "padding-left",
            }),
            Self::Margin(_) => Some("margin"),
            Self::MarginSide(side, _) => Some(match side {
                BoxSide::Top => "margin-top",
                BoxSide::Right => "margin-right",
                BoxSide::Bottom => "margin-bottom",
                BoxSide::Left => "margin-left",
            }),
        }
    }
}

pub fn compile_declaration(
    parsed: &ParsedDeclaration,
) -> Result<CompiledDeclaration, CssValueError> {
    compile_declaration_from_value(&parsed.property, &parsed.value)
}

pub fn compile_declaration_from_value(
    property: &str,
    value: &CssValue,
) -> Result<CompiledDeclaration, CssValueError> {
    match property {
        "display" => Ok(CompiledDeclaration::Display(parse_display_direct(property, value)?)),
        "box-sizing" => {
            Ok(CompiledDeclaration::BoxSizing(parse_box_sizing_direct(property, value)?))
        }
        "aspect-ratio" => {
            Ok(CompiledDeclaration::AspectRatio(parse_aspect_ratio_direct(property, value)?))
        }
        "order" => Ok(CompiledDeclaration::Order(parse_integer_direct(property, value)?)),
        "flex-direction" => {
            Ok(CompiledDeclaration::FlexDirection(parse_flex_direction_direct(property, value)?))
        }
        "flex-wrap" => Ok(CompiledDeclaration::FlexWrap(parse_flex_wrap_direct(property, value)?)),
        "flex-grow" => Ok(CompiledDeclaration::FlexGrow(parse_number_direct(property, value)?)),
        "flex-shrink" => Ok(CompiledDeclaration::FlexShrink(parse_number_direct(property, value)?)),
        "flex-basis" => {
            Ok(CompiledDeclaration::FlexBasis(parse_size_value_direct(property, value)?))
        }
        "position" => Ok(CompiledDeclaration::Position(parse_position_direct(property, value)?)),
        "inset" => Ok(CompiledDeclaration::Inset(parse_box_edges_size_direct(property, value)?)),
        "top" => Ok(CompiledDeclaration::InsetSide(
            BoxSide::Top,
            parse_size_value_direct(property, value)?,
        )),
        "right" => Ok(CompiledDeclaration::InsetSide(
            BoxSide::Right,
            parse_size_value_direct(property, value)?,
        )),
        "bottom" => Ok(CompiledDeclaration::InsetSide(
            BoxSide::Bottom,
            parse_size_value_direct(property, value)?,
        )),
        "left" => Ok(CompiledDeclaration::InsetSide(
            BoxSide::Left,
            parse_size_value_direct(property, value)?,
        )),
        "overflow" => {
            let (x, y) = parse_overflow_pair_direct(property, value)?;
            Ok(CompiledDeclaration::Overflow(x, y))
        }
        "overflow-x" => Ok(CompiledDeclaration::OverflowX(parse_overflow_direct(property, value)?)),
        "overflow-y" => Ok(CompiledDeclaration::OverflowY(parse_overflow_direct(property, value)?)),
        "width" => Ok(CompiledDeclaration::Width(parse_size_value_direct(property, value)?)),
        "height" => Ok(CompiledDeclaration::Height(parse_size_value_direct(property, value)?)),
        "min-width" => Ok(CompiledDeclaration::MinWidth(parse_size_value_direct(property, value)?)),
        "min-height" => {
            Ok(CompiledDeclaration::MinHeight(parse_size_value_direct(property, value)?))
        }
        "max-width" => Ok(CompiledDeclaration::MaxWidth(parse_size_value_direct(property, value)?)),
        "max-height" => {
            Ok(CompiledDeclaration::MaxHeight(parse_size_value_direct(property, value)?))
        }
        "align-items" => {
            Ok(CompiledDeclaration::AlignItems(parse_alignment_direct(property, value)?))
        }
        "align-self" => {
            Ok(CompiledDeclaration::AlignSelf(parse_alignment_direct(property, value)?))
        }
        "justify-items" => {
            Ok(CompiledDeclaration::JustifyItems(parse_alignment_direct(property, value)?))
        }
        "justify-self" => {
            Ok(CompiledDeclaration::JustifySelf(parse_alignment_direct(property, value)?))
        }
        "align-content" => {
            Ok(CompiledDeclaration::AlignContent(parse_content_alignment_direct(property, value)?))
        }
        "justify-content" => Ok(CompiledDeclaration::JustifyContent(
            parse_content_alignment_direct(property, value)?,
        )),
        "gap" => Ok(CompiledDeclaration::Gap(parse_gap_direct(property, value)?)),
        "row-gap" => Ok(CompiledDeclaration::Gap(parse_axis_gap_direct(property, value, true)?)),
        "column-gap" => {
            Ok(CompiledDeclaration::Gap(parse_axis_gap_direct(property, value, false)?))
        }
        "grid-template-rows" => {
            Ok(CompiledDeclaration::GridTemplateRows(parse_grid_tracks(property, value)?))
        }
        "grid-template-columns" => {
            Ok(CompiledDeclaration::GridTemplateColumns(parse_grid_tracks(property, value)?))
        }
        "grid-auto-rows" => {
            Ok(CompiledDeclaration::GridAutoRows(parse_grid_auto_tracks(property, value)?))
        }
        "grid-auto-columns" => {
            Ok(CompiledDeclaration::GridAutoColumns(parse_grid_auto_tracks(property, value)?))
        }
        "grid-template-areas" => {
            Ok(CompiledDeclaration::GridTemplateAreas(parse_grid_template_areas(property, value)?))
        }
        "grid-row" => Ok(CompiledDeclaration::GridRow(parse_grid_line_shorthand(property, value)?)),
        "grid-column" => {
            Ok(CompiledDeclaration::GridColumn(parse_grid_line_shorthand(property, value)?))
        }
        "grid-row-start" | "grid-row-end" => {
            Ok(CompiledDeclaration::GridRow(parse_grid_line_side(property, value)?))
        }
        "grid-column-start" | "grid-column-end" => {
            Ok(CompiledDeclaration::GridColumn(parse_grid_line_side(property, value)?))
        }
        "padding" => Ok(CompiledDeclaration::Padding(parse_box_edges_direct(property, value)?)),
        "padding-top" => Ok(CompiledDeclaration::PaddingSide(
            BoxSide::Top,
            parse_length_percentage_word(property, value.text.trim())?,
        )),
        "padding-right" => Ok(CompiledDeclaration::PaddingSide(
            BoxSide::Right,
            parse_length_percentage_word(property, value.text.trim())?,
        )),
        "padding-bottom" => Ok(CompiledDeclaration::PaddingSide(
            BoxSide::Bottom,
            parse_length_percentage_word(property, value.text.trim())?,
        )),
        "padding-left" => Ok(CompiledDeclaration::PaddingSide(
            BoxSide::Left,
            parse_length_percentage_word(property, value.text.trim())?,
        )),
        "margin" => Ok(CompiledDeclaration::Margin(parse_box_edges_size_direct(property, value)?)),
        "margin-top" => Ok(CompiledDeclaration::MarginSide(
            BoxSide::Top,
            parse_size_value_direct(property, value)?,
        )),
        "margin-right" => Ok(CompiledDeclaration::MarginSide(
            BoxSide::Right,
            parse_size_value_direct(property, value)?,
        )),
        "margin-bottom" => Ok(CompiledDeclaration::MarginSide(
            BoxSide::Bottom,
            parse_size_value_direct(property, value)?,
        )),
        "margin-left" => Ok(CompiledDeclaration::MarginSide(
            BoxSide::Left,
            parse_size_value_direct(property, value)?,
        )),
        _ => Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        }),
    }
}

fn keyword<'a>(property: &str, value: &'a CssValue) -> Result<&'a str, CssValueError> {
    let trimmed = value.text.trim();
    if trimmed.is_empty() || trimmed.split_whitespace().count() != 1 {
        return Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        });
    }
    Ok(trimmed)
}

fn parse_display_direct(property: &str, value: &CssValue) -> Result<Display, CssValueError> {
    match keyword(property, value)? {
        "block" => Ok(Display::Block),
        "flex" => Ok(Display::Flex),
        "grid" => Ok(Display::Grid),
        "none" => Ok(Display::None),
        _ => Err(CssValueError::UnsupportedValue {
            property: property.into(),
            value: value.text.clone(),
        }),
    }
}

fn parse_box_sizing_direct(
    property: &str,
    value: &CssValue,
) -> Result<BoxSizingValue, CssValueError> {
    match keyword(property, value)? {
        "border-box" => Ok(BoxSizingValue::BorderBox),
        "content-box" => Ok(BoxSizingValue::ContentBox),
        _ => Err(CssValueError::UnsupportedValue {
            property: property.into(),
            value: value.text.clone(),
        }),
    }
}

fn parse_aspect_ratio_direct(property: &str, value: &CssValue) -> Result<f32, CssValueError> {
    let trimmed = value.text.trim();
    if let Some((left, right)) = trimmed.split_once('/') {
        let left = left.trim().parse::<f32>().map_err(|_| CssValueError::UnsupportedValue {
            property: property.into(),
            value: value.text.clone(),
        })?;
        let right = right.trim().parse::<f32>().map_err(|_| CssValueError::UnsupportedValue {
            property: property.into(),
            value: value.text.clone(),
        })?;
        if right == 0.0 {
            return Err(CssValueError::UnsupportedValue {
                property: property.into(),
                value: value.text.clone(),
            });
        }
        return Ok(left / right);
    }

    trimmed.parse::<f32>().map_err(|_| CssValueError::UnsupportedValue {
        property: property.into(),
        value: value.text.clone(),
    })
}

fn parse_flex_direction_direct(
    property: &str,
    value: &CssValue,
) -> Result<FlexDirectionValue, CssValueError> {
    match keyword(property, value)? {
        "row" => Ok(FlexDirectionValue::Row),
        "column" => Ok(FlexDirectionValue::Column),
        "row-reverse" => Ok(FlexDirectionValue::RowReverse),
        "column-reverse" => Ok(FlexDirectionValue::ColumnReverse),
        _ => Err(CssValueError::UnsupportedValue {
            property: property.into(),
            value: value.text.clone(),
        }),
    }
}

fn parse_flex_wrap_direct(
    property: &str,
    value: &CssValue,
) -> Result<FlexWrapValue, CssValueError> {
    match keyword(property, value)? {
        "nowrap" => Ok(FlexWrapValue::NoWrap),
        "wrap" => Ok(FlexWrapValue::Wrap),
        "wrap-reverse" => Ok(FlexWrapValue::WrapReverse),
        _ => Err(CssValueError::UnsupportedValue {
            property: property.into(),
            value: value.text.clone(),
        }),
    }
}

fn parse_position_direct(property: &str, value: &CssValue) -> Result<PositionValue, CssValueError> {
    match keyword(property, value)? {
        "relative" => Ok(PositionValue::Relative),
        "absolute" => Ok(PositionValue::Absolute),
        _ => Err(CssValueError::UnsupportedValue {
            property: property.into(),
            value: value.text.clone(),
        }),
    }
}

fn parse_overflow_direct(property: &str, value: &CssValue) -> Result<OverflowValue, CssValueError> {
    match keyword(property, value)? {
        "visible" => Ok(OverflowValue::Visible),
        "clip" => Ok(OverflowValue::Clip),
        "hidden" => Ok(OverflowValue::Hidden),
        "scroll" => Ok(OverflowValue::Scroll),
        _ => Err(CssValueError::UnsupportedValue {
            property: property.into(),
            value: value.text.clone(),
        }),
    }
}

fn parse_overflow_pair_direct(
    property: &str,
    value: &CssValue,
) -> Result<(OverflowValue, OverflowValue), CssValueError> {
    let values = split_words(value)
        .into_iter()
        .map(|word| {
            parse_overflow_direct(property, &CssValue { text: word.into(), components: Vec::new() })
        })
        .collect::<Result<Vec<_>, _>>()?;

    match values.as_slice() {
        [single] => Ok((*single, *single)),
        [x, y] => Ok((*x, *y)),
        _ => Err(CssValueError::UnsupportedValue {
            property: property.into(),
            value: value.text.clone(),
        }),
    }
}

fn parse_alignment_direct(
    property: &str,
    value: &CssValue,
) -> Result<AlignmentValue, CssValueError> {
    match keyword(property, value)? {
        "start" => Ok(AlignmentValue::Start),
        "end" => Ok(AlignmentValue::End),
        "flex-start" => Ok(AlignmentValue::FlexStart),
        "flex-end" => Ok(AlignmentValue::FlexEnd),
        "center" => Ok(AlignmentValue::Center),
        "baseline" => Ok(AlignmentValue::Baseline),
        "stretch" => Ok(AlignmentValue::Stretch),
        _ => Err(CssValueError::UnsupportedValue {
            property: property.into(),
            value: value.text.clone(),
        }),
    }
}

fn parse_content_alignment_direct(
    property: &str,
    value: &CssValue,
) -> Result<ContentAlignmentValue, CssValueError> {
    match keyword(property, value)? {
        "start" => Ok(ContentAlignmentValue::Start),
        "end" => Ok(ContentAlignmentValue::End),
        "flex-start" => Ok(ContentAlignmentValue::FlexStart),
        "flex-end" => Ok(ContentAlignmentValue::FlexEnd),
        "center" => Ok(ContentAlignmentValue::Center),
        "stretch" => Ok(ContentAlignmentValue::Stretch),
        "space-between" => Ok(ContentAlignmentValue::SpaceBetween),
        "space-evenly" => Ok(ContentAlignmentValue::SpaceEvenly),
        "space-around" => Ok(ContentAlignmentValue::SpaceAround),
        _ => Err(CssValueError::UnsupportedValue {
            property: property.into(),
            value: value.text.clone(),
        }),
    }
}

fn parse_gap_direct(
    property: &str,
    value: &CssValue,
) -> Result<Size2<LengthPercentage>, CssValueError> {
    let values = split_words(value)
        .into_iter()
        .map(|word| parse_length_percentage_word(property, word))
        .collect::<Result<Vec<_>, _>>()?;

    match values.as_slice() {
        [single] => Ok(Size2 { width: *single, height: *single }),
        [row, column] => Ok(Size2 { width: *column, height: *row }),
        _ => Err(CssValueError::UnsupportedValue {
            property: property.into(),
            value: value.text.clone(),
        }),
    }
}

fn parse_axis_gap_direct(
    property: &str,
    value: &CssValue,
    is_row: bool,
) -> Result<Size2<LengthPercentage>, CssValueError> {
    let parsed = match split_words(value).as_slice() {
        [single] => parse_length_percentage_word(property, single)?,
        _ => {
            return Err(CssValueError::UnsupportedValue {
                property: property.into(),
                value: value.text.clone(),
            });
        }
    };

    Ok(if is_row {
        Size2 { width: LengthPercentage::Px(0.0), height: parsed }
    } else {
        Size2 { width: parsed, height: LengthPercentage::Px(0.0) }
    })
}

fn parse_number_direct(property: &str, value: &CssValue) -> Result<f32, CssValueError> {
    value.text.trim().parse::<f32>().map_err(|_| CssValueError::UnsupportedValue {
        property: property.into(),
        value: value.text.clone(),
    })
}

fn parse_integer_direct(property: &str, value: &CssValue) -> Result<i32, CssValueError> {
    value.text.trim().parse::<i32>().map_err(|_| CssValueError::UnsupportedValue {
        property: property.into(),
        value: value.text.clone(),
    })
}

fn split_words(value: &CssValue) -> Vec<&str> {
    value.text.split_whitespace().collect()
}

fn parse_length_percentage_word(
    property: &str,
    word: &str,
) -> Result<LengthPercentage, CssValueError> {
    if word == "0" || word == "0.0" {
        return Ok(LengthPercentage::Px(0.0));
    }
    if let Some(number) = word.strip_suffix("px") {
        return number.parse::<f32>().map(LengthPercentage::Px).map_err(|_| {
            CssValueError::UnsupportedValue { property: property.into(), value: word.into() }
        });
    }
    if let Some(number) = word.strip_suffix('%') {
        return number.parse::<f32>().map(LengthPercentage::Percent).map_err(|_| {
            CssValueError::UnsupportedValue { property: property.into(), value: word.into() }
        });
    }
    Err(CssValueError::UnsupportedValue { property: property.into(), value: word.into() })
}

fn parse_size_value_direct(property: &str, value: &CssValue) -> Result<SizeValue, CssValueError> {
    match split_words(value).as_slice() {
        ["auto"] => Ok(SizeValue::Auto),
        [single] => {
            Ok(SizeValue::LengthPercentage(parse_length_percentage_word(property, single)?))
        }
        _ => Err(CssValueError::UnsupportedValue {
            property: property.into(),
            value: value.text.clone(),
        }),
    }
}

fn expand_box_sides<T: Copy>(values: &[T]) -> Option<BoxEdges<T>> {
    match values {
        [a] => Some(BoxEdges { top: *a, right: *a, bottom: *a, left: *a }),
        [vertical, horizontal] => Some(BoxEdges {
            top: *vertical,
            right: *horizontal,
            bottom: *vertical,
            left: *horizontal,
        }),
        [top, horizontal, bottom] => {
            Some(BoxEdges { top: *top, right: *horizontal, bottom: *bottom, left: *horizontal })
        }
        [top, right, bottom, left] => {
            Some(BoxEdges { top: *top, right: *right, bottom: *bottom, left: *left })
        }
        _ => None,
    }
}

fn parse_box_edges_direct(
    property: &str,
    value: &CssValue,
) -> Result<BoxEdges<LengthPercentage>, CssValueError> {
    let parsed = split_words(value)
        .into_iter()
        .map(|word| parse_length_percentage_word(property, word))
        .collect::<Result<Vec<_>, _>>()?;
    expand_box_sides(&parsed).ok_or_else(|| CssValueError::UnsupportedValue {
        property: property.into(),
        value: value.text.clone(),
    })
}

fn parse_box_edges_size_direct(
    property: &str,
    value: &CssValue,
) -> Result<BoxEdges<SizeValue>, CssValueError> {
    let parsed = split_words(value)
        .into_iter()
        .map(|word| {
            if word == "auto" {
                Ok(SizeValue::Auto)
            } else {
                parse_length_percentage_word(property, word).map(SizeValue::LengthPercentage)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    expand_box_sides(&parsed).ok_or_else(|| CssValueError::UnsupportedValue {
        property: property.into(),
        value: value.text.clone(),
    })
}

// ── Value utility helpers (used by grid and other css submodules) ──────────────

pub(super) fn text_for_value(value: &CssValue) -> &str {
    value.text.as_str()
}

pub(super) fn normalized_components(value: &CssValue) -> Vec<&CssValueToken> {
    value
        .components
        .iter()
        .filter(|component| !matches!(component, CssValueToken::Whitespace))
        .collect()
}

pub(super) fn normalized_components_owned(value: &CssValue) -> Vec<CssValueToken> {
    value
        .components
        .iter()
        .filter(|component| !matches!(component, CssValueToken::Whitespace))
        .cloned()
        .collect()
}

pub(super) fn parse_length_percentage(
    property: &str,
    value: &CssValue,
) -> Result<LengthPercentage, CssValueError> {
    let components = normalized_components(value);
    match components.as_slice() {
        [CssValueToken::Integer(0)] => Ok(LengthPercentage::Px(0.0)),
        [CssValueToken::Number(number)] if *number == 0.0 => Ok(LengthPercentage::Px(0.0)),
        [CssValueToken::Dimension(dimension)] => {
            parse_dimension_length_percentage(property, dimension)
        }
        [CssValueToken::Percentage(percent)] => Ok(LengthPercentage::Percent(*percent)),
        _ => Err(invalid_value(property, text_for_value(value))),
    }
}

pub(super) fn parse_dimension_length_percentage(
    property: &str,
    dimension: &CssDimension,
) -> Result<LengthPercentage, CssValueError> {
    if dimension.unit.eq_ignore_ascii_case("px") {
        Ok(LengthPercentage::Px(dimension.value))
    } else {
        Err(invalid_value(property, &format!("{}{}", dimension.value, dimension.unit)))
    }
}

pub(super) fn function_args_value(function: &CssFunction) -> CssValue {
    CssValue { text: components_to_text(&function.value), components: function.value.clone() }
}

pub(super) fn split_function_args(function: &CssFunction) -> Vec<CssValue> {
    let mut groups = Vec::new();
    let mut current = Vec::new();

    for component in &function.value {
        if matches!(component, CssValueToken::Delimiter(CssDelimiter::Comma)) {
            groups.push(CssValue {
                text: components_to_text(&current),
                components: std::mem::take(&mut current),
            });
            continue;
        }
        current.push(component.clone());
    }

    groups.push(CssValue { text: components_to_text(&current), components: current });
    groups
}

pub(super) fn slice_to_value(components: &[&CssValueToken]) -> CssValue {
    let owned = components.iter().map(|component| (*component).clone()).collect::<Vec<_>>();
    CssValue { text: components_to_text(&owned), components: owned }
}

pub(super) fn components_to_text(components: &[CssValueToken]) -> String {
    let mut output = String::new();
    for component in components {
        output.push_str(&component_text(component));
    }
    output.trim().to_owned()
}

pub(super) fn component_text(component: &CssValueToken) -> String {
    match component {
        CssValueToken::Ident(value) => value.clone(),
        CssValueToken::String(value) => format!("\"{value}\""),
        CssValueToken::Number(value) => value.to_string(),
        CssValueToken::Integer(value) => value.to_string(),
        CssValueToken::Dimension(value) => format!("{}{}", value.value, value.unit),
        CssValueToken::Percentage(value) => format!("{value}%"),
        CssValueToken::Function(function) => {
            format!("{}({})", function.name, components_to_text(&function.value))
        }
        CssValueToken::SimpleBlock(block) => {
            let (open, close) = match block.kind {
                CssSimpleBlockKind::Bracket => ('[', ']'),
                CssSimpleBlockKind::Parenthesis => ('(', ')'),
                CssSimpleBlockKind::Brace => ('{', '}'),
            };
            format!("{open}{}{close}", components_to_text(&block.value))
        }
        CssValueToken::Delimiter(CssDelimiter::Comma) => ",".into(),
        CssValueToken::Delimiter(CssDelimiter::Solidus) => "/".into(),
        CssValueToken::Delimiter(CssDelimiter::Semicolon) => ";".into(),
        CssValueToken::Whitespace => " ".into(),
        CssValueToken::Unknown(value) => value.clone(),
    }
}

pub(super) fn invalid_value(property: &str, value: &str) -> CssValueError {
    CssValueError::UnsupportedValue { property: property.to_owned(), value: value.to_owned() }
}
