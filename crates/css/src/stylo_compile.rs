use style::values::generics::grid::{
    GridTemplateComponent as StyloGridTemplateComponent,
    ImplicitGridTracks as StyloImplicitGridTracks, RepeatCount as StyloRepeatCount,
    TrackBreadth as StyloTrackBreadth, TrackList as StyloTrackList,
    TrackListValue as StyloTrackListValue, TrackRepeat as StyloTrackRepeat,
    TrackSize as StyloTrackSize,
};
use style::values::specified::GridLine as StyloGridLine;
use style::values::specified::position::{
    GridAutoFlow as StyloGridAutoFlow, GridTemplateAreas as StyloGridTemplateAreas,
};
use style::values::specified::{
    Integer as StyloInteger, LengthPercentage as StyloLengthPercentage,
};
use style_traits::values::ToCss;

use crate::compile::{CompiledDeclaration, compile_declaration};
use crate::parse_values::{CssValue, ParsedDeclaration};
use crate::parsing::CssParseError;
use crate::tokenizer::parse_value_tokens;

pub(super) fn compile_stylo_declaration(
    declaration: &style::properties::PropertyDeclaration,
) -> Result<Option<CompiledDeclaration>, CssParseError> {
    use style::properties::PropertyDeclaration::*;

    fn from_value(
        property: &str,
        value: &impl ToCss,
    ) -> Result<CompiledDeclaration, CssParseError> {
        let mut text = String::new();
        value
            .to_css(&mut style_traits::CssWriter::new(&mut text))
            .map_err(|_| CssParseError::InvalidSyntax { line: 1, column: 1 })?;
        let parsed = ParsedDeclaration {
            property: property.into(),
            value: CssValue { text: text.clone(), components: parse_value_tokens(&text)? },
        };
        compile_declaration(&parsed).map_err(CssParseError::CssValue)
    }

    match declaration {
        Display(value) => from_value("display", value).map(Some),
        BoxSizing(value) => from_value("box-sizing", value).map(Some),
        AspectRatio(value) => from_value("aspect-ratio", value).map(Some),
        FlexDirection(value) => from_value("flex-direction", value).map(Some),
        FlexWrap(value) => from_value("flex-wrap", value).map(Some),
        FlexGrow(value) => from_value("flex-grow", value).map(Some),
        FlexShrink(value) => from_value("flex-shrink", value).map(Some),
        FlexBasis(value) => from_value("flex-basis", value).map(Some),
        Position(value) => from_value("position", value).map(Some),
        Top(value) => from_value("top", value).map(Some),
        Right(value) => from_value("right", value).map(Some),
        Bottom(value) => from_value("bottom", value).map(Some),
        Left(value) => from_value("left", value).map(Some),
        OverflowX(value) => from_value("overflow-x", value).map(Some),
        OverflowY(value) => from_value("overflow-y", value).map(Some),
        Width(value) => from_value("width", value).map(Some),
        Height(value) => from_value("height", value).map(Some),
        MinWidth(value) => from_value("min-width", value).map(Some),
        MinHeight(value) => from_value("min-height", value).map(Some),
        MaxWidth(value) => from_value("max-width", value).map(Some),
        MaxHeight(value) => from_value("max-height", value).map(Some),
        AlignItems(value) => from_value("align-items", value).map(Some),
        AlignSelf(value) => from_value("align-self", value).map(Some),
        JustifyItems(value) => from_value("justify-items", value).map(Some),
        JustifySelf(value) => from_value("justify-self", value).map(Some),
        AlignContent(value) => from_value("align-content", value).map(Some),
        JustifyContent(value) => from_value("justify-content", value).map(Some),
        RowGap(value) => from_value("row-gap", value).map(Some),
        ColumnGap(value) => from_value("column-gap", value).map(Some),
        GridAutoFlow(value) => compile_grid_auto_flow(value).map(Some),
        GridTemplateAreas(value) => compile_grid_template_areas(value).map(Some),
        GridTemplateRows(value) => {
            compile_grid_template_component("grid-template-rows", value).map(Some)
        }
        GridTemplateColumns(value) => {
            compile_grid_template_component("grid-template-columns", value).map(Some)
        }
        GridAutoRows(value) => compile_grid_auto_tracks("grid-auto-rows", value).map(Some),
        GridAutoColumns(value) => compile_grid_auto_tracks("grid-auto-columns", value).map(Some),
        GridRowStart(value) => compile_grid_line_side("grid-row-start", value).map(Some),
        GridRowEnd(value) => compile_grid_line_side("grid-row-end", value).map(Some),
        GridColumnStart(value) => compile_grid_line_side("grid-column-start", value).map(Some),
        GridColumnEnd(value) => compile_grid_line_side("grid-column-end", value).map(Some),
        PaddingTop(value) => from_value("padding-top", value).map(Some),
        PaddingRight(value) => from_value("padding-right", value).map(Some),
        PaddingBottom(value) => from_value("padding-bottom", value).map(Some),
        PaddingLeft(value) => from_value("padding-left", value).map(Some),
        MarginTop(value) => from_value("margin-top", value).map(Some),
        MarginRight(value) => from_value("margin-right", value).map(Some),
        MarginBottom(value) => from_value("margin-bottom", value).map(Some),
        MarginLeft(value) => from_value("margin-left", value).map(Some),
        BorderTopWidth(value) => from_value("border-top-width", value).map(Some),
        BorderRightWidth(value) => from_value("border-right-width", value).map(Some),
        BorderBottomWidth(value) => from_value("border-bottom-width", value).map(Some),
        BorderLeftWidth(value) => from_value("border-left-width", value).map(Some),
        BoxShadow(value) => from_value("box-shadow", value).map(Some),
        _ => Ok(None),
    }
}

fn compile_grid_auto_flow(value: &StyloGridAutoFlow) -> Result<CompiledDeclaration, CssParseError> {
    let flow = if value.intersects(StyloGridAutoFlow::COLUMN) {
        if value.intersects(StyloGridAutoFlow::DENSE) {
            crate::style::GridAutoFlow::ColumnDense
        } else {
            crate::style::GridAutoFlow::Column
        }
    } else if value.intersects(StyloGridAutoFlow::DENSE) {
        crate::style::GridAutoFlow::RowDense
    } else {
        crate::style::GridAutoFlow::Row
    };
    Ok(CompiledDeclaration::GridAutoFlow(flow))
}

fn compile_grid_line_side(
    property: &str,
    value: &StyloGridLine,
) -> Result<CompiledDeclaration, CssParseError> {
    let placement = if value.is_auto() {
        crate::style::GridPlacementValue::Auto
    } else if value.is_span {
        if value.ident.0.is_empty() {
            crate::style::GridPlacementValue::Span(value.line_num.value().max(1) as u16)
        } else {
            crate::style::GridPlacementValue::NamedSpan(
                value.ident.to_css_string(),
                value.line_num.value().max(1) as u16,
            )
        }
    } else if !value.ident.0.is_empty() {
        crate::style::GridPlacementValue::NamedLine(
            value.ident.to_css_string(),
            if value.line_num.value() == 0 { 1 } else { value.line_num.value() as i16 },
        )
    } else {
        crate::style::GridPlacementValue::Line(value.line_num.value() as i16)
    };

    let line = match property {
        "grid-row-start" | "grid-column-start" => {
            crate::style::Line { start: placement, end: crate::style::GridPlacementValue::Auto }
        }
        "grid-row-end" | "grid-column-end" => {
            crate::style::Line { start: crate::style::GridPlacementValue::Auto, end: placement }
        }
        _ => return Err(CssParseError::InvalidSyntax { line: 1, column: 1 }),
    };

    Ok(match property {
        "grid-row-start" | "grid-row-end" => CompiledDeclaration::GridRow(line),
        "grid-column-start" | "grid-column-end" => CompiledDeclaration::GridColumn(line),
        _ => return Err(CssParseError::InvalidSyntax { line: 1, column: 1 }),
    })
}

fn compile_grid_template_areas(
    value: &StyloGridTemplateAreas,
) -> Result<CompiledDeclaration, CssParseError> {
    let areas = match value {
        StyloGridTemplateAreas::None => Vec::new(),
        StyloGridTemplateAreas::Areas(areas) => areas
            .0
            .areas
            .iter()
            .map(|area| crate::style::GridTemplateArea {
                name: area.name.to_css_string(),
                row_start: u16::try_from(area.rows.start).unwrap_or(u16::MAX),
                row_end: u16::try_from(area.rows.end).unwrap_or(u16::MAX),
                column_start: u16::try_from(area.columns.start).unwrap_or(u16::MAX),
                column_end: u16::try_from(area.columns.end).unwrap_or(u16::MAX),
            })
            .collect(),
    };

    Ok(CompiledDeclaration::GridTemplateAreas(areas))
}

fn compile_grid_template_component(
    property: &str,
    value: &StyloGridTemplateComponent<StyloLengthPercentage, StyloInteger>,
) -> Result<CompiledDeclaration, CssParseError> {
    let template = match value {
        StyloGridTemplateComponent::TrackList(list) => compile_grid_track_list(list)?,
        _ => return Err(CssParseError::InvalidSyntax { line: 1, column: 1 }),
    };

    Ok(match property {
        "grid-template-rows" => CompiledDeclaration::GridTemplateRows(template),
        "grid-template-columns" => CompiledDeclaration::GridTemplateColumns(template),
        _ => return Err(CssParseError::InvalidSyntax { line: 1, column: 1 }),
    })
}

fn compile_grid_auto_tracks(
    property: &str,
    value: &StyloImplicitGridTracks<StyloTrackSize<StyloLengthPercentage>>,
) -> Result<CompiledDeclaration, CssParseError> {
    let tracks = value.0.iter().map(compile_grid_track_size).collect::<Result<Vec<_>, _>>()?;

    Ok(match property {
        "grid-auto-rows" => CompiledDeclaration::GridAutoRows(tracks),
        "grid-auto-columns" => CompiledDeclaration::GridAutoColumns(tracks),
        _ => return Err(CssParseError::InvalidSyntax { line: 1, column: 1 }),
    })
}

fn compile_grid_track_list(
    value: &StyloTrackList<StyloLengthPercentage, StyloInteger>,
) -> Result<crate::style::GridTemplate, CssParseError> {
    let components = value
        .values
        .iter()
        .map(|item| match item {
            StyloTrackListValue::TrackSize(size) => {
                compile_grid_track_size(size).map(crate::style::GridTemplateComponent::Single)
            }
            StyloTrackListValue::TrackRepeat(repeat) => {
                compile_grid_track_repeat(repeat).map(crate::style::GridTemplateComponent::Repeat)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    let line_names = value
        .line_names
        .iter()
        .map(|names| names.iter().map(|name| name.to_css_string()).collect())
        .collect();

    Ok(crate::style::GridTemplate { components, line_names })
}

fn compile_grid_track_repeat(
    value: &StyloTrackRepeat<StyloLengthPercentage, StyloInteger>,
) -> Result<crate::style::GridTrackRepeat, CssParseError> {
    let count = match value.count {
        StyloRepeatCount::Number(number) => {
            crate::style::GridRepetitionCount::Count(number.value().max(0) as u16)
        }
        StyloRepeatCount::AutoFill => crate::style::GridRepetitionCount::AutoFill,
        StyloRepeatCount::AutoFit => crate::style::GridRepetitionCount::AutoFit,
    };
    let line_names = value
        .line_names
        .iter()
        .map(|names| names.iter().map(|name| name.to_css_string()).collect())
        .collect();
    let tracks =
        value.track_sizes.iter().map(compile_grid_track_size).collect::<Result<Vec<_>, _>>()?;

    Ok(crate::style::GridTrackRepeat { count, line_names, tracks })
}

fn compile_grid_track_size(
    value: &StyloTrackSize<StyloLengthPercentage>,
) -> Result<crate::style::GridTrackValue, CssParseError> {
    match value {
        StyloTrackSize::Breadth(breadth) => compile_grid_track_breadth(breadth),
        StyloTrackSize::Minmax(min, max) => Ok(crate::style::GridTrackValue::MinMax(
            compile_grid_track_min_breadth(min)?,
            compile_grid_track_max_breadth(max)?,
        )),
        StyloTrackSize::FitContent(breadth) => match breadth {
            StyloTrackBreadth::Breadth(length) => {
                Ok(crate::style::GridTrackValue::FitContent(compile_length_percentage(length)?))
            }
            _ => Err(CssParseError::InvalidSyntax { line: 1, column: 1 }),
        },
    }
}

fn compile_grid_track_breadth(
    value: &StyloTrackBreadth<StyloLengthPercentage>,
) -> Result<crate::style::GridTrackValue, CssParseError> {
    Ok(match value {
        StyloTrackBreadth::Breadth(length) => {
            crate::style::GridTrackValue::LengthPercentage(compile_length_percentage(length)?)
        }
        StyloTrackBreadth::Fr(fr) => crate::style::GridTrackValue::Fraction(*fr),
        StyloTrackBreadth::Auto => crate::style::GridTrackValue::Auto,
        StyloTrackBreadth::MinContent => crate::style::GridTrackValue::MinContent,
        StyloTrackBreadth::MaxContent => crate::style::GridTrackValue::MaxContent,
    })
}

fn compile_grid_track_min_breadth(
    value: &StyloTrackBreadth<StyloLengthPercentage>,
) -> Result<crate::style::GridTrackMinValue, CssParseError> {
    Ok(match value {
        StyloTrackBreadth::Breadth(length) => {
            crate::style::GridTrackMinValue::LengthPercentage(compile_length_percentage(length)?)
        }
        StyloTrackBreadth::Auto => crate::style::GridTrackMinValue::Auto,
        StyloTrackBreadth::MinContent => crate::style::GridTrackMinValue::MinContent,
        StyloTrackBreadth::MaxContent => crate::style::GridTrackMinValue::MaxContent,
        StyloTrackBreadth::Fr(_) => {
            return Err(CssParseError::InvalidSyntax { line: 1, column: 1 });
        }
    })
}

fn compile_grid_track_max_breadth(
    value: &StyloTrackBreadth<StyloLengthPercentage>,
) -> Result<crate::style::GridTrackMaxValue, CssParseError> {
    Ok(match value {
        StyloTrackBreadth::Breadth(length) => {
            crate::style::GridTrackMaxValue::LengthPercentage(compile_length_percentage(length)?)
        }
        StyloTrackBreadth::Fr(fr) => crate::style::GridTrackMaxValue::Fraction(*fr),
        StyloTrackBreadth::Auto => crate::style::GridTrackMaxValue::Auto,
        StyloTrackBreadth::MinContent => crate::style::GridTrackMaxValue::MinContent,
        StyloTrackBreadth::MaxContent => crate::style::GridTrackMaxValue::MaxContent,
    })
}

fn compile_length_percentage(
    value: &StyloLengthPercentage,
) -> Result<crate::style::LengthPercentage, CssParseError> {
    Ok(match value {
        StyloLengthPercentage::Length(length) => crate::style::LengthPercentage::Px(
            length
                .to_computed_pixel_length_without_context()
                .map_err(|_| CssParseError::InvalidSyntax { line: 1, column: 1 })?,
        ),
        StyloLengthPercentage::Percentage(percent) => {
            crate::style::LengthPercentage::Percent(percent.0 * 100.0)
        }
        StyloLengthPercentage::Calc(_) => {
            return Err(CssParseError::InvalidSyntax { line: 1, column: 1 });
        }
    })
}
