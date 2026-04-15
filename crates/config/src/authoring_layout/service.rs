use std::collections::BTreeMap;

use hypreact_core::runtime::layout_context::{
    LayoutEvaluationContext, LayoutEvaluationDependencies,
};
use hypreact_core::runtime::prepared_layout::PreparedLayout;
use hypreact_core::runtime::runtime_contract::PreparedLayoutRuntime;
use hypreact_core::runtime::runtime_error::RuntimeError;
use hypreact_core::snapshot::{StateSnapshot, WorkspaceSnapshot};
use hypreact_core::types::LayoutRef;
use hypreact_core::{SourceLayoutNode, WorkspaceId};
use tracing::{debug, info, warn};

use super::config_paths;
use super::prepared_cache;
use crate::model::{Config, ConfigDiscoveryOptions, ConfigPaths, LayoutConfigError};
use crate::runtime::AuthoringConfigRuntime;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum AuthoringLayoutServiceError {
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error(transparent)]
    Config(#[from] LayoutConfigError),
}

#[derive(Debug)]
pub struct AuthoringLayoutService {
    config_runtime: Box<dyn AuthoringConfigRuntime>,
    layout_runtime: Box<dyn PreparedLayoutRuntime<Config = Config>>,
    cache: BTreeMap<String, PreparedLayout>,
    paths: Option<ConfigPaths>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedLayoutEvaluation {
    pub artifact: PreparedLayout,
    pub context: LayoutEvaluationContext,
    pub layout: SourceLayoutNode,
    pub dependencies: LayoutEvaluationDependencies,
}

impl AuthoringLayoutService {
    pub fn new<C, L>(config_runtime: C, layout_runtime: L) -> Self
    where
        C: AuthoringConfigRuntime + 'static,
        L: PreparedLayoutRuntime<Config = Config> + 'static,
    {
        Self {
            config_runtime: Box::new(config_runtime),
            layout_runtime: Box::new(layout_runtime),
            cache: BTreeMap::new(),
            paths: None,
        }
    }

    pub fn with_paths<C, L>(config_runtime: C, layout_runtime: L, paths: ConfigPaths) -> Self
    where
        C: AuthoringConfigRuntime + 'static,
        L: PreparedLayoutRuntime<Config = Config> + 'static,
    {
        Self {
            config_runtime: Box::new(config_runtime),
            layout_runtime: Box::new(layout_runtime),
            cache: BTreeMap::new(),
            paths: Some(paths),
        }
    }

    pub(crate) fn from_runtime_bundle(
        config_runtime: Box<dyn AuthoringConfigRuntime>,
        layout_runtime: Box<dyn PreparedLayoutRuntime<Config = Config>>,
        paths: ConfigPaths,
    ) -> Self {
        Self { config_runtime, layout_runtime, cache: BTreeMap::new(), paths: Some(paths) }
    }

    pub fn discover_config_paths(
        &self,
        options: ConfigDiscoveryOptions,
    ) -> Result<ConfigPaths, AuthoringLayoutServiceError> {
        config_paths::discover_config_paths(options)
    }

    pub fn load_config(&self, paths: &ConfigPaths) -> Result<Config, AuthoringLayoutServiceError> {
        debug!(authored_config = %paths.authored_config.display(), prepared_config = %paths.prepared_config.display(), "loading config");
        Ok(self.load_config_with_cache_update(paths)?.0)
    }

    pub fn load_config_with_cache_update(
        &self,
        paths: &ConfigPaths,
    ) -> Result<
        (Config, Option<hypreact_core::runtime::runtime_error::RuntimeRefreshSummary>),
        AuthoringLayoutServiceError,
    > {
        prepared_cache::load_config_with_cache_update(self.config_runtime.as_ref(), paths)
    }

    pub fn load_authored_config(
        &self,
        paths: &ConfigPaths,
    ) -> Result<Config, AuthoringLayoutServiceError> {
        prepared_cache::load_authored_config(self.config_runtime.as_ref(), paths)
    }

    pub fn write_prepared_config(
        &self,
        paths: &ConfigPaths,
        _config: &Config,
    ) -> Result<
        hypreact_core::runtime::runtime_error::RuntimeRefreshSummary,
        AuthoringLayoutServiceError,
    > {
        prepared_cache::write_prepared_config(self.config_runtime.as_ref(), paths)
    }

    pub fn reload_config(&mut self) -> Result<Config, AuthoringLayoutServiceError> {
        debug!("reloading config and clearing prepared layout cache");
        let config =
            prepared_cache::reload_config(self.config_runtime.as_ref(), self.paths.as_ref())?;
        self.cache.clear();
        info!(layout_count = config.layouts.len(), "reloaded config");
        Ok(config)
    }

    pub fn validate_layout_modules(
        &self,
        config: &Config,
    ) -> Result<Vec<String>, AuthoringLayoutServiceError> {
        debug!(layout_count = config.layouts.len(), "validating layout modules");
        let mut errors = Vec::new();

        for layout in &config.layouts {
            let workspace = validation_workspace(&layout.name);

            if let Err(error) = self.layout_runtime.prepare_layout(config, &workspace) {
                warn!(layout = %layout.name, %error, "layout validation failed");
                errors.push(format!("{}: {error}", layout.name));
            }
        }

        Ok(errors)
    }

    pub fn prepare_for_workspace(
        &mut self,
        config: &Config,
        workspace: &WorkspaceSnapshot,
    ) -> Result<Option<&PreparedLayout>, AuthoringLayoutServiceError> {
        debug!(workspace_id = %workspace.id, workspace_name = %workspace.name, "preparing layout for workspace");
        let Some(loaded) = self.layout_runtime.prepare_layout(config, workspace)? else {
            debug!(workspace_id = %workspace.id, workspace_name = %workspace.name, "no selected layout for workspace");
            return Ok(None);
        };

        let key = loaded.selected.name.clone();
        self.cache.insert(key.clone(), loaded);
        debug!(workspace_id = %workspace.id, workspace_name = %workspace.name, layout = %key, "prepared layout cached");
        Ok(self.cache.get(&key))
    }

    pub fn evaluate_prepared_for_workspace(
        &mut self,
        config: &Config,
        state: &StateSnapshot,
        workspace: &WorkspaceSnapshot,
    ) -> Result<Option<PreparedLayoutEvaluation>, AuthoringLayoutServiceError> {
        debug!(workspace_id = %workspace.id, workspace_name = %workspace.name, window_count = state.windows.len(), "evaluating prepared layout for workspace");
        let Some(loaded) = self.prepare_for_workspace(config, workspace)?.cloned() else {
            return Ok(None);
        };
        let context = self.layout_runtime.build_context(state, workspace, Some(&loaded));
        let layout = self.layout_runtime.evaluate_layout(&loaded, &context)?;

        debug!(workspace_id = %workspace.id, workspace_name = %workspace.name, layout = %loaded.selected.name, "evaluated prepared layout");

        Ok(Some(PreparedLayoutEvaluation {
            artifact: loaded,
            context,
            layout,
            dependencies: LayoutEvaluationDependencies::default(),
        }))
    }

    pub fn cache(&self) -> &BTreeMap<String, PreparedLayout> {
        &self.cache
    }
}

fn validation_workspace(layout_name: &str) -> WorkspaceSnapshot {
    WorkspaceSnapshot {
        id: WorkspaceId::from("validation"),
        name: "validation".into(),
        output_id: None,
        layout_space: None,
        active_workspaces: vec![],
        focused: true,
        visible: true,
        effective_layout: Some(LayoutRef { name: layout_name.into() }),
    }
}
