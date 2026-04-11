use crate::authoring_layout::AuthoringLayoutServiceError;
use crate::model::{ConfigDiscoveryOptions, ConfigPaths};

pub(super) fn discover_config_paths(
    options: ConfigDiscoveryOptions,
) -> Result<ConfigPaths, AuthoringLayoutServiceError> {
    Ok(ConfigPaths::discover(options)?)
}
