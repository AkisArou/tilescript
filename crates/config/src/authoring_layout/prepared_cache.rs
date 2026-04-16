use tilescript_core::runtime::runtime_error::{RuntimeError, RuntimeRefreshSummary};

use crate::authoring_layout::AuthoringLayoutServiceError;
use crate::model::{Config, ConfigPaths};
use crate::runtime::AuthoringConfigRuntime;

pub(super) fn load_config_with_cache_update<C>(
    runtime: &C,
    paths: &ConfigPaths,
) -> Result<(Config, Option<RuntimeRefreshSummary>), AuthoringLayoutServiceError>
where
    C: AuthoringConfigRuntime + ?Sized,
{
    if paths.authored_config.exists() {
        let update =
            runtime.refresh_prepared_config(&paths.authored_config, &paths.prepared_config)?;
        match runtime.load_prepared_config(&paths.prepared_config) {
            Ok(config) => Ok((config, Some(update))),
            Err(load_error) => {
                let rebuild = runtime
                    .rebuild_prepared_config(&paths.authored_config, &paths.prepared_config)?;
                match runtime.load_prepared_config(&paths.prepared_config) {
                    Ok(config) => Ok((config, Some(merge_refresh_summaries(update, rebuild)))),
                    Err(_) => Err(load_error.into()),
                }
            }
        }
    } else if paths.prepared_config.exists() {
        Ok((runtime.load_prepared_config(&paths.prepared_config)?, None))
    } else {
        Ok((runtime.load_authored_config(&paths.authored_config)?, None))
    }
}

fn merge_refresh_summaries(
    update: RuntimeRefreshSummary,
    rebuild: RuntimeRefreshSummary,
) -> RuntimeRefreshSummary {
    RuntimeRefreshSummary {
        refreshed_files: update.refreshed_files + rebuild.refreshed_files,
        pruned_files: update.pruned_files + rebuild.pruned_files,
    }
}

pub(super) fn load_authored_config<C>(
    runtime: &C,
    paths: &ConfigPaths,
) -> Result<Config, AuthoringLayoutServiceError>
where
    C: AuthoringConfigRuntime + ?Sized,
{
    Ok(runtime.load_authored_config(&paths.authored_config)?)
}

pub(super) fn write_prepared_config<C>(
    runtime: &C,
    paths: &ConfigPaths,
) -> Result<RuntimeRefreshSummary, AuthoringLayoutServiceError>
where
    C: AuthoringConfigRuntime + ?Sized,
{
    Ok(runtime.rebuild_prepared_config(&paths.authored_config, &paths.prepared_config)?)
}

pub(super) fn reload_config<C>(
    runtime: &C,
    paths: Option<&ConfigPaths>,
) -> Result<Config, AuthoringLayoutServiceError>
where
    C: AuthoringConfigRuntime + ?Sized,
{
    let Some(paths) = paths else {
        return Err(RuntimeError::Other {
            message: "prepared config reload requires configured paths".into(),
        }
        .into());
    };

    let _ = write_prepared_config(runtime, paths)?;
    Ok(runtime.load_prepared_config(&paths.prepared_config)?)
}
