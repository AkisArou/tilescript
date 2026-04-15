use hypreact_core::LayoutRect;
use hypreact_css::{BorderStyleValue, ColorValue, LengthPercentage, OverflowValue};
use hypreact_scene::ComputedStyle;

pub fn pane_style(
    geometry: LayoutRect,
    accent: &str,
    canvas_width: i32,
    canvas_height: i32,
) -> String {
    let left = geometry.x as f32 / canvas_width as f32 * 100.0;
    let top = geometry.y as f32 / canvas_height as f32 * 100.0;
    let width = geometry.width as f32 / canvas_width as f32 * 100.0;
    let height = geometry.height as f32 / canvas_height as f32 * 100.0;

    format!(
        "left: {left:.3}%; top: {top:.3}%; width: {width:.3}%; height: {height:.3}%; --accent: {};",
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
        "rgba(125, 211, 199, 1.0)".to_string()
    } else {
        layout_style
            .and_then(|style| style.border_color)
            .or_else(|| {
                layout_style
                    .and_then(|style| style.border_side_colors)
                    .and_then(|colors| colors.top)
            })
            .map(css_color)
            .unwrap_or_else(|| "rgba(47, 54, 71, 1.0)".to_string())
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
        .unwrap_or_default();
    let border_css = if matches!(border_style.map(|edges| edges.top), Some(BorderStyleValue::None))
        || border_width == 0
    {
        "border: none;".to_string()
    } else {
        format!("border: {}px solid {};", border_width.max(1), border_color)
    };

    format!("background: {background}; {border_css} {radius_css}")
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
