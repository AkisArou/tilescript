use std::path::Path;

use crate::model::{LayoutConfigError, LayoutDefinition, LayoutRule};

pub fn validate_layout_selection(
    path: &Path,
    default_layout: Option<&str>,
    layout_rules: &[LayoutRule],
    layouts: &[LayoutDefinition],
) -> Result<(), LayoutConfigError> {
    let known = layouts.iter().map(|layout| layout.name.as_str()).collect::<Vec<_>>();
    let is_known = |name: &str| known.iter().any(|known_name| *known_name == name);

    if let Some(default) = default_layout
        && !is_known(default)
    {
        return Err(LayoutConfigError::DecodeAuthoredConfig {
            path: path.to_path_buf(),
            message: format!(
                "selected layout `{default}` is not defined by discovered layout modules"
            ),
        });
    }

    for rule in layout_rules {
        if !is_known(&rule.layout) {
            return Err(LayoutConfigError::DecodeAuthoredConfig {
                path: path.to_path_buf(),
                message: format!(
                    "selected layout `{}` is not defined by discovered layout modules",
                    rule.layout
                ),
            });
        }
    }

    Ok(())
}
