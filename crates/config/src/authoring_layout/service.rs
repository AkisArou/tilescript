use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use tilescript_core::runtime::artifact_state::{
    ArtifactGraph, ArtifactKey, ArtifactRecord, ArtifactRegistry,
};
use tilescript_core::runtime::layout_context::{
    LayoutEvaluationContext, LayoutEvaluationDependencies,
};
use tilescript_core::runtime::native_artifact::dependencies_match;
use tilescript_core::runtime::prepared_layout::PreparedLayout;
use tilescript_core::runtime::runtime_contract::PreparedLayoutRuntime;
use tilescript_core::runtime::runtime_error::RuntimeError;
use tilescript_core::runtime::runtime_kind::RuntimeKind;
use tilescript_core::snapshot::{StateSnapshot, WorkspaceSnapshot};
use tilescript_core::types::LayoutRef;
use tilescript_core::{SourceLayoutNode, WorkspaceId};
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
    layout_artifacts: ArtifactRegistry<PreparedLayout>,
    artifact_graph: ArtifactGraph,
    config_artifact: Option<ArtifactRecord<Config>>,
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
            layout_artifacts: ArtifactRegistry::new(),
            artifact_graph: ArtifactGraph::new(),
            config_artifact: None,
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
            layout_artifacts: ArtifactRegistry::new(),
            artifact_graph: ArtifactGraph::new(),
            config_artifact: None,
            paths: Some(paths),
        }
    }

    pub(crate) fn from_runtime_bundle(
        config_runtime: Box<dyn AuthoringConfigRuntime>,
        layout_runtime: Box<dyn PreparedLayoutRuntime<Config = Config>>,
        paths: ConfigPaths,
    ) -> Self {
        Self {
            config_runtime,
            layout_runtime,
            layout_artifacts: ArtifactRegistry::new(),
            artifact_graph: ArtifactGraph::new(),
            config_artifact: None,
            paths: Some(paths),
        }
    }

    pub fn discover_config_paths(
        &self,
        options: ConfigDiscoveryOptions,
    ) -> Result<ConfigPaths, AuthoringLayoutServiceError> {
        config_paths::discover_config_paths(options)
    }

    pub fn load_config(
        &mut self,
        paths: &ConfigPaths,
    ) -> Result<Config, AuthoringLayoutServiceError> {
        debug!(authored_config = %paths.authored_config.display(), prepared_config = %paths.prepared_config.display(), "loading config");
        let (config, update) = self.load_config_with_cache_update(paths)?;
        if update
            .as_ref()
            .is_none_or(|summary| summary.refreshed_files == 0 && summary.pruned_files == 0)
        {
            if let Some(cached) = self.config_artifact.as_ref() {
                let fingerprint = config_fingerprint(&config);
                if cached.fingerprint == fingerprint {
                    return Ok(cached.value.clone());
                }
            }
        }

        self.store_loaded_config(config.clone());
        Ok(config)
    }

    pub fn load_config_with_cache_update(
        &self,
        paths: &ConfigPaths,
    ) -> Result<
        (Config, Option<tilescript_core::runtime::runtime_error::RuntimeRefreshSummary>),
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
        tilescript_core::runtime::runtime_error::RuntimeRefreshSummary,
        AuthoringLayoutServiceError,
    > {
        prepared_cache::write_prepared_config(self.config_runtime.as_ref(), paths)
    }

    pub fn reload_config(&mut self) -> Result<Config, AuthoringLayoutServiceError> {
        debug!("reloading config and clearing prepared layout cache");
        let config =
            prepared_cache::reload_config(self.config_runtime.as_ref(), self.paths.as_ref())?;
        self.invalidate_config_dependents();
        self.store_loaded_config(config.clone());
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
        self.prune_layout_artifacts(config);
        if let Some(selected_layout) = config.resolve_selected_layout(workspace)? {
            let current_runtime_payload = config
                .layouts
                .iter()
                .find(|layout| layout.name == selected_layout.name)
                .and_then(|layout| layout.runtime_cache_payload.clone())
                .unwrap_or(serde_json::Value::Null);
            let can_reuse_prepared = self
                .layout_artifacts
                .get(&ArtifactKey::layout(selected_layout.name.clone()))
                .is_some_and(|cached| {
                    let payload_matches = match selected_layout.runtime {
                        RuntimeKind::Js => cached.value.runtime_payload == current_runtime_payload,
                        _ => true,
                    };
                    cached.value.selected == selected_layout
                        && payload_matches
                        && dependencies_match(&cached.value.dependencies)
                });

            if can_reuse_prepared {
                debug!(workspace_id = %workspace.id, workspace_name = %workspace.name, layout = %selected_layout.name, "reused prepared layout without runtime preparation");
                return Ok(self
                    .layout_artifacts
                    .get(&ArtifactKey::layout(selected_layout.name.clone()))
                    .map(|entry| &entry.value));
            }
        }

        let Some(loaded) = self.layout_runtime.prepare_layout(config, workspace)? else {
            debug!(workspace_id = %workspace.id, workspace_name = %workspace.name, "no selected layout for workspace");
            return Ok(None);
        };

        let key = loaded.selected.name.clone();
        let fingerprint = prepared_layout_fingerprint(workspace, &loaded);

        let artifact_key = ArtifactKey::layout(key.clone());
        let reused_cached_layout = self
            .layout_artifacts
            .get(&artifact_key)
            .is_some_and(|cached| cached.fingerprint == fingerprint);

        if reused_cached_layout {
            debug!(workspace_id = %workspace.id, workspace_name = %workspace.name, layout = %key, "reused prepared layout cache entry");
            return Ok(self.layout_artifacts.get(&artifact_key).map(|entry| &entry.value));
        }

        self.layout_artifacts
            .insert(artifact_key.clone(), ArtifactRecord::new(fingerprint, loaded));
        self.record_layout_dependencies(&artifact_key);
        debug!(workspace_id = %workspace.id, workspace_name = %workspace.name, layout = %key, "prepared layout cached");
        Ok(self.layout_artifacts.get(&artifact_key).map(|entry| &entry.value))
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

    pub fn cache(&self) -> BTreeMap<String, PreparedLayout> {
        self.layout_artifacts
            .iter()
            .filter_map(|(key, entry)| match key.kind {
                tilescript_core::runtime::artifact_state::ArtifactKind::Layout => {
                    Some((key.identity.clone(), entry.value.clone()))
                }
                _ => None,
            })
            .collect()
    }

    pub fn cached_layouts(&self) -> impl Iterator<Item = &PreparedLayout> {
        self.layout_artifacts.values().map(|entry| &entry.value)
    }

    fn store_loaded_config(&mut self, config: Config) {
        let fingerprint = config_fingerprint(&config);
        self.config_artifact = Some(ArtifactRecord::new(fingerprint, config));
    }

    fn prune_layout_artifacts(&mut self, config: &Config) {
        let mut removed = Vec::new();
        self.layout_artifacts.retain(|key, _| {
            let keep = key.kind != tilescript_core::runtime::artifact_state::ArtifactKind::Layout
                || config.layouts.iter().any(|layout| layout.name == key.identity);
            if !keep {
                removed.push(key.clone());
            }
            keep
        });
        for key in removed {
            self.artifact_graph.invalidate(&key, &mut self.layout_artifacts);
        }
    }

    fn invalidate_config_dependents(&mut self) {
        let config_key = ArtifactKey::config("authoring-config");
        self.artifact_graph.invalidate_dependents_of(&config_key, &mut self.layout_artifacts);
        self.artifact_graph.remove(&config_key);
    }

    fn record_layout_dependencies(&mut self, layout_key: &ArtifactKey) {
        let _ = layout_key;
        let config_key = ArtifactKey::config("authoring-config");
        let layout_keys = self
            .layout_artifacts
            .iter()
            .filter_map(|(key, _)| match key.kind {
                tilescript_core::runtime::artifact_state::ArtifactKind::Layout => Some(key.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        self.artifact_graph.replace_edges(config_key, layout_keys);
    }
}

fn config_fingerprint(config: &Config) -> String {
    hash_string(
        &serde_json::to_string(config)
            .unwrap_or_else(|_| format!("config-layouts:{}", config.layouts.len())),
    )
}

fn prepared_layout_fingerprint(workspace: &WorkspaceSnapshot, layout: &PreparedLayout) -> String {
    let mut fingerprint = String::new();
    fingerprint.push_str(&layout.selected.name);
    fingerprint.push('|');
    fingerprint.push_str(&layout.selected.module);
    fingerprint.push('|');
    fingerprint.push_str(workspace.id.as_str());
    fingerprint.push('|');
    fingerprint.push_str(workspace.name.as_str());
    fingerprint.push('|');
    fingerprint.push_str(&hash_string(
        &serde_json::to_string(&layout.runtime_payload).unwrap_or_default(),
    ));
    fingerprint.push('|');
    fingerprint
        .push_str(&hash_string(&serde_json::to_string(&layout.dependencies).unwrap_or_default()));
    hash_string(&fingerprint)
}

fn hash_string(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LayoutDefinition;
    use tilescript_core::runtime::prepared_layout::{
        PreparedStylesheet, PreparedStylesheets, SelectedLayout,
    };
    use tilescript_core::runtime::runtime_contract::LayoutModuleContract;
    use tilescript_core::runtime::runtime_error::{RuntimeError, RuntimeRefreshSummary};
    use tilescript_core::runtime::runtime_kind::RuntimeKind;
    use std::path::Path;

    #[derive(Debug, Clone, Default)]
    struct StubConfigRuntime {
        config: Config,
        refresh_summary: RuntimeRefreshSummary,
    }

    impl crate::runtime::AuthoringConfigRuntime for StubConfigRuntime {
        fn load_authored_config(&self, _path: &Path) -> Result<Config, RuntimeError> {
            Ok(self.config.clone())
        }

        fn load_prepared_config(&self, _path: &Path) -> Result<Config, RuntimeError> {
            Ok(self.config.clone())
        }

        fn refresh_prepared_config(
            &self,
            _authored: &Path,
            _prepared: &Path,
        ) -> Result<RuntimeRefreshSummary, RuntimeError> {
            Ok(self.refresh_summary.clone())
        }

        fn rebuild_prepared_config(
            &self,
            _authored: &Path,
            _prepared: &Path,
        ) -> Result<RuntimeRefreshSummary, RuntimeError> {
            Ok(self.refresh_summary.clone())
        }
    }

    #[derive(Debug, Clone, Default)]
    struct StubLayoutRuntime {
        prepared_layout: Option<PreparedLayout>,
        prepare_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl PreparedLayoutRuntime for StubLayoutRuntime {
        type Config = Config;

        fn prepare_layout(
            &self,
            _config: &Self::Config,
            _workspace: &WorkspaceSnapshot,
        ) -> Result<Option<PreparedLayout>, RuntimeError> {
            self.prepare_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(self.prepared_layout.clone())
        }

        fn build_context(
            &self,
            state: &StateSnapshot,
            workspace: &WorkspaceSnapshot,
            artifact: Option<&PreparedLayout>,
        ) -> LayoutEvaluationContext {
            state.layout_context(workspace, artifact.map(|artifact| artifact.selected.clone()))
        }

        fn evaluate_layout(
            &self,
            _artifact: &PreparedLayout,
            _context: &LayoutEvaluationContext,
        ) -> Result<SourceLayoutNode, RuntimeError> {
            Ok(SourceLayoutNode::Workspace { meta: Default::default(), children: vec![] })
        }

        fn contract(&self) -> LayoutModuleContract {
            LayoutModuleContract::default()
        }
    }

    #[test]
    fn reuses_cached_config_when_refresh_reports_no_changes() {
        let config = Config {
            layouts: vec![LayoutDefinition {
                name: "master-stack".into(),
                runtime: RuntimeKind::Lua,
                directory: "layouts/master-stack".into(),
                module: "layouts/master-stack/index.lua".into(),
                stylesheet_path: None,
                runtime_cache_payload: None,
            }],
            default_layout: Some("master-stack".into()),
            ..Config::default()
        };
        let paths = ConfigPaths::new("config.lua", ".tilescript-build/config.js");
        let mut service = AuthoringLayoutService::with_paths(
            StubConfigRuntime {
                config: config.clone(),
                refresh_summary: RuntimeRefreshSummary { refreshed_files: 0, pruned_files: 0 },
            },
            StubLayoutRuntime::default(),
            paths.clone(),
        );

        let first = service.load_config(&paths).unwrap();
        let second = service.load_config(&paths).unwrap();

        assert_eq!(first, second);
        assert!(service.config_artifact.is_some());
    }

    #[test]
    fn reuses_prepared_layout_when_fingerprint_matches() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let stylesheet_path = temp_dir.path().join("master-stack.css");
        std::fs::write(&stylesheet_path, ".master-slot { flex: 1; }").unwrap();
        let stylesheet = PreparedStylesheet {
            path: stylesheet_path.to_string_lossy().into_owned(),
            source: ".master-slot { flex: 1; }".into(),
        };
        let layout = PreparedLayout {
            selected: SelectedLayout {
                name: "master-stack".into(),
                runtime: RuntimeKind::Lua,
                directory: "layouts/master-stack".into(),
                module: "layouts/master-stack/index.lua".into(),
            },
            runtime_payload: serde_json::json!({ "source": "return function() end" }),
            stylesheets: PreparedStylesheets { global: None, layout: Some(stylesheet) },
            dependencies: vec![tilescript_core::runtime::native_artifact::NativeDependencySnapshot {
                path: stylesheet_path.to_string_lossy().into_owned(),
                content_hash: {
                    use std::collections::hash_map::DefaultHasher;
                    use std::hash::{Hash, Hasher};

                    let mut hasher = DefaultHasher::new();
                    ".master-slot { flex: 1; }".hash(&mut hasher);
                    format!("{:016x}", hasher.finish())
                },
            }],
        };
        let workspace = validation_workspace("master-stack");
        let mut service = AuthoringLayoutService::new(
            StubConfigRuntime::default(),
            StubLayoutRuntime {
                prepared_layout: Some(layout.clone()),
                prepare_count: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            },
        );

        let config = Config {
            layouts: vec![LayoutDefinition {
                name: "master-stack".into(),
                runtime: RuntimeKind::Lua,
                directory: "layouts/master-stack".into(),
                module: "layouts/master-stack/index.lua".into(),
                stylesheet_path: None,
                runtime_cache_payload: None,
            }],
            default_layout: Some("master-stack".into()),
            ..Config::default()
        };

        let first = service.prepare_for_workspace(&config, &workspace).unwrap().unwrap().clone();
        let second = service.prepare_for_workspace(&config, &workspace).unwrap().unwrap().clone();

        assert_eq!(first, second);
        assert_eq!(service.layout_artifacts.len(), 1);
    }

    #[test]
    fn skips_runtime_prepare_when_cached_dependencies_still_match() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let stylesheet_path = temp_dir.path().join("master-stack.css");
        std::fs::write(&stylesheet_path, ".master-slot { flex: 1; }").unwrap();
        let prepare_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let layout = PreparedLayout {
            selected: SelectedLayout {
                name: "master-stack".into(),
                runtime: RuntimeKind::Lua,
                directory: "layouts/master-stack".into(),
                module: "layouts/master-stack/index.lua".into(),
            },
            runtime_payload: serde_json::json!({ "source": "return function() end" }),
            stylesheets: PreparedStylesheets {
                global: None,
                layout: Some(PreparedStylesheet {
                    path: stylesheet_path.to_string_lossy().into_owned(),
                    source: ".master-slot { flex: 1; }".into(),
                }),
            },
            dependencies: vec![tilescript_core::runtime::native_artifact::NativeDependencySnapshot {
                path: stylesheet_path.to_string_lossy().into_owned(),
                content_hash: {
                    use std::collections::hash_map::DefaultHasher;
                    use std::hash::{Hash, Hasher};

                    let mut hasher = DefaultHasher::new();
                    ".master-slot { flex: 1; }".hash(&mut hasher);
                    format!("{:016x}", hasher.finish())
                },
            }],
        };
        let workspace = validation_workspace("master-stack");
        let mut service = AuthoringLayoutService::new(
            StubConfigRuntime::default(),
            StubLayoutRuntime {
                prepared_layout: Some(layout),
                prepare_count: prepare_count.clone(),
            },
        );
        let config = Config {
            layouts: vec![LayoutDefinition {
                name: "master-stack".into(),
                runtime: RuntimeKind::Lua,
                directory: "layouts/master-stack".into(),
                module: "layouts/master-stack/index.lua".into(),
                stylesheet_path: Some(stylesheet_path.to_string_lossy().into_owned()),
                runtime_cache_payload: None,
            }],
            default_layout: Some("master-stack".into()),
            ..Config::default()
        };

        let _ = service.prepare_for_workspace(&config, &workspace).unwrap();
        let _ = service.prepare_for_workspace(&config, &workspace).unwrap();

        assert_eq!(prepare_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn re_prepares_when_dependency_snapshot_changes() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let stylesheet_path = temp_dir.path().join("master-stack.css");
        std::fs::write(&stylesheet_path, ".master-slot { flex: 1; }").unwrap();
        let prepare_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let workspace = validation_workspace("master-stack");

        let mut hasher = DefaultHasher::new();
        ".master-slot { flex: 1; }".hash(&mut hasher);
        let first_hash = format!("{:016x}", hasher.finish());

        let runtime = StubLayoutRuntime {
            prepared_layout: Some(PreparedLayout {
                selected: SelectedLayout {
                    name: "master-stack".into(),
                    runtime: RuntimeKind::Lua,
                    directory: "layouts/master-stack".into(),
                    module: "layouts/master-stack/index.lua".into(),
                },
                runtime_payload: serde_json::json!({ "source": "return function() end" }),
                stylesheets: PreparedStylesheets {
                    global: None,
                    layout: Some(PreparedStylesheet {
                        path: stylesheet_path.to_string_lossy().into_owned(),
                        source: ".master-slot { flex: 1; }".into(),
                    }),
                },
                dependencies: vec![
                    tilescript_core::runtime::native_artifact::NativeDependencySnapshot {
                        path: stylesheet_path.to_string_lossy().into_owned(),
                        content_hash: first_hash,
                    },
                ],
            }),
            prepare_count: prepare_count.clone(),
        };
        let mut service = AuthoringLayoutService::new(StubConfigRuntime::default(), runtime);
        let config = Config {
            layouts: vec![LayoutDefinition {
                name: "master-stack".into(),
                runtime: RuntimeKind::Lua,
                directory: "layouts/master-stack".into(),
                module: "layouts/master-stack/index.lua".into(),
                stylesheet_path: Some(stylesheet_path.to_string_lossy().into_owned()),
                runtime_cache_payload: None,
            }],
            default_layout: Some("master-stack".into()),
            ..Config::default()
        };

        let _ = service.prepare_for_workspace(&config, &workspace).unwrap();
        std::fs::write(&stylesheet_path, ".master-slot { flex: 2; }").unwrap();
        let _ = service.prepare_for_workspace(&config, &workspace).unwrap();

        assert_eq!(prepare_count.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[test]
    fn reuses_prepared_layout_once_runtime_payload_matches_cached_artifact() {
        let prepare_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let workspace = validation_workspace("master-stack");
        let runtime = StubLayoutRuntime {
            prepared_layout: Some(PreparedLayout {
                selected: SelectedLayout {
                    name: "master-stack".into(),
                    runtime: RuntimeKind::Js,
                    directory: "layouts/master-stack".into(),
                    module: "layouts/master-stack/index.js".into(),
                },
                runtime_payload: serde_json::json!({ "graph": "new" }),
                stylesheets: PreparedStylesheets::default(),
                dependencies: vec![],
            }),
            prepare_count: prepare_count.clone(),
        };
        let mut service = AuthoringLayoutService::new(StubConfigRuntime::default(), runtime);
        let config_a = Config {
            layouts: vec![LayoutDefinition {
                name: "master-stack".into(),
                runtime: RuntimeKind::Js,
                directory: "layouts/master-stack".into(),
                module: "layouts/master-stack/index.js".into(),
                stylesheet_path: None,
                runtime_cache_payload: Some(serde_json::json!({ "graph": "old" })),
            }],
            default_layout: Some("master-stack".into()),
            ..Config::default()
        };
        let config_b = Config {
            layouts: vec![LayoutDefinition {
                name: "master-stack".into(),
                runtime: RuntimeKind::Js,
                directory: "layouts/master-stack".into(),
                module: "layouts/master-stack/index.js".into(),
                stylesheet_path: None,
                runtime_cache_payload: Some(serde_json::json!({ "graph": "new" })),
            }],
            default_layout: Some("master-stack".into()),
            ..Config::default()
        };

        let _ = service.prepare_for_workspace(&config_a, &workspace).unwrap();
        let _ = service.prepare_for_workspace(&config_b, &workspace).unwrap();

        assert_eq!(prepare_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
