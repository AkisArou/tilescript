use thiserror::Error;

use crate::grid::*;
use crate::parse_values::*;
use cssparser::{Parser, ParserInput};

use crate::style::{
    AlignmentValue, AnimationDirectionValue, AnimationFillModeValue, AnimationIterationCountValue,
    AnimationPlayStateValue, AppearanceValue, BorderRadiusValue, BorderStyleValue, BoxEdges,
    BoxShadowValue, BoxSizingValue, ColorValue, ContentAlignmentValue, Display, FlexDirectionValue,
    FlexWrapValue, FontFamilyName, FontFamilyValue, FontWeightValue, GridAutoFlow,
    GridPlacementValue, GridTemplate, GridTemplateArea, GridTrackValue, LengthPercentage, Line,
    LinearStopValue, MotionEasingKeywordValue, MotionEasingValue, MotionPropertyValue,
    MotionTimeValue, OverflowValue, PositionValue, ScaleTransformValue, Size2, SizeValue,
    StepPositionValue, TextAlignValue, TextTransformValue, TransformOperationValue, TransformValue,
    TranslateTransformValue,
};
use style::parser::{Parse as StyloParse, ParserContext};
use style::stylesheets::{CssRuleType, Origin, UrlExtraData};
use style::values::computed::easing::TimingFunction as ComputedTimingFunction;
use style::values::generics::easing::{StepPosition as StyloStepPosition, TimingKeyword};
use style::values::specified::Time as StyloTime;
use style::values::specified::animation::{
    AnimationDirection as StyloAnimationDirection, AnimationDuration as StyloAnimationDuration,
    AnimationFillMode as StyloAnimationFillMode,
    AnimationIterationCount as StyloAnimationIterationCount, AnimationName as StyloAnimationName,
    AnimationPlayState as StyloAnimationPlayState, TransitionProperty as StyloTransitionProperty,
};
use style::values::specified::easing::TimingFunction as StyloTimingFunction;
use style::values::specified::effects::BoxShadow as StyloBoxShadow;
use style_traits::ParsingMode;
use style_traits::values::ToCss;

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
    Appearance(AppearanceValue),
    Background(ColorValue),
    Color(ColorValue),
    Opacity(f32),
    BorderColor(ColorValue),
    BorderColorSide(BoxSide, ColorValue),
    BorderStyle(BoxEdges<BorderStyleValue>),
    BorderStyleSide(BoxSide, BorderStyleValue),
    BorderRadius(BorderRadiusValue),
    BoxShadow(Vec<BoxShadowValue>),
    BackdropFilter(String),
    Transform(TransformValue),
    TextAlign(TextAlignValue),
    TextTransform(TextTransformValue),
    FontFamily(FontFamilyValue),
    FontSize(LengthPercentage),
    FontWeight(FontWeightValue),
    LetterSpacing(f32),
    AnimationName(Vec<String>),
    AnimationDuration(Vec<MotionTimeValue>),
    AnimationTimingFunction(Vec<MotionEasingValue>),
    AnimationDelay(Vec<MotionTimeValue>),
    AnimationIterationCount(Vec<AnimationIterationCountValue>),
    AnimationDirection(Vec<AnimationDirectionValue>),
    AnimationFillMode(Vec<AnimationFillModeValue>),
    AnimationPlayState(Vec<AnimationPlayStateValue>),
    TransitionProperty(Vec<MotionPropertyValue>),
    TransitionDuration(Vec<MotionTimeValue>),
    TransitionTimingFunction(Vec<MotionEasingValue>),
    TransitionDelay(Vec<MotionTimeValue>),
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
    Border(BoxEdges<LengthPercentage>),
    BorderSide(BoxSide, LengthPercentage),
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
            Self::Appearance(_) => Some("appearance"),
            Self::Background(_) => Some("background"),
            Self::Color(_) => Some("color"),
            Self::Opacity(_) => Some("opacity"),
            Self::BorderColor(_) => Some("border-color"),
            Self::BorderColorSide(side, _) => Some(match side {
                BoxSide::Top => "border-top-color",
                BoxSide::Right => "border-right-color",
                BoxSide::Bottom => "border-bottom-color",
                BoxSide::Left => "border-left-color",
            }),
            Self::BorderStyle(_) => Some("border-style"),
            Self::BorderStyleSide(side, _) => Some(match side {
                BoxSide::Top => "border-top-style",
                BoxSide::Right => "border-right-style",
                BoxSide::Bottom => "border-bottom-style",
                BoxSide::Left => "border-left-style",
            }),
            Self::BorderRadius(_) => Some("border-radius"),
            Self::BoxShadow(_) => Some("box-shadow"),
            Self::BackdropFilter(_) => Some("backdrop-filter"),
            Self::Transform(_) => Some("transform"),
            Self::TextAlign(_) => Some("text-align"),
            Self::TextTransform(_) => Some("text-transform"),
            Self::FontFamily(_) => Some("font-family"),
            Self::FontSize(_) => Some("font-size"),
            Self::FontWeight(_) => Some("font-weight"),
            Self::LetterSpacing(_) => Some("letter-spacing"),
            Self::AnimationName(_) => Some("animation-name"),
            Self::AnimationDuration(_) => Some("animation-duration"),
            Self::AnimationTimingFunction(_) => Some("animation-timing-function"),
            Self::AnimationDelay(_) => Some("animation-delay"),
            Self::AnimationIterationCount(_) => Some("animation-iteration-count"),
            Self::AnimationDirection(_) => Some("animation-direction"),
            Self::AnimationFillMode(_) => Some("animation-fill-mode"),
            Self::AnimationPlayState(_) => Some("animation-play-state"),
            Self::TransitionProperty(_) => Some("transition-property"),
            Self::TransitionDuration(_) => Some("transition-duration"),
            Self::TransitionTimingFunction(_) => Some("transition-timing-function"),
            Self::TransitionDelay(_) => Some("transition-delay"),
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
            Self::Border(_) => Some("border-width"),
            Self::BorderSide(side, _) => Some(match side {
                BoxSide::Top => "border-top-width",
                BoxSide::Right => "border-right-width",
                BoxSide::Bottom => "border-bottom-width",
                BoxSide::Left => "border-left-width",
            }),
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
        "appearance" => {
            Ok(CompiledDeclaration::Appearance(parse_appearance_direct(property, value)?))
        }
        "background" | "background-color" => {
            Ok(CompiledDeclaration::Background(parse_color_direct(property, value)?))
        }
        "color" => Ok(CompiledDeclaration::Color(parse_color_direct(property, value)?)),
        "opacity" => Ok(CompiledDeclaration::Opacity(parse_number_direct(property, value)?)),
        "border-color" => {
            Ok(CompiledDeclaration::BorderColor(parse_color_direct(property, value)?))
        }
        "border-style" => {
            Ok(CompiledDeclaration::BorderStyle(parse_box_border_styles_direct(property, value)?))
        }
        "border-top-style" => Ok(CompiledDeclaration::BorderStyleSide(
            BoxSide::Top,
            parse_border_style_direct(property, value)?,
        )),
        "border-right-style" => Ok(CompiledDeclaration::BorderStyleSide(
            BoxSide::Right,
            parse_border_style_direct(property, value)?,
        )),
        "border-bottom-style" => Ok(CompiledDeclaration::BorderStyleSide(
            BoxSide::Bottom,
            parse_border_style_direct(property, value)?,
        )),
        "border-left-style" => Ok(CompiledDeclaration::BorderStyleSide(
            BoxSide::Left,
            parse_border_style_direct(property, value)?,
        )),
        "border-top-color" => Ok(CompiledDeclaration::BorderColorSide(
            BoxSide::Top,
            parse_color_direct(property, value)?,
        )),
        "border-right-color" => Ok(CompiledDeclaration::BorderColorSide(
            BoxSide::Right,
            parse_color_direct(property, value)?,
        )),
        "border-bottom-color" => Ok(CompiledDeclaration::BorderColorSide(
            BoxSide::Bottom,
            parse_color_direct(property, value)?,
        )),
        "border-left-color" => Ok(CompiledDeclaration::BorderColorSide(
            BoxSide::Left,
            parse_color_direct(property, value)?,
        )),
        "border-radius" => {
            Ok(CompiledDeclaration::BorderRadius(parse_border_radius_direct(property, value)?))
        }
        "box-shadow" => {
            Ok(CompiledDeclaration::BoxShadow(parse_box_shadow_direct(property, value)?))
        }
        "backdrop-filter" => {
            Ok(CompiledDeclaration::BackdropFilter(parse_raw_text_direct(property, value)?))
        }
        "transform" => Ok(CompiledDeclaration::Transform(parse_transform_direct(property, value)?)),
        "text-align" => {
            Ok(CompiledDeclaration::TextAlign(parse_text_align_direct(property, value)?))
        }
        "text-transform" => {
            Ok(CompiledDeclaration::TextTransform(parse_text_transform_direct(property, value)?))
        }
        "font-family" => {
            Ok(CompiledDeclaration::FontFamily(parse_font_family_direct(property, value)?))
        }
        "font-size" => Ok(CompiledDeclaration::FontSize(parse_length_percentage_word(
            property,
            value.text.trim(),
        )?)),
        "font-weight" => {
            Ok(CompiledDeclaration::FontWeight(parse_font_weight_direct(property, value)?))
        }
        "letter-spacing" => {
            Ok(CompiledDeclaration::LetterSpacing(parse_letter_spacing_direct(property, value)?))
        }
        "animation" => Ok(CompiledDeclaration::Ignored),
        "animation-name" => {
            Ok(CompiledDeclaration::AnimationName(parse_animation_name_direct(property, value)?))
        }
        "animation-duration" => Ok(CompiledDeclaration::AnimationDuration(
            parse_animation_duration_direct(property, value)?,
        )),
        "animation-timing-function" => Ok(CompiledDeclaration::AnimationTimingFunction(
            parse_timing_function_list_direct(property, value)?,
        )),
        "animation-delay" => {
            Ok(CompiledDeclaration::AnimationDelay(parse_time_list_direct(property, value)?))
        }
        "animation-iteration-count" => Ok(CompiledDeclaration::AnimationIterationCount(
            parse_animation_iteration_count_direct(property, value)?,
        )),
        "animation-direction" => Ok(CompiledDeclaration::AnimationDirection(
            parse_animation_direction_direct(property, value)?,
        )),
        "animation-fill-mode" => Ok(CompiledDeclaration::AnimationFillMode(
            parse_animation_fill_mode_direct(property, value)?,
        )),
        "animation-play-state" => Ok(CompiledDeclaration::AnimationPlayState(
            parse_animation_play_state_direct(property, value)?,
        )),
        "transition" => Ok(CompiledDeclaration::Ignored),
        "transition-property" => Ok(CompiledDeclaration::TransitionProperty(
            parse_transition_property_direct(property, value)?,
        )),
        "transition-duration" => Ok(CompiledDeclaration::TransitionDuration(
            parse_non_negative_time_list_direct(property, value)?,
        )),
        "transition-timing-function" => Ok(CompiledDeclaration::TransitionTimingFunction(
            parse_timing_function_list_direct(property, value)?,
        )),
        "transition-delay" => {
            Ok(CompiledDeclaration::TransitionDelay(parse_time_list_direct(property, value)?))
        }
        "transition-behavior" => Ok(CompiledDeclaration::Ignored),
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
        "border-width" => Ok(CompiledDeclaration::Border(parse_box_edges_direct(property, value)?)),
        "border-top-width" => Ok(CompiledDeclaration::BorderSide(
            BoxSide::Top,
            parse_length_percentage_word(property, value.text.trim())?,
        )),
        "border-right-width" => Ok(CompiledDeclaration::BorderSide(
            BoxSide::Right,
            parse_length_percentage_word(property, value.text.trim())?,
        )),
        "border-bottom-width" => Ok(CompiledDeclaration::BorderSide(
            BoxSide::Bottom,
            parse_length_percentage_word(property, value.text.trim())?,
        )),
        "border-left-width" => Ok(CompiledDeclaration::BorderSide(
            BoxSide::Left,
            parse_length_percentage_word(property, value.text.trim())?,
        )),
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

fn parse_appearance_direct(
    property: &str,
    value: &CssValue,
) -> Result<AppearanceValue, CssValueError> {
    match keyword(property, value)? {
        "auto" => Ok(AppearanceValue::Auto),
        "none" => Ok(AppearanceValue::None),
        _ => Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        }),
    }
}

fn parse_text_align_direct(
    property: &str,
    value: &CssValue,
) -> Result<TextAlignValue, CssValueError> {
    match keyword(property, value)? {
        "left" => Ok(TextAlignValue::Left),
        "right" => Ok(TextAlignValue::Right),
        "center" => Ok(TextAlignValue::Center),
        "start" => Ok(TextAlignValue::Start),
        "end" => Ok(TextAlignValue::End),
        _ => Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        }),
    }
}

fn parse_text_transform_direct(
    property: &str,
    value: &CssValue,
) -> Result<TextTransformValue, CssValueError> {
    match keyword(property, value)? {
        "none" => Ok(TextTransformValue::None),
        "uppercase" => Ok(TextTransformValue::Uppercase),
        "lowercase" => Ok(TextTransformValue::Lowercase),
        "capitalize" => Ok(TextTransformValue::Capitalize),
        _ => Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        }),
    }
}

fn parse_font_weight_direct(
    property: &str,
    value: &CssValue,
) -> Result<FontWeightValue, CssValueError> {
    match keyword(property, value)? {
        "normal" | "400" => Ok(FontWeightValue::Normal),
        "bold" | "700" => Ok(FontWeightValue::Bold),
        _ => Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        }),
    }
}

fn parse_letter_spacing_direct(property: &str, value: &CssValue) -> Result<f32, CssValueError> {
    match keyword(property, value)? {
        "normal" => Ok(0.0),
        text => {
            text.strip_suffix("px").and_then(|number| number.parse::<f32>().ok()).ok_or_else(|| {
                CssValueError::UnsupportedValue {
                    property: property.to_string(),
                    value: value.text.clone(),
                }
            })
        }
    }
}

fn parse_color_direct(property: &str, value: &CssValue) -> Result<ColorValue, CssValueError> {
    let text = value.text.trim();
    if text.is_empty() {
        return Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        });
    }
    if text.eq_ignore_ascii_case("transparent") {
        return Ok(ColorValue { red: 0, green: 0, blue: 0, alpha: 0 });
    }

    if let Some(color) = parse_hex_color(text) {
        return Ok(color);
    }

    if let Some(color) = parse_rgb_function(text) {
        return Ok(color);
    }

    Err(CssValueError::UnsupportedValue {
        property: property.to_string(),
        value: value.text.clone(),
    })
}

fn parse_border_style_direct(
    property: &str,
    value: &CssValue,
) -> Result<BorderStyleValue, CssValueError> {
    match keyword(property, value)? {
        "none" => Ok(BorderStyleValue::None),
        "solid" => Ok(BorderStyleValue::Solid),
        _ => Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        }),
    }
}

fn parse_raw_text_direct(property: &str, value: &CssValue) -> Result<String, CssValueError> {
    let text = value.text.trim();
    if text.is_empty() {
        Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        })
    } else {
        Ok(text.to_string())
    }
}

fn parse_transform_direct(
    property: &str,
    value: &CssValue,
) -> Result<TransformValue, CssValueError> {
    let text = value.text.trim();
    if text.is_empty() {
        return Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        });
    }

    if text.eq_ignore_ascii_case("none") {
        return Ok(TransformValue { operations: Vec::new() });
    }

    let mut operations = Vec::new();
    for component in &value.components {
        let CssValueToken::Function(function) = component else {
            continue;
        };

        let args = non_whitespace_components(&function.value);
        match function.name.to_ascii_lowercase().as_str() {
            "translate" => operations.push(TransformOperationValue::Translate(
                parse_translate_function_args(property, value, &args)?,
            )),
            "translatex" => {
                operations.push(TransformOperationValue::Translate(TranslateTransformValue {
                    x: parse_transform_length_percentage_arg(property, value, args.first())?,
                    y: LengthPercentage::Px(0.0),
                }))
            }
            "translatey" => {
                operations.push(TransformOperationValue::Translate(TranslateTransformValue {
                    x: LengthPercentage::Px(0.0),
                    y: parse_transform_length_percentage_arg(property, value, args.first())?,
                }))
            }
            "scale" => operations.push(TransformOperationValue::Scale(parse_scale_function_args(
                property, value, &args,
            )?)),
            "scalex" => operations.push(TransformOperationValue::Scale(ScaleTransformValue {
                x: parse_transform_number_arg(property, value, args.first())?,
                y: 1.0,
            })),
            "scaley" => operations.push(TransformOperationValue::Scale(ScaleTransformValue {
                x: 1.0,
                y: parse_transform_number_arg(property, value, args.first())?,
            })),
            _ => {
                return Err(CssValueError::UnsupportedValue {
                    property: property.to_string(),
                    value: value.text.clone(),
                });
            }
        }
    }

    if operations.is_empty() {
        return Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        });
    }

    Ok(TransformValue { operations })
}

fn parse_translate_function_args(
    property: &str,
    value: &CssValue,
    args: &[CssValueToken],
) -> Result<TranslateTransformValue, CssValueError> {
    let parts = split_transform_function_args(args);
    if parts.is_empty() || parts.len() > 2 {
        return Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        });
    }

    let x = parse_transform_length_percentage_arg(
        property,
        value,
        parts.first().and_then(|part| part.first()),
    )?;
    let y = match parts.get(1).and_then(|part| part.first()) {
        Some(token) => parse_transform_length_percentage_arg(property, value, Some(token))?,
        None => LengthPercentage::Px(0.0),
    };

    Ok(TranslateTransformValue { x, y })
}

fn parse_scale_function_args(
    property: &str,
    value: &CssValue,
    args: &[CssValueToken],
) -> Result<ScaleTransformValue, CssValueError> {
    let parts = split_transform_function_args(args);
    if parts.is_empty() || parts.len() > 2 {
        return Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        });
    }

    let x =
        parse_transform_number_arg(property, value, parts.first().and_then(|part| part.first()))?;
    let y = match parts.get(1).and_then(|part| part.first()) {
        Some(token) => parse_transform_number_arg(property, value, Some(token))?,
        None => x,
    };

    Ok(ScaleTransformValue { x, y })
}

fn parse_transform_length_percentage_arg(
    property: &str,
    value: &CssValue,
    token: Option<&CssValueToken>,
) -> Result<LengthPercentage, CssValueError> {
    match token {
        Some(CssValueToken::Dimension(dimension)) if dimension.unit.eq_ignore_ascii_case("px") => {
            Ok(LengthPercentage::Px(dimension.value))
        }
        Some(CssValueToken::Percentage(percent)) => Ok(LengthPercentage::Percent(*percent)),
        Some(CssValueToken::Number(number)) if *number == 0.0 => Ok(LengthPercentage::Px(0.0)),
        Some(CssValueToken::Integer(integer)) if *integer == 0 => Ok(LengthPercentage::Px(0.0)),
        _ => Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        }),
    }
}

fn parse_transform_number_arg(
    property: &str,
    value: &CssValue,
    token: Option<&CssValueToken>,
) -> Result<f32, CssValueError> {
    match token {
        Some(CssValueToken::Number(number)) => Ok(*number),
        Some(CssValueToken::Integer(integer)) => Ok(*integer as f32),
        _ => Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        }),
    }
}

fn non_whitespace_components(tokens: &[CssValueToken]) -> Vec<CssValueToken> {
    tokens.iter().filter(|token| !matches!(token, CssValueToken::Whitespace)).cloned().collect()
}

fn split_transform_function_args(tokens: &[CssValueToken]) -> Vec<Vec<CssValueToken>> {
    let mut parts = Vec::new();
    let mut current = Vec::new();

    for token in tokens {
        match token {
            CssValueToken::Delimiter(CssDelimiter::Comma) => {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(token.clone()),
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }

    parts
}

fn parse_transition_property_direct(
    property: &str,
    value: &CssValue,
) -> Result<Vec<MotionPropertyValue>, CssValueError> {
    let text = value.text.trim();
    if text.is_empty() {
        return Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        });
    }

    let url_data = UrlExtraData(url::Url::parse("about:blank").unwrap().into());
    let context = stylo_parser_context(&url_data);
    let mut input = ParserInput::new(text);
    let mut parser = Parser::new(&mut input);
    let parsed = parser
        .parse_comma_separated(|input| StyloTransitionProperty::parse(&context, input))
        .map_err(|_| CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        })?;
    parser.expect_exhausted().map_err(|_| CssValueError::UnsupportedValue {
        property: property.to_string(),
        value: value.text.clone(),
    })?;

    Ok(parsed
        .into_iter()
        .map(|item| {
            if item.is_all() {
                MotionPropertyValue::All
            } else {
                MotionPropertyValue::Named(item.to_css_string())
            }
        })
        .collect())
}

fn parse_animation_name_direct(
    property: &str,
    value: &CssValue,
) -> Result<Vec<String>, CssValueError> {
    let text = value.text.trim();
    if text.is_empty() {
        return Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        });
    }

    let url_data = UrlExtraData(url::Url::parse("about:blank").unwrap().into());
    let context = stylo_parser_context(&url_data);
    let mut input = ParserInput::new(text);
    let mut parser = Parser::new(&mut input);
    let parsed = parser
        .parse_comma_separated(|input| StyloAnimationName::parse(&context, input))
        .map_err(|_| CssValueError::UnsupportedValue {
        property: property.to_string(),
        value: value.text.clone(),
    })?;
    parser.expect_exhausted().map_err(|_| CssValueError::UnsupportedValue {
        property: property.to_string(),
        value: value.text.clone(),
    })?;

    Ok(parsed
        .into_iter()
        .map(|name| name.as_atom().map(|atom| atom.to_string()).unwrap_or_else(|| "none".into()))
        .collect())
}

fn parse_animation_duration_direct(
    property: &str,
    value: &CssValue,
) -> Result<Vec<MotionTimeValue>, CssValueError> {
    let text = value.text.trim();
    if text.is_empty() {
        return Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        });
    }

    let url_data = UrlExtraData(url::Url::parse("about:blank").unwrap().into());
    let context = stylo_parser_context(&url_data);
    let mut input = ParserInput::new(text);
    let mut parser = Parser::new(&mut input);
    let parsed = parser
        .parse_comma_separated(|input| StyloAnimationDuration::parse(&context, input))
        .map_err(|_| CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        })?;
    parser.expect_exhausted().map_err(|_| CssValueError::UnsupportedValue {
        property: property.to_string(),
        value: value.text.clone(),
    })?;

    Ok(parsed
        .into_iter()
        .map(|item| match item {
            StyloAnimationDuration::Auto => MotionTimeValue(0.0),
            StyloAnimationDuration::Time(time) => motion_time_from_stylo(time),
        })
        .collect())
}

fn parse_timing_function_list_direct(
    property: &str,
    value: &CssValue,
) -> Result<Vec<MotionEasingValue>, CssValueError> {
    let text = value.text.trim();
    if text.is_empty() {
        return Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        });
    }

    let url_data = UrlExtraData(url::Url::parse("about:blank").unwrap().into());
    let context = stylo_parser_context(&url_data);
    let mut input = ParserInput::new(text);
    let mut parser = Parser::new(&mut input);
    let parsed = parser
        .parse_comma_separated(|input| StyloTimingFunction::parse(&context, input))
        .map_err(|_| CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        })?;
    parser.expect_exhausted().map_err(|_| CssValueError::UnsupportedValue {
        property: property.to_string(),
        value: value.text.clone(),
    })?;

    Ok(parsed
        .into_iter()
        .map(|item| timing_function_to_scene(&item.to_computed_value_without_context()))
        .collect())
}

fn parse_non_negative_time_list_direct(
    property: &str,
    value: &CssValue,
) -> Result<Vec<MotionTimeValue>, CssValueError> {
    let text = value.text.trim();
    if text.is_empty() {
        return Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        });
    }

    let url_data = UrlExtraData(url::Url::parse("about:blank").unwrap().into());
    let context = stylo_parser_context(&url_data);
    let mut input = ParserInput::new(text);
    let mut parser = Parser::new(&mut input);
    let parsed = parser
        .parse_comma_separated(|input| StyloTime::parse_non_negative(&context, input))
        .map_err(|_| CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        })?;
    parser.expect_exhausted().map_err(|_| CssValueError::UnsupportedValue {
        property: property.to_string(),
        value: value.text.clone(),
    })?;

    Ok(parsed.into_iter().map(motion_time_from_stylo).collect())
}

fn parse_time_list_direct(
    property: &str,
    value: &CssValue,
) -> Result<Vec<MotionTimeValue>, CssValueError> {
    let text = value.text.trim();
    if text.is_empty() {
        return Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        });
    }

    let url_data = UrlExtraData(url::Url::parse("about:blank").unwrap().into());
    let context = stylo_parser_context(&url_data);
    let mut input = ParserInput::new(text);
    let mut parser = Parser::new(&mut input);
    let parsed =
        parser.parse_comma_separated(|input| StyloTime::parse(&context, input)).map_err(|_| {
            CssValueError::UnsupportedValue {
                property: property.to_string(),
                value: value.text.clone(),
            }
        })?;
    parser.expect_exhausted().map_err(|_| CssValueError::UnsupportedValue {
        property: property.to_string(),
        value: value.text.clone(),
    })?;

    Ok(parsed.into_iter().map(motion_time_from_stylo).collect())
}

fn parse_animation_iteration_count_direct(
    property: &str,
    value: &CssValue,
) -> Result<Vec<AnimationIterationCountValue>, CssValueError> {
    let text = value.text.trim();
    if text.is_empty() {
        return Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        });
    }

    let url_data = UrlExtraData(url::Url::parse("about:blank").unwrap().into());
    let context = stylo_parser_context(&url_data);
    let mut input = ParserInput::new(text);
    let mut parser = Parser::new(&mut input);
    let parsed = parser
        .parse_comma_separated(|input| StyloAnimationIterationCount::parse(&context, input))
        .map_err(|_| CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        })?;
    parser.expect_exhausted().map_err(|_| CssValueError::UnsupportedValue {
        property: property.to_string(),
        value: value.text.clone(),
    })?;

    Ok(parsed
        .into_iter()
        .map(|item| match item {
            StyloAnimationIterationCount::Number(number) => {
                AnimationIterationCountValue::Number(number.get())
            }
            StyloAnimationIterationCount::Infinite => AnimationIterationCountValue::Infinite,
        })
        .collect())
}

fn parse_animation_direction_direct(
    property: &str,
    value: &CssValue,
) -> Result<Vec<AnimationDirectionValue>, CssValueError> {
    let text = value.text.trim();
    if text.is_empty() {
        return Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        });
    }

    let mut input = ParserInput::new(text);
    let mut parser = Parser::new(&mut input);
    let parsed = parser.parse_comma_separated(StyloAnimationDirection::parse).map_err(|_| {
        CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        }
    })?;
    parser.expect_exhausted().map_err(|_| CssValueError::UnsupportedValue {
        property: property.to_string(),
        value: value.text.clone(),
    })?;

    Ok(parsed
        .into_iter()
        .map(|item| match item {
            StyloAnimationDirection::Normal => AnimationDirectionValue::Normal,
            StyloAnimationDirection::Reverse => AnimationDirectionValue::Reverse,
            StyloAnimationDirection::Alternate => AnimationDirectionValue::Alternate,
            StyloAnimationDirection::AlternateReverse => AnimationDirectionValue::AlternateReverse,
        })
        .collect())
}

fn parse_animation_fill_mode_direct(
    property: &str,
    value: &CssValue,
) -> Result<Vec<AnimationFillModeValue>, CssValueError> {
    let text = value.text.trim();
    if text.is_empty() {
        return Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        });
    }

    let mut input = ParserInput::new(text);
    let mut parser = Parser::new(&mut input);
    let parsed = parser.parse_comma_separated(StyloAnimationFillMode::parse).map_err(|_| {
        CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        }
    })?;
    parser.expect_exhausted().map_err(|_| CssValueError::UnsupportedValue {
        property: property.to_string(),
        value: value.text.clone(),
    })?;

    Ok(parsed
        .into_iter()
        .map(|item| match item {
            StyloAnimationFillMode::None => AnimationFillModeValue::None,
            StyloAnimationFillMode::Forwards => AnimationFillModeValue::Forwards,
            StyloAnimationFillMode::Backwards => AnimationFillModeValue::Backwards,
            StyloAnimationFillMode::Both => AnimationFillModeValue::Both,
        })
        .collect())
}

fn parse_animation_play_state_direct(
    property: &str,
    value: &CssValue,
) -> Result<Vec<AnimationPlayStateValue>, CssValueError> {
    let text = value.text.trim();
    if text.is_empty() {
        return Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        });
    }

    let mut input = ParserInput::new(text);
    let mut parser = Parser::new(&mut input);
    let parsed = parser.parse_comma_separated(StyloAnimationPlayState::parse).map_err(|_| {
        CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        }
    })?;
    parser.expect_exhausted().map_err(|_| CssValueError::UnsupportedValue {
        property: property.to_string(),
        value: value.text.clone(),
    })?;

    Ok(parsed
        .into_iter()
        .map(|item| match item {
            StyloAnimationPlayState::Running => AnimationPlayStateValue::Running,
            StyloAnimationPlayState::Paused => AnimationPlayStateValue::Paused,
        })
        .collect())
}

fn stylo_parser_context<'a>(url_data: &'a UrlExtraData) -> ParserContext<'a> {
    ParserContext::new(
        Origin::Author,
        url_data,
        Some(CssRuleType::Style),
        ParsingMode::DEFAULT,
        style::context::QuirksMode::NoQuirks,
        Default::default(),
        None,
        None,
    )
}

fn motion_time_from_stylo(value: StyloTime) -> MotionTimeValue {
    MotionTimeValue(value.seconds())
}

fn timing_function_to_scene(value: &ComputedTimingFunction) -> MotionEasingValue {
    match value {
        ComputedTimingFunction::Keyword(keyword) => MotionEasingValue::Keyword(match keyword {
            TimingKeyword::Linear => MotionEasingKeywordValue::Linear,
            TimingKeyword::Ease => MotionEasingKeywordValue::Ease,
            TimingKeyword::EaseIn => MotionEasingKeywordValue::EaseIn,
            TimingKeyword::EaseOut => MotionEasingKeywordValue::EaseOut,
            TimingKeyword::EaseInOut => MotionEasingKeywordValue::EaseInOut,
        }),
        ComputedTimingFunction::CubicBezier { x1, y1, x2, y2 } => {
            MotionEasingValue::CubicBezier { x1: *x1, y1: *y1, x2: *x2, y2: *y2 }
        }
        ComputedTimingFunction::Steps(steps, position) => MotionEasingValue::Steps {
            count: (*steps).max(1) as u16,
            position: step_position_to_scene(*position),
        },
        ComputedTimingFunction::LinearFunction(function) => MotionEasingValue::LinearFunction(
            function
                .iter()
                .map(|entry| LinearStopValue { input: entry.x, output: entry.y })
                .collect(),
        ),
    }
}

fn step_position_to_scene(value: StyloStepPosition) -> StepPositionValue {
    match value {
        StyloStepPosition::JumpStart => StepPositionValue::JumpStart,
        StyloStepPosition::JumpEnd => StepPositionValue::JumpEnd,
        StyloStepPosition::JumpNone => StepPositionValue::JumpNone,
        StyloStepPosition::JumpBoth => StepPositionValue::JumpBoth,
        StyloStepPosition::Start => StepPositionValue::Start,
        StyloStepPosition::End => StepPositionValue::End,
    }
}

fn parse_box_shadow_direct(
    property: &str,
    value: &CssValue,
) -> Result<Vec<BoxShadowValue>, CssValueError> {
    let text = value.text.trim();
    if text.is_empty() {
        return Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        });
    }

    parse_box_shadow_list(text).ok_or_else(|| CssValueError::UnsupportedValue {
        property: property.to_string(),
        value: value.text.clone(),
    })
}

fn parse_font_family_direct(
    property: &str,
    value: &CssValue,
) -> Result<FontFamilyValue, CssValueError> {
    let families = split_font_family_list(value.text.trim())
        .into_iter()
        .filter(|family| !family.is_empty())
        .map(|family| parse_font_family_name(&family))
        .collect::<Vec<_>>();

    if families.is_empty() {
        return Err(CssValueError::UnsupportedValue {
            property: property.to_string(),
            value: value.text.clone(),
        });
    }

    Ok(families)
}

fn parse_font_family_name(value: &str) -> FontFamilyName {
    match value.trim().to_ascii_lowercase().as_str() {
        "serif" => FontFamilyName::Serif,
        "sans-serif" => FontFamilyName::SansSerif,
        "monospace" => FontFamilyName::Monospace,
        "cursive" => FontFamilyName::Cursive,
        "fantasy" => FontFamilyName::Fantasy,
        "system-ui" => FontFamilyName::SystemUi,
        _ => FontFamilyName::Named(value.trim().to_string()),
    }
}

fn parse_border_radius_direct(
    property: &str,
    value: &CssValue,
) -> Result<BorderRadiusValue, CssValueError> {
    let text = value.text.trim();
    let horizontal = text.split('/').next().unwrap_or(text);
    let values =
        horizontal.split_whitespace().map(parse_radius_px).collect::<Option<Vec<_>>>().ok_or_else(
            || CssValueError::UnsupportedValue {
                property: property.to_string(),
                value: value.text.clone(),
            },
        )?;

    let radius = match values.as_slice() {
        [single] => BorderRadiusValue {
            top_left: *single,
            top_right: *single,
            bottom_right: *single,
            bottom_left: *single,
        },
        [top_left, top_right] => BorderRadiusValue {
            top_left: *top_left,
            top_right: *top_right,
            bottom_right: *top_left,
            bottom_left: *top_right,
        },
        [top_left, top_right, bottom_right] => BorderRadiusValue {
            top_left: *top_left,
            top_right: *top_right,
            bottom_right: *bottom_right,
            bottom_left: *top_right,
        },
        [top_left, top_right, bottom_right, bottom_left, ..] => BorderRadiusValue {
            top_left: *top_left,
            top_right: *top_right,
            bottom_right: *bottom_right,
            bottom_left: *bottom_left,
        },
        _ => {
            return Err(CssValueError::UnsupportedValue {
                property: property.to_string(),
                value: value.text.clone(),
            });
        }
    };

    Ok(radius)
}

fn parse_radius_px(token: &str) -> Option<i32> {
    match token.trim() {
        "0" | "0.0" => Some(0),
        value => value
            .strip_suffix("px")
            .and_then(|number| number.parse::<f32>().ok())
            .map(|value| value.round() as i32)
            .map(|value| value.max(0)),
    }
}

fn split_font_family_list(font_family: &str) -> Vec<String> {
    let mut families = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;

    for character in font_family.chars() {
        match character {
            '\'' | '"' => {
                if quote == Some(character) {
                    quote = None;
                } else if quote.is_none() {
                    quote = Some(character);
                }
                current.push(character);
            }
            ',' if quote.is_none() => {
                let family = current.trim();
                if !family.is_empty() {
                    families.push(family.to_string());
                }
                current.clear();
            }
            _ => current.push(character),
        }
    }

    let family = current.trim();
    if !family.is_empty() {
        families.push(family.to_string());
    }

    families
}

fn parse_box_shadow_list(text: &str) -> Option<Vec<BoxShadowValue>> {
    let url_data = UrlExtraData(url::Url::parse("about:blank").ok()?.into());
    let context = stylo_parser_context(&url_data);
    let mut input = ParserInput::new(text);
    let mut parser = Parser::new(&mut input);
    let shadows =
        parser.parse_comma_separated(|input| StyloBoxShadow::parse(&context, input)).ok()?;
    parser.expect_exhausted().ok()?;

    shadows
        .into_iter()
        .map(|shadow| {
            Some(BoxShadowValue {
                color: shadow.base.color.as_ref().and_then(stylo_color_to_scene),
                offset_x: shadow
                    .base
                    .horizontal
                    .to_computed_pixel_length_without_context()
                    .ok()?
                    .round() as i32,
                offset_y: shadow
                    .base
                    .vertical
                    .to_computed_pixel_length_without_context()
                    .ok()?
                    .round() as i32,
                blur_radius: shadow
                    .base
                    .blur
                    .as_ref()
                    .map(|value| value.0.to_computed_pixel_length_without_context())
                    .transpose()
                    .ok()?
                    .unwrap_or(0.0)
                    .round() as i32,
                spread_radius: shadow
                    .spread
                    .as_ref()
                    .map(|value| value.to_computed_pixel_length_without_context())
                    .transpose()
                    .ok()?
                    .unwrap_or(0.0)
                    .round() as i32,
                inset: shadow.inset,
            })
        })
        .collect()
}

fn stylo_color_to_scene(color: &style::values::specified::color::Color) -> Option<ColorValue> {
    let rgba = color.resolve_to_absolute()?.into_srgb_legacy();
    let components = rgba.raw_components();
    Some(ColorValue {
        red: (components[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        green: (components[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        blue: (components[2].clamp(0.0, 1.0) * 255.0).round() as u8,
        alpha: (components[3].clamp(0.0, 1.0) * 255.0).round() as u8,
    })
}

fn parse_hex_color(input: &str) -> Option<ColorValue> {
    let hex = input.strip_prefix('#')?;
    match hex.len() {
        3 => Some(ColorValue {
            red: parse_hex_nibble(hex.as_bytes()[0])? * 17,
            green: parse_hex_nibble(hex.as_bytes()[1])? * 17,
            blue: parse_hex_nibble(hex.as_bytes()[2])? * 17,
            alpha: 255,
        }),
        4 => Some(ColorValue {
            red: parse_hex_nibble(hex.as_bytes()[0])? * 17,
            green: parse_hex_nibble(hex.as_bytes()[1])? * 17,
            blue: parse_hex_nibble(hex.as_bytes()[2])? * 17,
            alpha: parse_hex_nibble(hex.as_bytes()[3])? * 17,
        }),
        6 => Some(ColorValue {
            red: parse_hex_byte(&hex[0..2])?,
            green: parse_hex_byte(&hex[2..4])?,
            blue: parse_hex_byte(&hex[4..6])?,
            alpha: 255,
        }),
        8 => Some(ColorValue {
            red: parse_hex_byte(&hex[0..2])?,
            green: parse_hex_byte(&hex[2..4])?,
            blue: parse_hex_byte(&hex[4..6])?,
            alpha: parse_hex_byte(&hex[6..8])?,
        }),
        _ => None,
    }
}

fn parse_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_hex_byte(input: &str) -> Option<u8> {
    u8::from_str_radix(input, 16).ok()
}

fn parse_rgb_function(input: &str) -> Option<ColorValue> {
    let (name, args) = input.split_once('(')?;
    let args = args.strip_suffix(')')?;
    let parts = args.split(',').map(str::trim).collect::<Vec<_>>();

    match name {
        "rgb" if parts.len() == 3 => Some(ColorValue {
            red: parse_color_channel(parts[0])?,
            green: parse_color_channel(parts[1])?,
            blue: parse_color_channel(parts[2])?,
            alpha: 255,
        }),
        "rgba" if parts.len() == 4 => Some(ColorValue {
            red: parse_color_channel(parts[0])?,
            green: parse_color_channel(parts[1])?,
            blue: parse_color_channel(parts[2])?,
            alpha: parse_alpha_channel(parts[3])?,
        }),
        _ => None,
    }
}

fn parse_color_channel(input: &str) -> Option<u8> {
    input.parse::<u16>().ok().map(|value| value.min(255) as u8)
}

fn parse_alpha_channel(input: &str) -> Option<u8> {
    if let Ok(value) = input.parse::<f32>() {
        return Some((value.clamp(0.0, 1.0) * 255.0).round() as u8);
    }

    input.parse::<u16>().ok().map(|value| value.min(255) as u8)
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

fn parse_box_border_styles_direct(
    property: &str,
    value: &CssValue,
) -> Result<BoxEdges<BorderStyleValue>, CssValueError> {
    let parsed = split_words(value)
        .into_iter()
        .map(|word| {
            parse_border_style_direct(
                property,
                &CssValue { text: word.to_string(), components: Vec::new() },
            )
        })
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
