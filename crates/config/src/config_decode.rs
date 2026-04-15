use std::path::Path;

use serde_json::Value;

use crate::model::{Config, LayoutConfigError, LayoutRule, ResizeConfig};

pub fn decode_config_value(path: &Path, value: &Value) -> Result<Config, LayoutConfigError> {
    let root = expect_object(path, value, "root")?;

    Ok(Config {
        layouts: Vec::new(),
        global_stylesheet_path: None,
        default_layout: decode_optional_string(
            root.get("defaultLayout"),
            path,
            "root.defaultLayout",
        )?,
        layout_rules: decode_layout_rules(root.get("layoutRules"), path)?,
        resize: decode_resize_config(root.get("resize"), path)?,
    })
}

fn decode_layout_rules(
    value: Option<&Value>,
    path: &Path,
) -> Result<Vec<LayoutRule>, LayoutConfigError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let items = expect_array(path, value, "root.layoutRules")?;

    items
        .iter()
        .enumerate()
        .map(|(index, value)| {
            decode_layout_rule(value, path, &format!("root.layoutRules[{index}]"))
        })
        .collect()
}

fn decode_layout_rule(
    value: &Value,
    path: &Path,
    field: &str,
) -> Result<LayoutRule, LayoutConfigError> {
    let object = expect_object(path, value, field)?;

    Ok(LayoutRule {
        layout: expect_string(
            path,
            object.get("layout").ok_or_else(|| LayoutConfigError::DecodeAuthoredConfig {
                path: path.to_path_buf(),
                message: format!("expected string at {field}.layout"),
            })?,
            &format!("{field}.layout"),
        )?
        .to_owned(),
        index: decode_optional_usize(object.get("index"), path, &format!("{field}.index"))?,
        name: decode_optional_string(object.get("name"), path, &format!("{field}.name"))?,
        monitor: decode_optional_string(object.get("monitor"), path, &format!("{field}.monitor"))?,
    })
}

fn decode_resize_config(
    value: Option<&Value>,
    path: &Path,
) -> Result<ResizeConfig, LayoutConfigError> {
    let Some(value) = value else {
        return Ok(ResizeConfig::default());
    };
    let object = expect_object(path, value, "root.resize")?;

    Ok(ResizeConfig {
        step_px: decode_optional_f32(object.get("stepPx"), path, "root.resize.stepPx")?,
        min_branch_size_px: decode_optional_f32(
            object.get("minBranchSizePx"),
            path,
            "root.resize.minBranchSizePx",
        )?,
    })
}

fn decode_optional_string(
    value: Option<&Value>,
    path: &Path,
    field: &str,
) -> Result<Option<String>, LayoutConfigError> {
    value.map(|value| expect_string(path, value, field).map(str::to_owned)).transpose()
}

fn decode_optional_usize(
    value: Option<&Value>,
    path: &Path,
    field: &str,
) -> Result<Option<usize>, LayoutConfigError> {
    value.map(|value| expect_usize(path, value, field)).transpose()
}

fn decode_optional_f32(
    value: Option<&Value>,
    path: &Path,
    field: &str,
) -> Result<Option<f32>, LayoutConfigError> {
    value.map(|value| expect_f32(path, value, field)).transpose()
}

fn expect_object<'a>(
    path: &Path,
    value: &'a Value,
    field: &str,
) -> Result<&'a serde_json::Map<String, Value>, LayoutConfigError> {
    value.as_object().ok_or_else(|| LayoutConfigError::DecodeAuthoredConfig {
        path: path.to_path_buf(),
        message: format!("expected object at {field}"),
    })
}

fn expect_array<'a>(
    path: &Path,
    value: &'a Value,
    field: &str,
) -> Result<&'a Vec<Value>, LayoutConfigError> {
    value.as_array().ok_or_else(|| LayoutConfigError::DecodeAuthoredConfig {
        path: path.to_path_buf(),
        message: format!("expected array at {field}"),
    })
}

fn expect_string<'a>(
    path: &Path,
    value: &'a Value,
    field: &str,
) -> Result<&'a str, LayoutConfigError> {
    value.as_str().ok_or_else(|| LayoutConfigError::DecodeAuthoredConfig {
        path: path.to_path_buf(),
        message: format!("expected string at {field}"),
    })
}

fn expect_f32(path: &Path, value: &Value, field: &str) -> Result<f32, LayoutConfigError> {
    value
        .as_f64()
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| value as f32)
        .ok_or_else(|| LayoutConfigError::DecodeAuthoredConfig {
            path: path.to_path_buf(),
            message: format!("expected non-negative number at {field}"),
        })
}

fn expect_usize(path: &Path, value: &Value, field: &str) -> Result<usize, LayoutConfigError> {
    value.as_u64().and_then(|value| usize::try_from(value).ok()).ok_or_else(|| {
        LayoutConfigError::DecodeAuthoredConfig {
            path: path.to_path_buf(),
            message: format!("expected non-negative integer at {field}"),
        }
    })
}
