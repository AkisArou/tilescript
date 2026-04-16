use tilescript_core::SourceLayoutNode;
use tilescript_core::runtime::artifact_state::{
    ArtifactGraph, ArtifactKey, ArtifactRecord, ArtifactRegistry,
};
use tilescript_core::runtime::layout_context::{
    LayoutEvaluationContext, LayoutEvaluationDependencies,
};
use tilescript_core::runtime::prepared_layout::PreparedLayout;
use tilescript_core::runtime::runtime_kind::RuntimeKind;
use tilescript_core::snapshot::{StateSnapshot, WorkspaceSnapshot};

use crate::model::{Config, LayoutConfigError};
use crate::runtime::{SourceBundle, SourceBundleConfigRuntime, SourceBundlePreparedLayoutRuntime};

#[derive(Debug)]
pub struct SourceBundleAuthoringLayoutService {
    config_runtime: Box<dyn SourceBundleConfigRuntime>,
    layout_runtime: Box<dyn SourceBundlePreparedLayoutRuntime>,
    config_artifact: Option<ArtifactRecord<Config>>,
    layout_artifacts: ArtifactRegistry<PreparedLayout>,
    evaluation_artifacts: ArtifactRegistry<PreparedSourceBundleLayoutEvaluation>,
    artifact_graph: ArtifactGraph,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedSourceBundleLayoutEvaluation {
    pub artifact: PreparedLayout,
    pub context: LayoutEvaluationContext,
    pub layout: SourceLayoutNode,
    pub dependencies: LayoutEvaluationDependencies,
}

impl SourceBundleAuthoringLayoutService {
    pub(crate) fn from_runtime_bundle(
        config_runtime: Box<dyn SourceBundleConfigRuntime>,
        layout_runtime: Box<dyn SourceBundlePreparedLayoutRuntime>,
    ) -> Self {
        Self {
            config_runtime,
            layout_runtime,
            config_artifact: None,
            layout_artifacts: ArtifactRegistry::new(),
            evaluation_artifacts: ArtifactRegistry::new(),
            artifact_graph: ArtifactGraph::new(),
        }
    }

    pub async fn load_config(
        &mut self,
        root_dir: &std::path::Path,
        entry_path: &std::path::Path,
        sources: &SourceBundle,
    ) -> Result<Config, LayoutConfigError> {
        let config = self.config_runtime.load_config(root_dir, entry_path, sources).await?;
        self.prune_for_config(&config, root_dir, entry_path, sources);
        let fingerprint = config_fingerprint(&config)?;
        self.config_artifact = Some(ArtifactRecord::new(fingerprint, config.clone()));
        self.record_config_dependencies(root_dir, entry_path, sources, &config);
        Ok(config)
    }

    pub async fn prepare_for_workspace(
        &mut self,
        root_dir: &std::path::Path,
        sources: &SourceBundle,
        config: &Config,
        workspace: &WorkspaceSnapshot,
    ) -> Result<Option<&PreparedLayout>, LayoutConfigError> {
        let Some(loaded) =
            self.layout_runtime.prepare_layout(root_dir, sources, config, workspace).await?
        else {
            return Ok(None);
        };
        self.prune_for_layout(root_dir, sources, config, &loaded.selected.name);

        let key = loaded.selected.name.clone();
        let artifact_key = ArtifactKey::layout(key.clone());
        let fingerprint = prepared_layout_fingerprint(&loaded)?;
        let cache_miss = self
            .layout_artifacts
            .get(&artifact_key)
            .is_none_or(|cached| cached.fingerprint != fingerprint);

        if cache_miss {
            self.layout_artifacts
                .insert(artifact_key.clone(), ArtifactRecord::new(fingerprint, loaded));
        }

        let cached_layout = self
            .layout_artifacts
            .get(&artifact_key)
            .expect("layout cached")
            .value
            .clone();
        self.record_layout_dependencies(root_dir, sources, &artifact_key, &cached_layout);
        Ok(self.layout_artifacts.get(&artifact_key).map(|entry| &entry.value))
    }

    pub async fn evaluate_prepared_for_workspace(
        &mut self,
        root_dir: &std::path::Path,
        sources: &SourceBundle,
        config: &Config,
        state: &StateSnapshot,
        workspace: &WorkspaceSnapshot,
    ) -> Result<Option<PreparedSourceBundleLayoutEvaluation>, LayoutConfigError> {
        let Some(loaded) =
            self.prepare_for_workspace(root_dir, sources, config, workspace).await?.cloned()
        else {
            return Ok(None);
        };

        let context = self.layout_runtime.build_context(state, workspace, Some(&loaded));
        let evaluation_key = ArtifactKey::config(format!(
            "source-bundle-eval::{}::{}",
            loaded.selected.name,
            evaluation_context_fingerprint(&loaded, &context)?
        ));

        if let Some(cached) = self.evaluation_artifacts.get(&evaluation_key) {
            return Ok(Some(cached.value.clone()));
        }

        let evaluated =
            self.layout_runtime.evaluate_layout(root_dir, sources, &loaded, &context).await?;
        let layout_name = loaded.selected.name.clone();

        let result = PreparedSourceBundleLayoutEvaluation {
            artifact: loaded,
            context,
            layout: evaluated.layout,
            dependencies: evaluated.dependencies,
        };
        let fingerprint = evaluated_layout_fingerprint(&result)?;
        self.evaluation_artifacts
            .insert(evaluation_key.clone(), ArtifactRecord::new(fingerprint, result.clone()));
        let layout_key = ArtifactKey::layout(layout_name);
        let mut dependents = self.artifact_graph.dependents_of(&layout_key).cloned().collect::<Vec<_>>();
        if !dependents.contains(&evaluation_key) {
            dependents.push(evaluation_key);
        }
        self.artifact_graph.replace_edges(layout_key, dependents);

        Ok(Some(result))
    }

    pub fn cached_layouts(&self) -> impl Iterator<Item = &PreparedLayout> {
        self.layout_artifacts.values().map(|entry| &entry.value)
    }

    pub fn cached_evaluations(&self) -> impl Iterator<Item = &PreparedSourceBundleLayoutEvaluation> {
        self.evaluation_artifacts.values().map(|entry| &entry.value)
    }

    pub fn dependency_graph(&self) -> &ArtifactGraph {
        &self.artifact_graph
    }

    fn prune_for_config(
        &mut self,
        config: &Config,
        root_dir: &std::path::Path,
        entry_path: &std::path::Path,
        sources: &SourceBundle,
    ) {
        let active_layouts = config.layouts.iter().map(|layout| layout.name.as_str()).collect::<std::collections::BTreeSet<_>>();
        let mut removed_layouts = Vec::new();
        self.layout_artifacts.retain(|key, _| {
            let keep = key.kind != tilescript_core::runtime::artifact_state::ArtifactKind::Layout
                || active_layouts.contains(key.identity.as_str());
            if !keep {
                removed_layouts.push(key.clone());
            }
            keep
        });
        for key in removed_layouts {
            self.artifact_graph.invalidate(&key, &mut self.layout_artifacts);
            let stale_evaluations = self
                .evaluation_artifacts
                .iter()
                .filter(|(evaluation_key, _)| {
                    evaluation_key.identity.starts_with(&format!("source-bundle-eval::{}::", key.identity))
                })
                .map(|(evaluation_key, _)| evaluation_key.clone())
                .collect::<Vec<_>>();
            for evaluation_key in stale_evaluations {
                self.artifact_graph.invalidate(&evaluation_key, &mut self.evaluation_artifacts);
            }
            self.artifact_graph.remove(&key);
        }

        self.prune_authored_file_nodes(root_dir, entry_path, sources, config);
    }

    fn prune_for_layout(
        &mut self,
        root_dir: &std::path::Path,
        sources: &SourceBundle,
        config: &Config,
        active_layout_name: &str,
    ) {
        let stale_evaluations = self
            .evaluation_artifacts
            .iter()
            .filter(|(key, _)| {
                key.identity.starts_with("source-bundle-eval::")
                    && !key.identity.starts_with(&format!("source-bundle-eval::{active_layout_name}::"))
            })
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        for key in stale_evaluations {
            self.artifact_graph.invalidate(&key, &mut self.evaluation_artifacts);
        }
        self.prune_authored_file_nodes(root_dir, root_dir, sources, config);
    }

    fn prune_authored_file_nodes(
        &mut self,
        root_dir: &std::path::Path,
        entry_path: &std::path::Path,
        sources: &SourceBundle,
        config: &Config,
    ) {
        let mut valid_authored_files = std::collections::BTreeSet::new();
        valid_authored_files.insert(resolve_source_bundle_path(root_dir, entry_path.to_string_lossy().as_ref()));
        for layout in &config.layouts {
            valid_authored_files.insert(resolve_source_bundle_path(root_dir, &layout.module));
            if let Some(stylesheet_path) = layout.stylesheet_path.as_deref() {
                valid_authored_files.insert(resolve_source_bundle_path(root_dir, stylesheet_path));
            }
            if let Some(payload) = layout.runtime_cache_payload.as_ref()
                && let Some(graph) = decode_js_runtime_graph(payload)
            {
                for module in &graph.modules {
                    valid_authored_files.insert(resolve_source_bundle_path(root_dir, &module.specifier));
                }
            }
        }
        if let Some(global_stylesheet) = config.global_stylesheet_path.as_deref() {
            valid_authored_files.insert(resolve_source_bundle_path(root_dir, global_stylesheet));
        }

        let stale_keys = self
            .artifact_graph
            .sources()
            .filter(|key| key.kind == tilescript_core::runtime::artifact_state::ArtifactKind::AuthoredFile)
            .filter(|key| {
                let path = std::path::PathBuf::from(&key.identity);
                !sources.contains_key(&path) || !valid_authored_files.contains(&path)
            })
            .cloned()
            .collect::<Vec<_>>();
        for key in stale_keys {
            self.artifact_graph.remove(&key);
        }
    }

    fn record_layout_dependencies(
        &mut self,
        root_dir: &std::path::Path,
        sources: &SourceBundle,
        layout_key: &ArtifactKey,
        artifact: &PreparedLayout,
    ) {
        let mut dependents = stylesheet_dependency_keys(artifact);
        for authored_path in authored_layout_dependency_paths(root_dir, sources, artifact) {
            self.artifact_graph
                .replace_edges(ArtifactKey::authored_file(authored_path.to_string_lossy()), [layout_key.clone()]);
        }
        if let Some(stylesheet) = artifact.stylesheets.global.as_ref() {
            let path = resolve_source_bundle_path(root_dir, &stylesheet.path);
            if sources.contains_key(&path) {
                self.artifact_graph.replace_edges(
                    ArtifactKey::authored_file(path.to_string_lossy()),
                    [ArtifactKey::stylesheet_analysis(stylesheet.path.clone())],
                );
            }
        }
        if let Some(stylesheet) = artifact.stylesheets.layout.as_ref() {
            let path = resolve_source_bundle_path(root_dir, &stylesheet.path);
            if sources.contains_key(&path) {
                self.artifact_graph.replace_edges(
                    ArtifactKey::authored_file(path.to_string_lossy()),
                    [ArtifactKey::stylesheet_analysis(stylesheet.path.clone())],
                );
            }
        }
        match artifact.selected.runtime {
            RuntimeKind::Js => {
                if let Some(module_key) = js_browser_executable_key(artifact) {
                    let module_graph_key = ArtifactKey::js_module_graph(module_key.clone());
                    let executable_key = ArtifactKey::js_executable(module_key);
                    dependents.push(module_graph_key.clone());
                    self.artifact_graph.replace_edges(module_graph_key, [executable_key]);
                }
            }
            RuntimeKind::Lua => {
                if let Some(source) = artifact.runtime_payload.get("source").and_then(serde_json::Value::as_str) {
                    let source_module = artifact
                        .runtime_payload
                        .get("sourceModule")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or(&artifact.selected.module);
                    let executable_key = ArtifactKey::lua_executable(format!(
                        "{}::{}",
                        artifact.selected.module,
                        hash_string(source)
                    ));
                    if std::path::Path::new(source_module)
                        .extension()
                        .and_then(|ext| ext.to_str())
                        == Some("fnl")
                    {
                        let compiled_key = ArtifactKey::lua_compiled_source(format!(
                            "{}::{}",
                            artifact.selected.module,
                            hash_string(source_module)
                        ));
                        dependents.push(compiled_key.clone());
                        self.artifact_graph.replace_edges(compiled_key, [executable_key]);
                    } else {
                        dependents.push(executable_key);
                    }
                }
            }
        }

        self.artifact_graph.replace_edges(layout_key.clone(), dependents);
    }

    fn record_config_dependencies(
        &mut self,
        root_dir: &std::path::Path,
        entry_path: &std::path::Path,
        sources: &SourceBundle,
        config: &Config,
    ) {
        let config_key = ArtifactKey::config("source-bundle-config");
        let layout_keys = config
            .layouts
            .iter()
            .map(|layout| ArtifactKey::layout(layout.name.clone()))
            .collect::<Vec<_>>();
        self.artifact_graph.replace_edges(config_key.clone(), layout_keys);
        let authored_config_path = resolve_source_bundle_path(root_dir, entry_path.to_string_lossy().as_ref());
        if sources.contains_key(&authored_config_path) {
            self.artifact_graph.replace_edges(
                ArtifactKey::authored_file(authored_config_path.to_string_lossy()),
                [config_key],
            );
        }
    }
}

fn decode_js_runtime_graph(payload: &serde_json::Value) -> Option<BrowserJsRuntimeGraph> {
    serde_json::from_value(payload.clone()).ok()
}

fn config_fingerprint(config: &Config) -> Result<String, LayoutConfigError> {
    serde_json::to_string(config)
        .map(|serialized| hash_string(&serialized))
        .map_err(|error| LayoutConfigError::EvaluateAuthoredConfig {
            path: std::path::PathBuf::from("<source-bundle-config>"),
            message: error.to_string(),
        })
}

fn prepared_layout_fingerprint(layout: &PreparedLayout) -> Result<String, LayoutConfigError> {
    serde_json::to_string(layout)
        .map(|serialized| hash_string(&serialized))
        .map_err(|error| LayoutConfigError::EvaluateAuthoredConfig {
            path: std::path::PathBuf::from(&layout.selected.module),
            message: error.to_string(),
        })
}

fn evaluation_context_fingerprint(
    layout: &PreparedLayout,
    context: &LayoutEvaluationContext,
) -> Result<String, LayoutConfigError> {
    let layout_json = serde_json::to_string(layout).map_err(|error| {
        LayoutConfigError::EvaluateAuthoredConfig {
            path: std::path::PathBuf::from(&layout.selected.module),
            message: error.to_string(),
        }
    })?;
    let context_json = serde_json::to_string(context).map_err(|error| {
        LayoutConfigError::EvaluateAuthoredConfig {
            path: std::path::PathBuf::from(&layout.selected.module),
            message: error.to_string(),
        }
    })?;
    Ok(hash_string(&format!("{layout_json}|{context_json}")))
}

fn evaluated_layout_fingerprint(
    evaluated: &PreparedSourceBundleLayoutEvaluation,
) -> Result<String, LayoutConfigError> {
    let artifact = serde_json::to_string(&evaluated.artifact).map_err(|error| {
        LayoutConfigError::EvaluateAuthoredConfig {
            path: std::path::PathBuf::from(&evaluated.artifact.selected.module),
            message: error.to_string(),
        }
    })?;
    let context = serde_json::to_string(&evaluated.context).map_err(|error| {
        LayoutConfigError::EvaluateAuthoredConfig {
            path: std::path::PathBuf::from(&evaluated.artifact.selected.module),
            message: error.to_string(),
        }
    })?;
    let layout = serde_json::to_string(&evaluated.layout).map_err(|error| {
        LayoutConfigError::EvaluateAuthoredConfig {
            path: std::path::PathBuf::from(&evaluated.artifact.selected.module),
            message: error.to_string(),
        }
    })?;
    let dependencies = serde_json::to_string(&evaluated.dependencies).map_err(|error| {
        LayoutConfigError::EvaluateAuthoredConfig {
            path: std::path::PathBuf::from(&evaluated.artifact.selected.module),
            message: error.to_string(),
        }
    })?;
    Ok(hash_string(&format!("{artifact}|{context}|{layout}|{dependencies}")))
}

fn stylesheet_dependency_keys(artifact: &PreparedLayout) -> Vec<ArtifactKey> {
    let mut keys = Vec::new();
    if let Some(stylesheet) = artifact.stylesheets.global.as_ref() {
        keys.push(ArtifactKey::stylesheet_analysis(stylesheet.path.clone()));
    }
    if let Some(stylesheet) = artifact.stylesheets.layout.as_ref() {
        keys.push(ArtifactKey::stylesheet_analysis(stylesheet.path.clone()));
    }
    keys
}

fn js_browser_executable_key(artifact: &PreparedLayout) -> Option<String> {
    let entry = artifact.runtime_payload.get("entry")?.as_str()?;
    let modules = artifact.runtime_payload.get("modules")?;
    Some(hash_string(&format!("{entry}:{}", modules)))
}

fn authored_layout_dependency_paths(
    root_dir: &std::path::Path,
    sources: &SourceBundle,
    artifact: &PreparedLayout,
) -> Vec<std::path::PathBuf> {
    if artifact.selected.runtime == RuntimeKind::Js
        && let Some(graph) = decode_js_runtime_graph(&artifact.runtime_payload)
    {
        return graph
            .modules
            .iter()
            .map(|module| resolve_source_bundle_path(root_dir, &module.specifier))
            .filter(|path| sources.contains_key(path))
            .collect();
    }

    let source_module = artifact
        .runtime_payload
        .get("sourceModule")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(&artifact.selected.module);
    let authored_module_path = resolve_source_bundle_path(root_dir, source_module);
    sources.contains_key(&authored_module_path).then_some(authored_module_path).into_iter().collect()
}

fn hash_string(value: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn resolve_source_bundle_path(root_dir: &std::path::Path, path: &str) -> std::path::PathBuf {
    let path_buf = std::path::PathBuf::from(path);
    if path_buf.is_absolute() {
        path_buf
    } else {
        root_dir.join(path_buf)
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
struct BrowserJsRuntimeGraph {
    modules: Vec<BrowserJsRuntimeModule>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct BrowserJsRuntimeModule {
    specifier: String,
}
