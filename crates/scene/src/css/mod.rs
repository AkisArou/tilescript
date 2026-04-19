pub(crate) mod apply;
mod taffy;

#[cfg(test)]
pub(crate) mod stylo_adapter {
    #[cfg(test)]
    pub(crate) use tilescript_css::parse_selector_list;
}

pub use crate::style::*;
pub use crate::style_calc::compute_style;
pub use taffy::{NodeComputedStyle, StyledLayoutTree, map_computed_style_to_taffy};
pub use tilescript_css::compile;
pub use tilescript_css::compile::CompiledDeclaration;
pub use tilescript_css::compile::CssValueError;
pub use tilescript_css::compiled::*;
pub use tilescript_css::parsing::{CssParseError, parse_stylesheet};

#[cfg(test)]
mod tests {
    use super::stylo_adapter::parse_selector_list;
    use super::*;
    use crate::css::compile::CompiledDeclaration;
    use crate::css_matching::{matching_rules, selector_matches};
    use tilescript_core::WindowId;
    use tilescript_core::{LayoutNodeMeta, ResolvedLayoutNode};
    fn runtime_window_with_meta(meta: LayoutNodeMeta) -> ResolvedLayoutNode {
        ResolvedLayoutNode::Window {
            meta,
            window_id: Some(WindowId::from("win-1")),
            children: vec![],
        }
    }

    fn only_declaration(source: &str) -> CompiledDeclaration {
        parse_stylesheet(source)
            .unwrap()
            .rules
            .into_iter()
            .next()
            .unwrap()
            .declarations
            .into_iter()
            .next()
            .unwrap()
    }

    #[test]
    fn parses_basic_rule_with_multiple_selectors() {
        let sheet =
            parse_stylesheet("workspace, .stack { display: flex; flex-direction: row; gap: 8px; }")
                .unwrap();

        assert_eq!(sheet.rules.len(), 1);
        assert_eq!(sheet.rules[0].selectors.slice().len(), 2);
        assert_eq!(sheet.rules[0].declarations.len(), 4);
    }

    #[test]
    fn parses_id_selector() {
        let sheet = parse_stylesheet("#main { width: 50%; }").unwrap();

        assert_eq!(sheet.rules[0].selectors.slice().len(), 1);
    }

    #[test]
    fn parses_attribute_selector() {
        let sheet = parse_stylesheet("window[app_id=\"foot\"] { width: 100%; }").unwrap();

        assert_eq!(sheet.rules[0].selectors.slice().len(), 1);
    }

    #[test]
    fn rejects_unsupported_selector() {
        let error = parse_stylesheet("slot { display: flex; }").unwrap_err();

        assert_eq!(error, CssParseError::UnsupportedSelector { selector: "slot".into() });
    }

    #[test]
    fn rejects_unsupported_property() {
        let error = parse_stylesheet("window { color: red; }").unwrap_err();

        assert_eq!(error, CssParseError::UnsupportedProperty { property: "color".into() });
    }

    #[test]
    fn rejects_at_rules_for_v1() {
        let error = parse_stylesheet("@media screen { window { width: 100%; } }").unwrap_err();

        assert_eq!(error, CssParseError::UnsupportedAtRule { name: "media".into() });
    }

    #[test]
    fn matches_type_id_and_class_selectors_against_runtime_nodes() {
        let node = runtime_window_with_meta(LayoutNodeMeta {
            id: Some("main".into()),
            class: vec!["stack".into(), "focused".into()],
            data: [("app_id".into(), "foot".into())].into_iter().collect(),
            ..LayoutNodeMeta::default()
        });

        assert!(selector_matches(&parse_selector_list("window").unwrap(), &node));
        assert!(selector_matches(&parse_selector_list("#main").unwrap(), &node));
        assert!(selector_matches(&parse_selector_list(".stack").unwrap(), &node));
        assert!(selector_matches(&parse_selector_list("[app_id='foot']").unwrap(), &node));
        assert!(!selector_matches(&parse_selector_list("group").unwrap(), &node));
        assert!(!selector_matches(&parse_selector_list(".missing").unwrap(), &node));
    }

    #[test]
    fn matches_window_state_pseudo_selectors_against_runtime_nodes() {
        let node = runtime_window_with_meta(LayoutNodeMeta {
            class: vec!["focused".into(), "floating".into()],
            ..LayoutNodeMeta::default()
        });

        assert!(selector_matches(&parse_selector_list("window:focused").unwrap(), &node));
        assert!(selector_matches(&parse_selector_list("window:floating").unwrap(), &node));
        assert!(!selector_matches(&parse_selector_list("window:fullscreen").unwrap(), &node));
    }

    #[test]
    fn collects_rules_matching_any_selector_in_rule() {
        let sheet = parse_stylesheet(
            "group { gap: 8px; } #main, .stack { width: 50%; } window { height: 100%; }",
        )
        .unwrap();
        let node = runtime_window_with_meta(LayoutNodeMeta {
            id: Some("main".into()),
            class: vec!["stack".into()],
            ..LayoutNodeMeta::default()
        });

        let matches = matching_rules(&sheet, &node);

        assert_eq!(matches.len(), 2);
        assert!(matches!(matches[0].declarations[0], CompiledDeclaration::Width(_)));
        assert!(matches!(matches[1].declarations[0], CompiledDeclaration::Height(_)));
    }

    #[test]
    fn compiles_typed_declaration_values() {
        let sheet = parse_stylesheet("window { padding: 8px 16px; }").unwrap();
        let node = runtime_window_with_meta(LayoutNodeMeta::default());
        let style = compute_style(&sheet, &node).unwrap();

        assert_eq!(
            style.padding,
            Some(BoxEdges {
                top: LengthPercentage::Px(8.0),
                right: LengthPercentage::Px(16.0),
                bottom: LengthPercentage::Px(8.0),
                left: LengthPercentage::Px(16.0),
            })
        );
    }

    #[test]
    fn supports_display_none_aspect_ratio_and_two_axis_gap() {
        let sheet = parse_stylesheet(
            "window { display: none; aspect-ratio: 16 / 9; gap: 10px 20px; box-sizing: content-box; margin: auto 8px; }",
        )
        .unwrap();
        let node = runtime_window_with_meta(LayoutNodeMeta::default());

        let style = compute_style(&sheet, &node).unwrap();

        assert_eq!(style.display, Some(Display::None));
        assert_eq!(style.aspect_ratio, Some(16.0 / 9.0));
        assert_eq!(
            style.gap,
            Some(Size2 { width: LengthPercentage::Px(20.0), height: LengthPercentage::Px(10.0) })
        );
        assert_eq!(style.box_sizing, Some(BoxSizingValue::ContentBox));
        assert_eq!(
            style.margin,
            Some(BoxEdges {
                top: SizeValue::Auto,
                right: SizeValue::LengthPercentage(LengthPercentage::Px(8.0)),
                bottom: SizeValue::Auto,
                left: SizeValue::LengthPercentage(LengthPercentage::Px(8.0)),
            })
        );
    }

    #[test]
    fn rejects_titlebar_pseudo_styles() {
        let error = parse_stylesheet("window::titlebar { display: flex; }").unwrap_err();

        assert_eq!(
            error,
            CssParseError::UnsupportedSelector { selector: "window::titlebar".into() }
        );
    }

    #[test]
    fn rejects_removed_tilescript_resize_properties() {
        let error = parse_stylesheet("#frame { -tilescript-partition-axis: row; }").unwrap_err();

        assert_eq!(
            error,
            CssParseError::UnsupportedProperty { property: "-tilescript-partition-axis".into() }
        );
    }

    #[test]
    fn supports_row_and_column_gap_overrides() {
        let sheet =
            parse_stylesheet("window { gap: 4px; row-gap: 12px; column-gap: 24px; }").unwrap();
        let node = runtime_window_with_meta(LayoutNodeMeta::default());

        let style = compute_style(&sheet, &node).unwrap();

        assert_eq!(
            style.gap,
            Some(Size2 { width: LengthPercentage::Px(24.0), height: LengthPercentage::Px(12.0) })
        );
    }

    #[test]
    fn supports_unitless_zero_for_size_values() {
        let sheet =
            parse_stylesheet("window { flex-basis: 0; min-width: 0; min-height: 0; padding: 0; }")
                .unwrap();
        let node = runtime_window_with_meta(LayoutNodeMeta::default());

        let style = compute_style(&sheet, &node).unwrap();

        assert_eq!(style.flex_basis, Some(SizeValue::LengthPercentage(LengthPercentage::Px(0.0))));
        assert_eq!(style.min_width, Some(SizeValue::LengthPercentage(LengthPercentage::Px(0.0))));
        assert_eq!(style.min_height, Some(SizeValue::LengthPercentage(LengthPercentage::Px(0.0))));
        assert_eq!(
            style.padding,
            Some(BoxEdges {
                top: LengthPercentage::Px(0.0),
                right: LengthPercentage::Px(0.0),
                bottom: LengthPercentage::Px(0.0),
                left: LengthPercentage::Px(0.0),
            })
        );
    }

    #[test]
    fn later_matching_rules_override_earlier_declarations() {
        let sheet = parse_stylesheet(
            "window { width: 40%; gap: 8px; } .stack { width: 60%; } #main { gap: 12px; }",
        )
        .unwrap();
        let node = runtime_window_with_meta(LayoutNodeMeta {
            id: Some("main".into()),
            class: vec!["stack".into()],
            ..LayoutNodeMeta::default()
        });

        let style = compute_style(&sheet, &node).unwrap();

        assert_eq!(style.width, Some(SizeValue::LengthPercentage(LengthPercentage::Percent(60.0))));
        assert_eq!(
            style.gap,
            Some(Size2 { width: LengthPercentage::Px(12.0), height: LengthPercentage::Px(12.0) })
        );
    }

    #[test]
    fn invalid_supported_property_value_fails_during_compilation() {
        let error = parse_stylesheet("window { display: inline; }").unwrap_err();

        assert_eq!(
            error,
            CssParseError::CssValue(CssValueError::UnsupportedValue {
                property: "display".into(),
                value: "inline".into(),
            })
        );
    }

    #[test]
    fn compiles_order_value() {
        let declaration = only_declaration("window { order: -2; }");

        assert_eq!(declaration, CompiledDeclaration::Order(-2));
    }

    #[test]
    fn supports_order_property() {
        let sheet = parse_stylesheet("window { order: 3; }").unwrap();
        let node = runtime_window_with_meta(LayoutNodeMeta::default());
        let style = compute_style(&sheet, &node).unwrap();

        assert_eq!(style.order, Some(3));
    }

    #[test]
    fn compiles_grid_track_and_placement_values() {
        let tracks = only_declaration(
            "window { grid-template-columns: [left] 1fr repeat(2, [mid] 500px) minmax(100px, 2fr) [right]; }",
        );
        let placement = only_declaration("window { grid-column: left / span 2 right; }");

        assert_eq!(
            tracks,
            CompiledDeclaration::GridTemplateColumns(GridTemplate {
                components: vec![
                    GridTemplateComponent::Single(GridTrackValue::Fraction(1.0)),
                    GridTemplateComponent::Repeat(GridTrackRepeat {
                        count: GridRepetitionCount::Count(2),
                        tracks: vec![GridTrackValue::LengthPercentage(LengthPercentage::Px(500.0))],
                        line_names: vec![vec!["mid".into()], vec![]],
                    }),
                    GridTemplateComponent::Single(GridTrackValue::MinMax(
                        GridTrackMinValue::LengthPercentage(LengthPercentage::Px(100.0)),
                        GridTrackMaxValue::Fraction(2.0),
                    )),
                ],
                line_names: vec![vec!["left".into()], vec![], vec![], vec!["right".into()],],
            })
        );
        assert_eq!(
            placement,
            CompiledDeclaration::GridColumn(Line {
                start: GridPlacementValue::NamedLine("left".into(), 1),
                end: GridPlacementValue::NamedSpan("right".into(), 2),
            })
        );
    }

    #[test]
    fn compiles_grid_row_shorthand_value() {
        let placement = only_declaration("window { grid-row: span 3 header / footer; }");

        assert_eq!(
            placement,
            CompiledDeclaration::GridRow(Line {
                start: GridPlacementValue::NamedSpan("header".into(), 3),
                end: GridPlacementValue::NamedLine("footer".into(), 1),
            })
        );
    }

    #[test]
    fn merges_grid_line_side_declarations_into_single_line() {
        let sheet =
            parse_stylesheet("window { grid-column-start: left; grid-column-end: span 2 right; }")
                .unwrap();
        let node = runtime_window_with_meta(LayoutNodeMeta::default());
        let style = compute_style(&sheet, &node).unwrap();

        assert_eq!(
            style.grid_column,
            Some(Line {
                start: GridPlacementValue::NamedLine("left".into(), 1),
                end: GridPlacementValue::NamedSpan("right".into(), 2),
            })
        );
    }

    #[test]
    fn compiles_grid_template_areas() {
        let areas = only_declaration("window { grid-template-areas: \"nav nav\" \"main side\"; }");

        assert_eq!(
            areas,
            CompiledDeclaration::GridTemplateAreas(vec![
                GridTemplateArea {
                    name: "main".into(),
                    row_start: 2,
                    row_end: 3,
                    column_start: 1,
                    column_end: 2,
                },
                GridTemplateArea {
                    name: "nav".into(),
                    row_start: 1,
                    row_end: 2,
                    column_start: 1,
                    column_end: 3,
                },
                GridTemplateArea {
                    name: "side".into(),
                    row_start: 2,
                    row_end: 3,
                    column_start: 2,
                    column_end: 3,
                },
            ])
        );
    }

    #[test]
    fn maps_grid_style_into_taffy_style() {
        let style = ComputedStyle {
            display: Some(Display::Grid),
            grid_template_columns: Some(GridTemplate {
                components: vec![
                    GridTemplateComponent::Single(GridTrackValue::Fraction(1.0)),
                    GridTemplateComponent::Repeat(GridTrackRepeat {
                        count: GridRepetitionCount::Count(2),
                        tracks: vec![GridTrackValue::LengthPercentage(LengthPercentage::Px(500.0))],
                        line_names: vec![vec!["mid".into()], vec![]],
                    }),
                ],
                line_names: vec![vec!["left".into()], vec![], vec![]],
            }),
            grid_template_areas: Some(vec![GridTemplateArea {
                name: "hero".into(),
                row_start: 1,
                row_end: 2,
                column_start: 1,
                column_end: 3,
            }]),
            grid_column: Some(Line {
                start: GridPlacementValue::NamedLine("left".into(), 1),
                end: GridPlacementValue::Auto,
            }),
            ..ComputedStyle::default()
        };

        let mapped = map_computed_style_to_taffy(&style);

        assert_eq!(mapped.display, ::taffy::prelude::Display::Grid);
        assert_eq!(mapped.grid_template_columns.len(), 2);
        assert_eq!(mapped.grid_template_column_names[0][0], "left");
        assert_eq!(mapped.grid_template_areas[0].name, "hero");
        assert_eq!(
            mapped.grid_column.start,
            ::taffy::prelude::GridPlacement::NamedLine("left".into(), 1)
        );
    }

    #[test]
    fn maps_computed_style_into_taffy_style() {
        let style = ComputedStyle {
            display: Some(Display::Flex),
            flex_direction: Some(FlexDirectionValue::Column),
            width: Some(SizeValue::LengthPercentage(LengthPercentage::Percent(60.0))),
            height: Some(SizeValue::LengthPercentage(LengthPercentage::Px(200.0))),
            gap: Some(Size2 {
                width: LengthPercentage::Px(12.0),
                height: LengthPercentage::Px(12.0),
            }),
            padding: Some(BoxEdges {
                top: LengthPercentage::Px(8.0),
                right: LengthPercentage::Px(16.0),
                bottom: LengthPercentage::Px(8.0),
                left: LengthPercentage::Px(16.0),
            }),
            ..ComputedStyle::default()
        };

        let mapped = map_computed_style_to_taffy(&style);

        assert_eq!(mapped.display, ::taffy::prelude::Display::Flex);
        assert_eq!(mapped.flex_direction, ::taffy::prelude::FlexDirection::Column);
        assert_eq!(mapped.size.width, ::taffy::prelude::Dimension::percent(0.6));
        assert_eq!(mapped.size.height, ::taffy::prelude::Dimension::length(200.0));
        assert_eq!(mapped.gap.width, ::taffy::style::LengthPercentage::length(12.0));
        assert_eq!(mapped.padding.left, ::taffy::style::LengthPercentage::length(16.0));
    }
}
