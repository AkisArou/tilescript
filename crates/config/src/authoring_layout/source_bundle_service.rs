use std::collections::BTreeMap;

use hypreact_core::SourceLayoutNode;
use hypreact_core::runtime::layout_context::{
    LayoutEvaluationContext, LayoutEvaluationDependencies,
};
use hypreact_core::runtime::prepared_layout::PreparedLayout;
use hypreact_core::snapshot::{StateSnapshot, WorkspaceSnapshot};

use crate::model::{Config, LayoutConfigError};
use crate::runtime::{SourceBundle, SourceBundleConfigRuntime, SourceBundlePreparedLayoutRuntime};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct EvaluationCacheKey {
    layout_name: String,
    artifact_key: String,
    context_key: String,
}

#[derive(Debug)]
pub struct SourceBundleAuthoringLayoutService {
    config_runtime: Box<dyn SourceBundleConfigRuntime>,
    layout_runtime: Box<dyn SourceBundlePreparedLayoutRuntime>,
    cache: BTreeMap<String, PreparedLayout>,
    evaluation_cache: BTreeMap<EvaluationCacheKey, PreparedSourceBundleLayoutEvaluation>,
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
            cache: BTreeMap::new(),
            evaluation_cache: BTreeMap::new(),
        }
    }

    pub async fn load_config(
        &self,
        root_dir: &std::path::Path,
        entry_path: &std::path::Path,
        sources: &SourceBundle,
    ) -> Result<Config, LayoutConfigError> {
        self.config_runtime.load_config(root_dir, entry_path, sources).await
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

        let key = loaded.selected.name.clone();
        self.cache.insert(key.clone(), loaded);
        Ok(self.cache.get(&key))
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
        let cache_key = EvaluationCacheKey {
            layout_name: loaded.selected.name.clone(),
            artifact_key: serde_json::to_string(&loaded).map_err(|error| {
                LayoutConfigError::EvaluateAuthoredConfig {
                    path: root_dir.join(&loaded.selected.module),
                    message: error.to_string(),
                }
            })?,
            context_key: serde_json::to_string(&context).map_err(|error| {
                LayoutConfigError::EvaluateAuthoredConfig {
                    path: root_dir.join(&loaded.selected.module),
                    message: error.to_string(),
                }
            })?,
        };

        if let Some(cached) = self.evaluation_cache.get(&cache_key) {
            return Ok(Some(cached.clone()));
        }

        let evaluated =
            self.layout_runtime.evaluate_layout(root_dir, sources, &loaded, &context).await?;

        let result = PreparedSourceBundleLayoutEvaluation {
            artifact: loaded,
            context,
            layout: evaluated.layout,
            dependencies: evaluated.dependencies,
        };
        self.evaluation_cache.insert(cache_key, result.clone());

        Ok(Some(result))
    }
}
