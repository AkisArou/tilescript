use super::compile::{BoxSide, CompiledDeclaration};
use crate::style::*;

pub(crate) trait ApplyCompiledDeclaration {
    fn apply(&mut self, declaration: CompiledDeclaration);
}

impl ApplyCompiledDeclaration for ComputedStyle {
    fn apply(&mut self, declaration: CompiledDeclaration) {
        match declaration {
            CompiledDeclaration::Ignored => {}
            CompiledDeclaration::Display(value) => self.display = Some(value),
            CompiledDeclaration::BoxSizing(value) => self.box_sizing = Some(value),
            CompiledDeclaration::AspectRatio(value) => self.aspect_ratio = Some(value),
            CompiledDeclaration::Appearance(value) => self.appearance = Some(value),
            CompiledDeclaration::Background(value) => self.background = Some(value),
            CompiledDeclaration::Color(value) => self.color = Some(value),
            CompiledDeclaration::Opacity(value) => self.opacity = Some(value),
            CompiledDeclaration::BorderColor(value) => self.border_color = Some(value),
            CompiledDeclaration::BorderColorSide(side, value) => {
                let mut border_side_colors = self.border_side_colors.unwrap_or(BoxEdges {
                    top: None,
                    right: None,
                    bottom: None,
                    left: None,
                });
                match side {
                    BoxSide::Top => border_side_colors.top = Some(value),
                    BoxSide::Right => border_side_colors.right = Some(value),
                    BoxSide::Bottom => border_side_colors.bottom = Some(value),
                    BoxSide::Left => border_side_colors.left = Some(value),
                }
                self.border_side_colors = Some(border_side_colors);
            }
            CompiledDeclaration::BorderStyle(value) => self.border_style = Some(value),
            CompiledDeclaration::BorderRadius(value) => self.border_radius = Some(value),
            CompiledDeclaration::BoxShadow(value) => self.box_shadow = Some(value),
            CompiledDeclaration::BackdropFilter(value) => self.backdrop_filter = Some(value),
            CompiledDeclaration::Transform(value) => self.transform = Some(value),
            CompiledDeclaration::TextAlign(value) => self.text_align = Some(value),
            CompiledDeclaration::TextTransform(value) => self.text_transform = Some(value),
            CompiledDeclaration::FontFamily(value) => self.font_family = Some(value),
            CompiledDeclaration::FontSize(value) => self.font_size = Some(value),
            CompiledDeclaration::FontWeight(value) => self.font_weight = Some(value),
            CompiledDeclaration::LetterSpacing(value) => self.letter_spacing = Some(value),
            CompiledDeclaration::AnimationName(value) => self.animation_name = Some(value),
            CompiledDeclaration::AnimationDuration(value) => self.animation_duration = Some(value),
            CompiledDeclaration::AnimationTimingFunction(value) => {
                self.animation_timing_function = Some(value)
            }
            CompiledDeclaration::AnimationDelay(value) => self.animation_delay = Some(value),
            CompiledDeclaration::AnimationIterationCount(value) => {
                self.animation_iteration_count = Some(value)
            }
            CompiledDeclaration::AnimationDirection(value) => {
                self.animation_direction = Some(value)
            }
            CompiledDeclaration::AnimationFillMode(value) => self.animation_fill_mode = Some(value),
            CompiledDeclaration::AnimationPlayState(value) => {
                self.animation_play_state = Some(value)
            }
            CompiledDeclaration::TransitionProperty(value) => {
                self.transition_property = Some(value)
            }
            CompiledDeclaration::TransitionDuration(value) => {
                self.transition_duration = Some(value)
            }
            CompiledDeclaration::TransitionTimingFunction(value) => {
                self.transition_timing_function = Some(value)
            }
            CompiledDeclaration::TransitionDelay(value) => self.transition_delay = Some(value),
            CompiledDeclaration::FlexDirection(value) => self.flex_direction = Some(value),
            CompiledDeclaration::FlexWrap(value) => self.flex_wrap = Some(value),
            CompiledDeclaration::FlexGrow(value) => self.flex_grow = Some(value),
            CompiledDeclaration::FlexShrink(value) => self.flex_shrink = Some(value),
            CompiledDeclaration::FlexBasis(value) => self.flex_basis = Some(value),
            CompiledDeclaration::Position(value) => self.position = Some(value),
            CompiledDeclaration::Inset(value) => self.inset = Some(value),
            CompiledDeclaration::InsetSide(side, value) => {
                let mut inset = self.inset.unwrap_or(BoxEdges {
                    top: SizeValue::Auto,
                    right: SizeValue::Auto,
                    bottom: SizeValue::Auto,
                    left: SizeValue::Auto,
                });
                match side {
                    BoxSide::Top => inset.top = value,
                    BoxSide::Right => inset.right = value,
                    BoxSide::Bottom => inset.bottom = value,
                    BoxSide::Left => inset.left = value,
                }
                self.inset = Some(inset);
            }
            CompiledDeclaration::Overflow(x, y) => {
                self.overflow_x = Some(x);
                self.overflow_y = Some(y);
            }
            CompiledDeclaration::OverflowX(value) => self.overflow_x = Some(value),
            CompiledDeclaration::OverflowY(value) => self.overflow_y = Some(value),
            CompiledDeclaration::Width(value) => self.width = Some(value),
            CompiledDeclaration::Height(value) => self.height = Some(value),
            CompiledDeclaration::MinWidth(value) => self.min_width = Some(value),
            CompiledDeclaration::MinHeight(value) => self.min_height = Some(value),
            CompiledDeclaration::MaxWidth(value) => self.max_width = Some(value),
            CompiledDeclaration::MaxHeight(value) => self.max_height = Some(value),
            CompiledDeclaration::AlignItems(value) => self.align_items = Some(value),
            CompiledDeclaration::AlignSelf(value) => self.align_self = Some(value),
            CompiledDeclaration::JustifyItems(value) => self.justify_items = Some(value),
            CompiledDeclaration::JustifySelf(value) => self.justify_self = Some(value),
            CompiledDeclaration::AlignContent(value) => self.align_content = Some(value),
            CompiledDeclaration::JustifyContent(value) => self.justify_content = Some(value),
            CompiledDeclaration::Gap(value) => match &mut self.gap {
                Some(existing) => {
                    if !matches!(value.width, LengthPercentage::Px(px) if px == 0.0) {
                        existing.width = value.width;
                    }
                    if !matches!(value.height, LengthPercentage::Px(px) if px == 0.0) {
                        existing.height = value.height;
                    }
                }
                None => self.gap = Some(value),
            },
            CompiledDeclaration::GridTemplateRows(value) => self.grid_template_rows = Some(value),
            CompiledDeclaration::GridTemplateColumns(value) => {
                self.grid_template_columns = Some(value)
            }
            CompiledDeclaration::GridAutoRows(value) => self.grid_auto_rows = Some(value),
            CompiledDeclaration::GridAutoColumns(value) => self.grid_auto_columns = Some(value),
            CompiledDeclaration::GridAutoFlow(value) => self.grid_auto_flow = Some(value),
            CompiledDeclaration::GridTemplateAreas(value) => self.grid_template_areas = Some(value),
            CompiledDeclaration::GridRow(value) => merge_grid_line(&mut self.grid_row, value),
            CompiledDeclaration::GridColumn(value) => merge_grid_line(&mut self.grid_column, value),
            CompiledDeclaration::Border(value) => self.border = Some(value),
            CompiledDeclaration::BorderSide(side, value) => {
                let mut border = self.border.unwrap_or(BoxEdges {
                    top: LengthPercentage::Px(0.0),
                    right: LengthPercentage::Px(0.0),
                    bottom: LengthPercentage::Px(0.0),
                    left: LengthPercentage::Px(0.0),
                });
                match side {
                    BoxSide::Top => border.top = value,
                    BoxSide::Right => border.right = value,
                    BoxSide::Bottom => border.bottom = value,
                    BoxSide::Left => border.left = value,
                }
                self.border = Some(border);
            }
            CompiledDeclaration::BorderStyleSide(side, value) => {
                let mut border_style = self.border_style.unwrap_or(BoxEdges {
                    top: BorderStyleValue::None,
                    right: BorderStyleValue::None,
                    bottom: BorderStyleValue::None,
                    left: BorderStyleValue::None,
                });
                match side {
                    BoxSide::Top => border_style.top = value,
                    BoxSide::Right => border_style.right = value,
                    BoxSide::Bottom => border_style.bottom = value,
                    BoxSide::Left => border_style.left = value,
                }
                self.border_style = Some(border_style);
            }
            CompiledDeclaration::Padding(value) => self.padding = Some(value),
            CompiledDeclaration::PaddingSide(side, value) => {
                let mut padding = self.padding.unwrap_or(BoxEdges {
                    top: LengthPercentage::Px(0.0),
                    right: LengthPercentage::Px(0.0),
                    bottom: LengthPercentage::Px(0.0),
                    left: LengthPercentage::Px(0.0),
                });
                match side {
                    BoxSide::Top => padding.top = value,
                    BoxSide::Right => padding.right = value,
                    BoxSide::Bottom => padding.bottom = value,
                    BoxSide::Left => padding.left = value,
                }
                self.padding = Some(padding);
            }
            CompiledDeclaration::Margin(value) => self.margin = Some(value),
            CompiledDeclaration::MarginSide(side, value) => {
                let mut margin = self.margin.unwrap_or(BoxEdges {
                    top: SizeValue::Auto,
                    right: SizeValue::Auto,
                    bottom: SizeValue::Auto,
                    left: SizeValue::Auto,
                });
                match side {
                    BoxSide::Top => margin.top = value,
                    BoxSide::Right => margin.right = value,
                    BoxSide::Bottom => margin.bottom = value,
                    BoxSide::Left => margin.left = value,
                }
                self.margin = Some(margin);
            }
        }
    }
}

fn merge_grid_line(target: &mut Option<Line<GridPlacementValue>>, value: Line<GridPlacementValue>) {
    match target {
        Some(existing) => {
            if !matches!(value.start, GridPlacementValue::Auto) {
                existing.start = value.start;
            }
            if !matches!(value.end, GridPlacementValue::Auto) {
                existing.end = value.end;
            }
        }
        None => *target = Some(value),
    }
}
