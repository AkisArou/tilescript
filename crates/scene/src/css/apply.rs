use super::compile::{BoxSide, CompiledDeclaration};
use crate::style::*;

pub(crate) trait ApplyCompiledDeclaration {
    fn apply(&mut self, declaration: &CompiledDeclaration);
}

impl ApplyCompiledDeclaration for ComputedStyle {
    fn apply(&mut self, declaration: &CompiledDeclaration) {
        match declaration {
            CompiledDeclaration::Ignored => {}
            CompiledDeclaration::Display(value) => self.display = Some(*value),
            CompiledDeclaration::BoxSizing(value) => self.box_sizing = Some(*value),
            CompiledDeclaration::AspectRatio(value) => self.aspect_ratio = Some(*value),
            CompiledDeclaration::Flex(grow, shrink, basis) => {
                self.flex_grow = Some(*grow);
                self.flex_shrink = Some(*shrink);
                self.flex_basis = Some(*basis);
            }
            CompiledDeclaration::FlexFlow(direction, wrap) => {
                self.flex_direction = Some(*direction);
                self.flex_wrap = Some(*wrap);
            }
            CompiledDeclaration::FlexDirection(value) => self.flex_direction = Some(*value),
            CompiledDeclaration::FlexWrap(value) => self.flex_wrap = Some(*value),
            CompiledDeclaration::FlexGrow(value) => self.flex_grow = Some(*value),
            CompiledDeclaration::FlexShrink(value) => self.flex_shrink = Some(*value),
            CompiledDeclaration::FlexBasis(value) => self.flex_basis = Some(*value),
            CompiledDeclaration::Position(value) => self.position = Some(*value),
            CompiledDeclaration::Inset(value) => self.inset = Some(*value),
            CompiledDeclaration::InsetSide(side, value) => {
                let mut inset = self.inset.unwrap_or(BoxEdges {
                    top: SizeValue::Auto,
                    right: SizeValue::Auto,
                    bottom: SizeValue::Auto,
                    left: SizeValue::Auto,
                });
                match side {
                    BoxSide::Top => inset.top = *value,
                    BoxSide::Right => inset.right = *value,
                    BoxSide::Bottom => inset.bottom = *value,
                    BoxSide::Left => inset.left = *value,
                }
                self.inset = Some(inset);
            }
            CompiledDeclaration::Overflow(x, y) => {
                self.overflow_x = Some(*x);
                self.overflow_y = Some(*y);
            }
            CompiledDeclaration::OverflowX(value) => self.overflow_x = Some(*value),
            CompiledDeclaration::OverflowY(value) => self.overflow_y = Some(*value),
            CompiledDeclaration::Width(value) => self.width = Some(*value),
            CompiledDeclaration::Height(value) => self.height = Some(*value),
            CompiledDeclaration::MinWidth(value) => self.min_width = Some(*value),
            CompiledDeclaration::MinHeight(value) => self.min_height = Some(*value),
            CompiledDeclaration::MaxWidth(value) => self.max_width = Some(*value),
            CompiledDeclaration::MaxHeight(value) => self.max_height = Some(*value),
            CompiledDeclaration::AlignItems(value) => self.align_items = Some(*value),
            CompiledDeclaration::PlaceItems(align, justify) => {
                self.align_items = Some(*align);
                self.justify_items = Some(*justify);
            }
            CompiledDeclaration::AlignSelf(value) => self.align_self = Some(*value),
            CompiledDeclaration::PlaceSelf(align, justify) => {
                self.align_self = Some(*align);
                self.justify_self = Some(*justify);
            }
            CompiledDeclaration::JustifyItems(value) => self.justify_items = Some(*value),
            CompiledDeclaration::JustifySelf(value) => self.justify_self = Some(*value),
            CompiledDeclaration::AlignContent(value) => self.align_content = Some(*value),
            CompiledDeclaration::PlaceContent(align, justify) => {
                self.align_content = Some(*align);
                self.justify_content = Some(*justify);
            }
            CompiledDeclaration::JustifyContent(value) => self.justify_content = Some(*value),
            CompiledDeclaration::Gap(value) => match &mut self.gap {
                Some(existing) => {
                    if !matches!(value.width, LengthPercentage::Px(px) if px == 0.0) {
                        existing.width = value.width;
                    }
                    if !matches!(value.height, LengthPercentage::Px(px) if px == 0.0) {
                        existing.height = value.height;
                    }
                }
                None => self.gap = Some(*value),
            },
            CompiledDeclaration::GridTemplateRows(value) => {
                self.grid_template_rows = Some(value.clone())
            }
            CompiledDeclaration::GridTemplateColumns(value) => {
                self.grid_template_columns = Some(value.clone())
            }
            CompiledDeclaration::GridAutoRows(value) => self.grid_auto_rows = Some(value.clone()),
            CompiledDeclaration::GridAutoColumns(value) => {
                self.grid_auto_columns = Some(value.clone())
            }
            CompiledDeclaration::GridAutoFlow(value) => self.grid_auto_flow = Some(*value),
            CompiledDeclaration::GridTemplateAreas(value) => {
                self.grid_template_areas = Some(value.clone())
            }
            CompiledDeclaration::GridTemplate(rows, columns, areas) => {
                self.grid_template_rows = rows.clone();
                self.grid_template_columns = columns.clone();
                self.grid_template_areas = areas.clone();
            }
            CompiledDeclaration::Grid(rows, columns, areas, auto_flow, auto_rows, auto_columns) => {
                self.grid_template_rows = rows.clone();
                self.grid_template_columns = columns.clone();
                self.grid_template_areas = areas.clone();
                self.grid_auto_flow = *auto_flow;
                self.grid_auto_rows = auto_rows.clone();
                self.grid_auto_columns = auto_columns.clone();
            }
            CompiledDeclaration::GridArea(row, column) => {
                merge_grid_line(&mut self.grid_row, row.clone());
                merge_grid_line(&mut self.grid_column, column.clone());
            }
            CompiledDeclaration::GridRow(value) => {
                merge_grid_line(&mut self.grid_row, value.clone())
            }
            CompiledDeclaration::GridColumn(value) => {
                merge_grid_line(&mut self.grid_column, value.clone())
            }
            CompiledDeclaration::Padding(value) => self.padding = Some(*value),
            CompiledDeclaration::PaddingSide(side, value) => {
                let mut padding = self.padding.unwrap_or(BoxEdges {
                    top: LengthPercentage::Px(0.0),
                    right: LengthPercentage::Px(0.0),
                    bottom: LengthPercentage::Px(0.0),
                    left: LengthPercentage::Px(0.0),
                });
                match side {
                    BoxSide::Top => padding.top = *value,
                    BoxSide::Right => padding.right = *value,
                    BoxSide::Bottom => padding.bottom = *value,
                    BoxSide::Left => padding.left = *value,
                }
                self.padding = Some(padding);
            }
            CompiledDeclaration::Margin(value) => self.margin = Some(*value),
            CompiledDeclaration::MarginSide(side, value) => {
                let mut margin = self.margin.unwrap_or(BoxEdges {
                    top: SizeValue::Auto,
                    right: SizeValue::Auto,
                    bottom: SizeValue::Auto,
                    left: SizeValue::Auto,
                });
                match side {
                    BoxSide::Top => margin.top = *value,
                    BoxSide::Right => margin.right = *value,
                    BoxSide::Bottom => margin.bottom = *value,
                    BoxSide::Left => margin.left = *value,
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
