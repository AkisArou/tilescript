use tilescript_core::LayoutRect;
use tilescript_css::{LengthPercentage, OverflowValue};
use tilescript_scene::ComputedStyle;

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
        "will-change: transform, opacity; transition-property: transform, opacity, box-shadow, filter; transition-duration: 220ms, 170ms, 170ms, 190ms; transition-timing-function: cubic-bezier(0.23, 1, 0.32, 1), cubic-bezier(0.5, 0.5, 0.75, 1), cubic-bezier(0.15, 0, 0.1, 1), cubic-bezier(0.5, 0.5, 0.75, 1);"
    } else {
        "will-change: auto; transition: none;"
    };

    format!(
        "left: {left:.3}%; top: {top:.3}%; width: {width:.3}%; height: {height:.3}%; --accent: {}; transform: translate(0%, 0%); {transition_css}",
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
    let _ = layout_style;
    let background = if focused {
        "var(--color-preview-frame-focused-bg)"
    } else {
        "var(--color-preview-frame-bg)"
    };

    if focused {
        return format!(
            "border: 2px solid transparent; border-radius: 7px; background: linear-gradient({background}, {background}) padding-box, linear-gradient(180deg, var(--color-preview-frame-focus-from), var(--color-preview-frame-focus-to)) border-box; box-shadow: 0 12px 34px var(--color-preview-frame-shadow-strong);",
        );
    }

    format!(
        "background: {background}; border: 2px solid var(--color-preview-frame-border); border-radius: 7px; box-shadow: 0 10px 30px var(--color-preview-frame-shadow);",
    )
}

fn length_to_px(length: LengthPercentage) -> i32 {
    match length {
        LengthPercentage::Px(value) | LengthPercentage::Percent(value) => value.round() as i32,
    }
    .max(0)
}
