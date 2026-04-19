use taffy::geometry::{Line as TaffyLine, Rect as TaffyRect, Size as TaffySize};
use taffy::prelude::{
    AlignContent as TaffyAlignContent, AlignItems as TaffyAlignItems, BoxSizing as TaffyBoxSizing,
    Dimension as TaffyDimension, Display as TaffyDisplay, FlexDirection as TaffyFlexDirection,
    FlexWrap as TaffyFlexWrap, FromFr, FromLength, FromPercent, GridAutoFlow as TaffyGridAutoFlow,
    GridPlacement as TaffyGridPlacement, GridTemplateComponent as TaffyGridTemplateComponent,
    JustifyContent as TaffyJustifyContent, LengthPercentage as TaffyTrackLengthPercentage,
    MaxTrackSizingFunction as TaffyMaxTrackSizingFunction,
    MinTrackSizingFunction as TaffyMinTrackSizingFunction, Position as TaffyPosition,
    RepetitionCount as TaffyRepetitionCount, TaffyAuto, TaffyFitContent, TaffyMaxContent,
    TaffyMinContent, TrackSizingFunction as TaffyTrackSizingFunction,
};
use taffy::style::{
    GridTemplateArea as TaffyGridTemplateArea,
    GridTemplateRepetition as TaffyGridTemplateRepetition,
    LengthPercentage as TaffyLengthPercentage, LengthPercentageAuto as TaffyLengthPercentageAuto,
    Overflow as TaffyOverflow, Style as TaffyStyle,
};

use crate::style::*;
use tilescript_core::ResolvedLayoutNode;

#[derive(Debug, Clone, PartialEq)]
pub struct NodeComputedStyle {
    pub node: ResolvedLayoutNode,
    pub computed: ComputedStyle,
    pub taffy_style: TaffyStyle,
    pub children: Vec<NodeComputedStyle>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StyledLayoutTree {
    pub root: NodeComputedStyle,
}

pub fn map_computed_style_to_taffy(style: &ComputedStyle) -> TaffyStyle {
    let mut taffy_style = TaffyStyle::default();

    if let Some(display) = style.display {
        taffy_style.display = map_display(display);
    }
    if let Some(box_sizing) = style.box_sizing {
        taffy_style.box_sizing = map_box_sizing(box_sizing);
    }
    if let Some(aspect_ratio) = style.aspect_ratio {
        taffy_style.aspect_ratio = Some(aspect_ratio);
    }
    if let Some(direction) = style.flex_direction {
        taffy_style.flex_direction = map_flex_direction(direction);
    }
    if let Some(wrap) = style.flex_wrap {
        taffy_style.flex_wrap = map_flex_wrap(wrap);
    }
    if let Some(flex_grow) = style.flex_grow {
        taffy_style.flex_grow = flex_grow;
    }
    if let Some(flex_shrink) = style.flex_shrink {
        taffy_style.flex_shrink = flex_shrink;
    }
    if let Some(flex_basis) = style.flex_basis {
        taffy_style.flex_basis = map_size_value(flex_basis);
    }
    if let Some(position) = style.position {
        taffy_style.position = map_position(position);
    }
    if let Some(inset) = style.inset {
        taffy_style.inset = map_box_edges(inset, map_size_value_auto);
    }
    if style.overflow_x.is_some() || style.overflow_y.is_some() {
        taffy_style.overflow = taffy::geometry::Point {
            x: style.overflow_x.map(map_overflow).unwrap_or(TaffyOverflow::Visible),
            y: style.overflow_y.map(map_overflow).unwrap_or(TaffyOverflow::Visible),
        };
    }
    if style.width.is_some() || style.height.is_some() {
        taffy_style.size = TaffySize {
            width: style.width.map(map_size_value).unwrap_or_else(TaffyDimension::auto),
            height: style.height.map(map_size_value).unwrap_or_else(TaffyDimension::auto),
        };
    }
    if style.min_width.is_some() || style.min_height.is_some() {
        taffy_style.min_size = TaffySize {
            width: style.min_width.map(map_size_value).unwrap_or_else(TaffyDimension::auto),
            height: style.min_height.map(map_size_value).unwrap_or_else(TaffyDimension::auto),
        };
    }
    if style.max_width.is_some() || style.max_height.is_some() {
        taffy_style.max_size = TaffySize {
            width: style.max_width.map(map_size_value).unwrap_or_else(TaffyDimension::auto),
            height: style.max_height.map(map_size_value).unwrap_or_else(TaffyDimension::auto),
        };
    }
    if let Some(gap) = style.gap {
        taffy_style.gap = TaffySize {
            width: map_length_percentage(gap.width),
            height: map_length_percentage(gap.height),
        };
    }
    if let Some(align_items) = style.align_items {
        taffy_style.align_items = Some(map_align_items(align_items));
    }
    if let Some(align_self) = style.align_self {
        taffy_style.align_self = map_self_alignment(align_self);
    }
    if let Some(justify_items) = style.justify_items {
        taffy_style.justify_items = Some(map_align_items(justify_items));
    }
    if let Some(justify_self) = style.justify_self {
        taffy_style.justify_self = map_self_alignment(justify_self);
    }
    if let Some(align_content) = style.align_content {
        taffy_style.align_content = Some(map_align_content(align_content));
    }
    if let Some(justify_content) = style.justify_content {
        taffy_style.justify_content = Some(map_justify_content(justify_content));
    }
    if let Some(tracks) = &style.grid_template_rows {
        taffy_style.grid_template_rows =
            tracks.components.iter().map(map_grid_template_component).collect();
        taffy_style.grid_template_row_names = tracks.line_names.clone();
    }
    if let Some(tracks) = &style.grid_template_columns {
        taffy_style.grid_template_columns =
            tracks.components.iter().map(map_grid_template_component).collect();
        taffy_style.grid_template_column_names = tracks.line_names.clone();
    }
    if let Some(tracks) = &style.grid_auto_rows {
        taffy_style.grid_auto_rows =
            tracks.iter().copied().map(map_grid_track_sizing_function).collect();
    }
    if let Some(tracks) = &style.grid_auto_columns {
        taffy_style.grid_auto_columns =
            tracks.iter().copied().map(map_grid_track_sizing_function).collect();
    }
    if let Some(flow) = style.grid_auto_flow {
        taffy_style.grid_auto_flow = map_grid_auto_flow(flow);
    }
    if let Some(areas) = &style.grid_template_areas {
        taffy_style.grid_template_areas = areas.iter().map(map_grid_template_area).collect();
    }
    if let Some(grid_row) = &style.grid_row {
        taffy_style.grid_row = map_grid_line(grid_row.clone());
    }
    if let Some(grid_column) = &style.grid_column {
        taffy_style.grid_column = map_grid_line(grid_column.clone());
    }
    if let Some(padding) = style.padding {
        taffy_style.padding = map_box_edges(padding, map_length_percentage);
    }
    if let Some(margin) = style.margin {
        taffy_style.margin = map_box_edges(margin, map_size_value_auto);
    }

    taffy_style
}

fn map_display(display: Display) -> TaffyDisplay {
    match display {
        Display::Block => TaffyDisplay::Block,
        Display::Flex => TaffyDisplay::Flex,
        Display::Grid => TaffyDisplay::Grid,
        Display::None => TaffyDisplay::None,
    }
}

fn map_box_sizing(box_sizing: BoxSizingValue) -> TaffyBoxSizing {
    match box_sizing {
        BoxSizingValue::BorderBox => TaffyBoxSizing::BorderBox,
        BoxSizingValue::ContentBox => TaffyBoxSizing::ContentBox,
    }
}

fn map_flex_direction(direction: FlexDirectionValue) -> TaffyFlexDirection {
    match direction {
        FlexDirectionValue::Row => TaffyFlexDirection::Row,
        FlexDirectionValue::Column => TaffyFlexDirection::Column,
        FlexDirectionValue::RowReverse => TaffyFlexDirection::RowReverse,
        FlexDirectionValue::ColumnReverse => TaffyFlexDirection::ColumnReverse,
    }
}

fn map_flex_wrap(wrap: FlexWrapValue) -> TaffyFlexWrap {
    match wrap {
        FlexWrapValue::NoWrap => TaffyFlexWrap::NoWrap,
        FlexWrapValue::Wrap => TaffyFlexWrap::Wrap,
        FlexWrapValue::WrapReverse => TaffyFlexWrap::WrapReverse,
    }
}

fn map_position(position: PositionValue) -> TaffyPosition {
    match position {
        PositionValue::Relative => TaffyPosition::Relative,
        PositionValue::Absolute => TaffyPosition::Absolute,
    }
}

fn map_overflow(overflow: OverflowValue) -> TaffyOverflow {
    match overflow {
        OverflowValue::Visible => TaffyOverflow::Visible,
        OverflowValue::Clip => TaffyOverflow::Clip,
        OverflowValue::Hidden => TaffyOverflow::Hidden,
        OverflowValue::Scroll => TaffyOverflow::Scroll,
    }
}

fn map_align_items(value: AlignmentValue) -> TaffyAlignItems {
    match value {
        AlignmentValue::Start => TaffyAlignItems::Start,
        AlignmentValue::End => TaffyAlignItems::End,
        AlignmentValue::FlexStart => TaffyAlignItems::FlexStart,
        AlignmentValue::FlexEnd => TaffyAlignItems::FlexEnd,
        AlignmentValue::Center => TaffyAlignItems::Center,
        AlignmentValue::Baseline => TaffyAlignItems::Baseline,
        AlignmentValue::Stretch => TaffyAlignItems::Stretch,
    }
}

fn map_self_alignment(value: SelfAlignmentValue) -> Option<TaffyAlignItems> {
    match value {
        SelfAlignmentValue::Auto => None,
        SelfAlignmentValue::Start => Some(TaffyAlignItems::Start),
        SelfAlignmentValue::End => Some(TaffyAlignItems::End),
        SelfAlignmentValue::FlexStart => Some(TaffyAlignItems::FlexStart),
        SelfAlignmentValue::FlexEnd => Some(TaffyAlignItems::FlexEnd),
        SelfAlignmentValue::Center => Some(TaffyAlignItems::Center),
        SelfAlignmentValue::Baseline => Some(TaffyAlignItems::Baseline),
        SelfAlignmentValue::Stretch => Some(TaffyAlignItems::Stretch),
    }
}

fn map_align_content(value: ContentAlignmentValue) -> TaffyAlignContent {
    match value {
        ContentAlignmentValue::Start => TaffyAlignContent::Start,
        ContentAlignmentValue::End => TaffyAlignContent::End,
        ContentAlignmentValue::FlexStart => TaffyAlignContent::FlexStart,
        ContentAlignmentValue::FlexEnd => TaffyAlignContent::FlexEnd,
        ContentAlignmentValue::Center => TaffyAlignContent::Center,
        ContentAlignmentValue::Stretch => TaffyAlignContent::Stretch,
        ContentAlignmentValue::SpaceBetween => TaffyAlignContent::SpaceBetween,
        ContentAlignmentValue::SpaceEvenly => TaffyAlignContent::SpaceEvenly,
        ContentAlignmentValue::SpaceAround => TaffyAlignContent::SpaceAround,
    }
}

fn map_justify_content(value: ContentAlignmentValue) -> TaffyJustifyContent {
    match value {
        ContentAlignmentValue::Start => TaffyJustifyContent::Start,
        ContentAlignmentValue::End => TaffyJustifyContent::End,
        ContentAlignmentValue::FlexStart => TaffyJustifyContent::FlexStart,
        ContentAlignmentValue::FlexEnd => TaffyJustifyContent::FlexEnd,
        ContentAlignmentValue::Center => TaffyJustifyContent::Center,
        ContentAlignmentValue::Stretch => TaffyJustifyContent::Stretch,
        ContentAlignmentValue::SpaceBetween => TaffyJustifyContent::SpaceBetween,
        ContentAlignmentValue::SpaceEvenly => TaffyJustifyContent::SpaceEvenly,
        ContentAlignmentValue::SpaceAround => TaffyJustifyContent::SpaceAround,
    }
}

fn map_size_value(value: SizeValue) -> TaffyDimension {
    match value {
        SizeValue::Auto => TaffyDimension::auto(),
        SizeValue::LengthPercentage(value) => match value {
            LengthPercentage::Px(value) => TaffyDimension::length(value),
            LengthPercentage::Percent(value) => TaffyDimension::percent(value / 100.0),
        },
    }
}

fn map_size_value_auto(value: SizeValue) -> TaffyLengthPercentageAuto {
    match value {
        SizeValue::Auto => TaffyLengthPercentageAuto::AUTO,
        SizeValue::LengthPercentage(value) => map_length_percentage_auto(value),
    }
}

fn map_length_percentage(value: LengthPercentage) -> TaffyLengthPercentage {
    match value {
        LengthPercentage::Px(value) => TaffyLengthPercentage::length(value),
        LengthPercentage::Percent(value) => TaffyLengthPercentage::percent(value / 100.0),
    }
}

fn map_length_percentage_auto(value: LengthPercentage) -> TaffyLengthPercentageAuto {
    match value {
        LengthPercentage::Px(value) => TaffyLengthPercentageAuto::length(value),
        LengthPercentage::Percent(value) => TaffyLengthPercentageAuto::percent(value / 100.0),
    }
}

fn map_box_edges<T, U>(edges: BoxEdges<T>, map: fn(T) -> U) -> TaffyRect<U> {
    TaffyRect {
        left: map(edges.left),
        right: map(edges.right),
        top: map(edges.top),
        bottom: map(edges.bottom),
    }
}

fn map_grid_auto_flow(flow: GridAutoFlow) -> TaffyGridAutoFlow {
    match flow {
        GridAutoFlow::Row => TaffyGridAutoFlow::Row,
        GridAutoFlow::Column => TaffyGridAutoFlow::Column,
        GridAutoFlow::RowDense => TaffyGridAutoFlow::RowDense,
        GridAutoFlow::ColumnDense => TaffyGridAutoFlow::ColumnDense,
    }
}

fn map_grid_line(value: Line<GridPlacementValue>) -> TaffyLine<TaffyGridPlacement> {
    TaffyLine { start: map_grid_placement(value.start), end: map_grid_placement(value.end) }
}

fn map_grid_placement(value: GridPlacementValue) -> TaffyGridPlacement {
    match value {
        GridPlacementValue::Auto => TaffyGridPlacement::Auto,
        GridPlacementValue::Line(line) => TaffyGridPlacement::Line(line.into()),
        GridPlacementValue::NamedLine(name, index) => TaffyGridPlacement::NamedLine(name, index),
        GridPlacementValue::Span(span) => TaffyGridPlacement::Span(span),
        GridPlacementValue::NamedSpan(name, span) => TaffyGridPlacement::NamedSpan(name, span),
    }
}

fn map_grid_template_component(
    value: &GridTemplateComponent,
) -> TaffyGridTemplateComponent<String> {
    match value {
        GridTemplateComponent::Single(track) => {
            TaffyGridTemplateComponent::Single(map_grid_track_sizing_function(*track))
        }
        GridTemplateComponent::Repeat(repetition) => {
            TaffyGridTemplateComponent::Repeat(map_grid_track_repeat(repetition))
        }
    }
}

fn map_grid_track_repeat(repetition: &GridTrackRepeat) -> TaffyGridTemplateRepetition<String> {
    TaffyGridTemplateRepetition {
        count: map_grid_repetition_count(repetition.count),
        tracks: repetition.tracks.iter().copied().map(map_grid_track_sizing_function).collect(),
        line_names: repetition.line_names.clone(),
    }
}

fn map_grid_repetition_count(value: GridRepetitionCount) -> TaffyRepetitionCount {
    match value {
        GridRepetitionCount::AutoFill => TaffyRepetitionCount::AutoFill,
        GridRepetitionCount::AutoFit => TaffyRepetitionCount::AutoFit,
        GridRepetitionCount::Count(count) => TaffyRepetitionCount::Count(count),
    }
}

fn map_grid_template_area(area: &GridTemplateArea) -> TaffyGridTemplateArea<String> {
    TaffyGridTemplateArea {
        name: area.name.clone(),
        row_start: area.row_start,
        row_end: area.row_end,
        column_start: area.column_start,
        column_end: area.column_end,
    }
}

fn map_grid_track_sizing_function(value: GridTrackValue) -> TaffyTrackSizingFunction {
    match value {
        GridTrackValue::Auto => TaffyTrackSizingFunction::AUTO,
        GridTrackValue::MinContent => TaffyTrackSizingFunction::MIN_CONTENT,
        GridTrackValue::MaxContent => TaffyTrackSizingFunction::MAX_CONTENT,
        GridTrackValue::LengthPercentage(value) => match value {
            LengthPercentage::Px(value) => TaffyTrackSizingFunction::from_length(value),
            LengthPercentage::Percent(value) => {
                TaffyTrackSizingFunction::from_percent(value / 100.0)
            }
        },
        GridTrackValue::Fraction(value) => TaffyTrackSizingFunction::from_fr(value),
        GridTrackValue::FitContent(value) => {
            TaffyTrackSizingFunction::fit_content(map_track_length_percentage(value))
        }
        GridTrackValue::MinMax(min, max) => TaffyTrackSizingFunction {
            min: map_grid_track_min_value(min),
            max: map_grid_track_max_value(max),
        },
    }
}

fn map_grid_track_min_value(value: GridTrackMinValue) -> TaffyMinTrackSizingFunction {
    match value {
        GridTrackMinValue::Auto => TaffyMinTrackSizingFunction::AUTO,
        GridTrackMinValue::MinContent => TaffyMinTrackSizingFunction::MIN_CONTENT,
        GridTrackMinValue::MaxContent => TaffyMinTrackSizingFunction::MAX_CONTENT,
        GridTrackMinValue::LengthPercentage(value) => match value {
            LengthPercentage::Px(value) => TaffyMinTrackSizingFunction::length(value),
            LengthPercentage::Percent(value) => TaffyMinTrackSizingFunction::percent(value / 100.0),
        },
    }
}

fn map_grid_track_max_value(value: GridTrackMaxValue) -> TaffyMaxTrackSizingFunction {
    match value {
        GridTrackMaxValue::Auto => TaffyMaxTrackSizingFunction::AUTO,
        GridTrackMaxValue::MinContent => TaffyMaxTrackSizingFunction::MIN_CONTENT,
        GridTrackMaxValue::MaxContent => TaffyMaxTrackSizingFunction::MAX_CONTENT,
        GridTrackMaxValue::LengthPercentage(value) => match value {
            LengthPercentage::Px(value) => TaffyMaxTrackSizingFunction::length(value),
            LengthPercentage::Percent(value) => TaffyMaxTrackSizingFunction::percent(value / 100.0),
        },
        GridTrackMaxValue::Fraction(value) => TaffyMaxTrackSizingFunction::fr(value),
        GridTrackMaxValue::FitContent(value) => {
            TaffyMaxTrackSizingFunction::fit_content(map_track_length_percentage(value))
        }
    }
}

fn map_track_length_percentage(value: LengthPercentage) -> TaffyTrackLengthPercentage {
    match value {
        LengthPercentage::Px(value) => TaffyTrackLengthPercentage::from_length(value),
        LengthPercentage::Percent(value) => TaffyTrackLengthPercentage::from_percent(value / 100.0),
    }
}
