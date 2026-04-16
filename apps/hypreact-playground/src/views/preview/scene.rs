use hypreact_core::LayoutRect;
use hypreact_css::{BorderStyleValue, ColorValue, LengthPercentage, OverflowValue};
use hypreact_scene::ComputedStyle;

pub fn pane_style(
    geometry: LayoutRect,
    accent: &str,
    canvas_width: i32,
    canvas_height: i32,
    animations_enabled: bool,
) -> String {
    let left = geometry.x as f32 / canvas_width as f32 * 100.0;
    let top = geometry.y as f32 / canvas_height as f32 * 100.0;
    let width = geometry.width as f32 / canvas_width as f32 * 100.0;
    let height = geometry.height as f32 / canvas_height as f32 * 100.0;

    let transition_css = if animations_enabled {
        "will-change: left, top, width, height, transform, opacity; transition-property: left, top, width, height, transform, opacity, box-shadow, filter; transition-duration: 220ms, 220ms, 220ms, 220ms, 180ms, 170ms, 170ms, 190ms; transition-timing-function: cubic-bezier(0.23, 1, 0.32, 1), cubic-bezier(0.23, 1, 0.32, 1), cubic-bezier(0.23, 1, 0.32, 1), cubic-bezier(0.23, 1, 0.32, 1), cubic-bezier(0.23, 1, 0.32, 1), cubic-bezier(0.5, 0.5, 0.75, 1), cubic-bezier(0.15, 0, 0.1, 1), cubic-bezier(0.5, 0.5, 0.75, 1);"
    } else {
        "will-change: auto; transition: none;"
    };

    format!(
        "left: {left:.3}%; top: {top:.3}%; width: {width:.3}%; height: {height:.3}%; --accent: {}; {transition_css}",
        accent,
    )
}

pub fn body_style(layout_style: Option<&ComputedStyle>) -> String {
    let overflow_css = match layout_style.and_then(|style| style.overflow_y) {
        Some(OverflowValue::Hidden | OverflowValue::Clip) => "overflow: hidden;".to_string(),
        Some(OverflowValue::Scroll) => "overflow: auto;".to_string(),
        _ => String::new(),
    };

    let padding_css = layout_style
        .and_then(|style| style.padding)
        .map(|padding| {
            format!(
                "padding: {}px {}px {}px {}px;",
                length_to_px(padding.top),
                length_to_px(padding.right),
                length_to_px(padding.bottom),
                length_to_px(padding.left)
            )
        })
        .unwrap_or_default();

    format!("position: relative; width: 100%; height: 100%; {padding_css} {overflow_css}")
}

pub fn frame_style(layout_style: Option<&ComputedStyle>, focused: bool) -> String {
    let background =
        layout_style.and_then(|style| style.background).map(css_color).unwrap_or_else(|| {
            if focused {
                "rgba(20, 24, 33, 1.0)".to_string()
            } else {
                "rgba(24, 27, 36, 1.0)".to_string()
            }
        });
    let border_color = if focused {
        "linear-gradient(180deg, rgba(51, 204, 255, 0.93), rgba(0, 255, 153, 0.93))".to_string()
    } else {
        layout_style
            .and_then(|style| style.border_color)
            .or_else(|| {
                layout_style
                    .and_then(|style| style.border_side_colors)
                    .and_then(|colors| colors.top)
            })
            .map(css_color)
            .unwrap_or_else(|| "rgba(89, 89, 89, 0.67)".to_string())
    };
    let border_width = layout_style
        .and_then(|style| style.border)
        .map(|edges| {
            length_to_px(edges.top)
                .max(length_to_px(edges.right))
                .max(length_to_px(edges.bottom))
                .max(length_to_px(edges.left))
        })
        .unwrap_or(1)
        .max(0);
    let border_style = layout_style.and_then(|style| style.border_style);
    let radius_css = layout_style
        .and_then(|style| style.border_radius)
        .map(|radius| {
            format!(
                "border-radius: {}px {}px {}px {}px;",
                radius.top_left, radius.top_right, radius.bottom_right, radius.bottom_left
            )
        })
        .unwrap_or_else(|| "border-radius: 7px;".to_string());
    if matches!(border_style.map(|edges| edges.top), Some(BorderStyleValue::None))
        || border_width == 0
    {
        return format!(
            "background: {background}; {radius_css} box-shadow: 0 10px 30px rgba(0, 0, 0, 0.22);"
        );
    }

    if focused {
        return format!(
            "border: {}px solid transparent; {radius_css} background: linear-gradient({background}, {background}) padding-box, {} border-box; box-shadow: 0 12px 34px rgba(0, 0, 0, 0.24);",
            border_width.max(2),
            border_color,
        );
    }

    format!(
        "background: {background}; border: {}px solid {}; {radius_css} box-shadow: 0 10px 30px rgba(0, 0, 0, 0.22);",
        border_width.max(2),
        border_color,
    )
}

fn css_color(color: ColorValue) -> String {
    format!(
        "rgba({}, {}, {}, {:.3})",
        color.red,
        color.green,
        color.blue,
        f32::from(color.alpha) / 255.0
    )
}

fn length_to_px(length: LengthPercentage) -> i32 {
    match length {
        LengthPercentage::Px(value) | LengthPercentage::Percent(value) => value.round() as i32,
    }
    .max(0)
}
