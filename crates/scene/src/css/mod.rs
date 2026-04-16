pub(crate) mod apply;
mod taffy;

#[cfg(test)]
pub(crate) mod stylo_adapter {
    #[cfg(test)]
    pub(crate) use tilescript_css::parse_selector_list;
}

pub use crate::style::*;
pub use crate::style_calc::compute_style;
pub use tilescript_css::compile;
pub use tilescript_css::compile::CompiledDeclaration;
pub use tilescript_css::compile::CssValueError;
pub use tilescript_css::compiled::*;
pub use tilescript_css::parsing::{CssParseError, parse_stylesheet};
pub use taffy::{NodeComputedStyle, StyledLayoutTree, map_computed_style_to_taffy};

#[cfg(test)]
mod tests {
    use super::stylo_adapter::parse_selector_list;
    use super::*;
    use crate::css::compile::CompiledDeclaration;
    use crate::css_matching::{matching_rules, selector_matches};
    use tilescript_core::WindowId;
    use tilescript_core::{LayoutNodeMeta, ResolvedLayoutNode};
    use tilescript_css::FontFamilyName;

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

        assert_eq!(
            error,
            CssParseError::CssValue(CssValueError::UnsupportedValue {
                property: "color".into(),
                value: "red".into(),
            })
        );
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
    fn supports_window_appearance_values() {
        let auto_sheet = parse_stylesheet("window { appearance: auto; }").unwrap();
        let none_sheet = parse_stylesheet("window { appearance: none; }").unwrap();
        let node = runtime_window_with_meta(LayoutNodeMeta::default());

        assert_eq!(
            compute_style(&auto_sheet, &node).unwrap().appearance,
            Some(AppearanceValue::Auto)
        );
        assert_eq!(
            compute_style(&none_sheet, &node).unwrap().appearance,
            Some(AppearanceValue::None)
        );
    }

    #[test]
    fn rejects_titlebar_pseudo_styles() {
        let error = parse_stylesheet("window::titlebar { text-align: center; }").unwrap_err();

        assert_eq!(
            error,
            CssParseError::UnsupportedSelector { selector: "window::titlebar".into() }
        );
    }

    #[test]
    fn parses_border_style_shorthand() {
        let sheet = parse_stylesheet("window { border-style: solid none; }").unwrap();
        let node = runtime_window_with_meta(LayoutNodeMeta::default());
        let style = compute_style(&sheet, &node).unwrap();

        assert_eq!(
            style.border_style,
            Some(BoxEdges {
                top: BorderStyleValue::Solid,
                right: BorderStyleValue::None,
                bottom: BorderStyleValue::Solid,
                left: BorderStyleValue::None,
            })
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
    fn supports_color_property() {
        let sheet = parse_stylesheet("window { color: rgba(12, 34, 56, 0.5); }").unwrap();
        let node = runtime_window_with_meta(LayoutNodeMeta::default());
        let style = compute_style(&sheet, &node).unwrap();

        assert_eq!(style.color, Some(ColorValue { red: 12, green: 34, blue: 56, alpha: 128 }));
    }

    #[test]
    fn supports_text_align_and_text_transform_properties() {
        let sheet = parse_stylesheet(
            "window { text-align: end; text-transform: capitalize; font-family: serif; font-size: 85%; font-weight: 700; letter-spacing: normal; }",
        )
        .unwrap();
        let node = runtime_window_with_meta(LayoutNodeMeta::default());
        let style = compute_style(&sheet, &node).unwrap();

        assert_eq!(style.text_align, Some(TextAlignValue::End));
        assert_eq!(style.text_transform, Some(TextTransformValue::Capitalize));
        assert_eq!(style.font_family, Some(vec![FontFamilyName::Serif]));
        assert_eq!(style.font_size, Some(LengthPercentage::Percent(85.0)));
        assert_eq!(style.font_weight, Some(FontWeightValue::Bold));
        assert_eq!(style.letter_spacing, Some(0.0));
    }

    #[test]
    fn parses_transform_into_typed_operations() {
        let declaration =
            only_declaration("workspace { transform: translate(100%, 0%) scale(0.8, 1.2); }");

        assert_eq!(
            declaration,
            CompiledDeclaration::Transform(TransformValue {
                operations: vec![
                    TransformOperationValue::Translate(TranslateTransformValue {
                        x: LengthPercentage::Percent(100.0),
                        y: LengthPercentage::Percent(0.0),
                    }),
                    TransformOperationValue::Scale(ScaleTransformValue { x: 0.8, y: 1.2 }),
                ],
            })
        );
    }

    #[test]
    fn parses_transform_none_as_empty_operation_list() {
        let declaration = only_declaration("workspace { transform: none; }");

        assert_eq!(
            declaration,
            CompiledDeclaration::Transform(TransformValue { operations: Vec::new() })
        );
    }

    #[test]
    fn parses_effect_properties_and_keyframes_used_by_root_stylesheet() {
        let sheet = parse_stylesheet(
            r#"
            workspace {
                transition-property: transform, opacity;
                transition-duration: 220ms;
                transition-timing-function: cubic-bezier(0.46, 1, 0.29, 0.99);
            }

            workspace:enter-from-right {
                transform: translate(100%, 0%);
                opacity: 0.98;
            }

            window {
                border-width: 2px;
                border-color: #222222;
                opacity: 0.94;
                border-radius: 14px 10px 18px 8px / 14px 10px 18px 8px;
                box-shadow: 0 12px 28px 4px #00000066;
                backdrop-filter: blur(12px);
                animation: open-zoom 400ms cubic-bezier(0.46, 1, 0.29, 0.99) both;
                transition: opacity 140ms ease-in-out, box-shadow 220ms ease-out;
                appearance: none;
            }

            @keyframes open-zoom {
                from { opacity: 0.15; transform: translate(0px, 24px) scale(0.3); }
                to { opacity: 1; transform: translate(0px, 0px) scale(1); }
            }

            window:closing {
                opacity: 0;
                transition: all 300ms cubic-bezier(0.46, 1.0, 0.29, 0.99);
            }
            "#,
        )
        .unwrap();

        let node = runtime_window_with_meta(LayoutNodeMeta::default());
        let style = compute_style(&sheet, &node).unwrap();

        assert_eq!(style.appearance, Some(AppearanceValue::None));
        assert_eq!(style.opacity, Some(0.94));
        assert_eq!(
            style.border_color,
            Some(ColorValue { red: 34, green: 34, blue: 34, alpha: 255 })
        );
        assert_eq!(
            style.border_radius,
            Some(BorderRadiusValue {
                top_left: 14,
                top_right: 10,
                bottom_right: 18,
                bottom_left: 8,
            })
        );
        assert_eq!(style.animation_name, Some(vec!["open-zoom".into()]));
        assert!(matches!(
            style.transition_property.as_ref(),
            Some(properties) if properties.len() == 2
        ));
        assert!(sheet.rules.iter().any(|rule| {
            rule.declarations.iter().any(|declaration| {
                matches!(
                    declaration,
                    CompiledDeclaration::Transform(TransformValue { operations })
                    if operations.len() == 1
                )
            })
        }));
        assert_eq!(sheet.keyframes.len(), 1);
        assert_eq!(sheet.keyframes[0].name, "open-zoom");
        assert_eq!(sheet.keyframes[0].steps.len(), 2);
        assert!(sheet.keyframes[0].steps.iter().flat_map(|step| step.declarations.iter()).any(
            |declaration| matches!(
                declaration,
                CompiledDeclaration::Transform(TransformValue { operations })
                if operations.len() == 2
            )
        ));
        assert!(style.box_shadow.is_some());
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
