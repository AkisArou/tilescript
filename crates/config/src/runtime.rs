use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use tilescript_core::SourceLayoutNode;
use tilescript_core::runtime::layout_context::{
    LayoutEvaluationContext, LayoutEvaluationDependencies,
};
use tilescript_core::runtime::prepared_layout::PreparedLayout;
use tilescript_core::runtime::runtime_contract::PreparedLayoutRuntime;
use tilescript_core::runtime::runtime_error::{RuntimeError, RuntimeRefreshSummary};
use tilescript_core::snapshot::{StateSnapshot, WorkspaceSnapshot};

use crate::authoring_layout::{AuthoringLayoutService, SourceBundleAuthoringLayoutService};
use crate::model::{Config, ConfigPaths, LayoutConfigError};

#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedSourceLayout {
    pub layout: SourceLayoutNode,
    pub dependencies: LayoutEvaluationDependencies,
}

pub type SourceBundle = BTreeMap<PathBuf, String>;

pub trait AuthoringConfigRuntime: std::fmt::Debug {
    fn load_authored_config(&self, path: &Path) -> Result<Config, RuntimeError>;
    fn load_prepared_config(&self, path: &Path) -> Result<Config, RuntimeError>;
    fn refresh_prepared_config(
        &self,
        authored: &Path,
        prepared: &Path,
    ) -> Result<RuntimeRefreshSummary, RuntimeError>;
    fn rebuild_prepared_config(
        &self,
        authored: &Path,
        prepared: &Path,
    ) -> Result<RuntimeRefreshSummary, RuntimeError>;
}

#[derive(Debug)]
pub struct RuntimeBundle {
    pub config_runtime: Box<dyn AuthoringConfigRuntime>,
    pub layout_runtime: Box<dyn PreparedLayoutRuntime<Config = Config>>,
}

pub trait SourceBundleConfigRuntime: std::fmt::Debug {
    fn load_config<'a>(
        &'a self,
        root_dir: &'a Path,
        entry_path: &'a Path,
        sources: &'a SourceBundle,
    ) -> Pin<Box<dyn Future<Output = Result<Config, LayoutConfigError>> + 'a>>;
}

pub trait SourceBundlePreparedLayoutRuntime: std::fmt::Debug {
    fn prepare_layout<'a>(
        &'a self,
        root_dir: &'a Path,
        sources: &'a SourceBundle,
        config: &'a Config,
        workspace: &'a WorkspaceSnapshot,
    ) -> Pin<Box<dyn Future<Output = Result<Option<PreparedLayout>, LayoutConfigError>> + 'a>>;

    fn build_context(
        &self,
        state: &StateSnapshot,
        workspace: &WorkspaceSnapshot,
        artifact: Option<&PreparedLayout>,
    ) -> LayoutEvaluationContext;

    fn evaluate_layout<'a>(
        &'a self,
        root_dir: &'a Path,
        sources: &'a SourceBundle,
        artifact: &'a PreparedLayout,
        context: &'a LayoutEvaluationContext,
    ) -> Pin<Box<dyn Future<Output = Result<EvaluatedSourceLayout, LayoutConfigError>> + 'a>>;
}

#[derive(Debug)]
pub struct SourceBundleRuntimeBundle {
    pub config_runtime: Box<dyn SourceBundleConfigRuntime>,
    pub layout_runtime: Box<dyn SourceBundlePreparedLayoutRuntime>,
}

pub fn build_authoring_layout_service(
    paths: &ConfigPaths,
    bundle: RuntimeBundle,
) -> Result<AuthoringLayoutService, LayoutConfigError> {
    Ok(AuthoringLayoutService::from_runtime_bundle(
        bundle.config_runtime,
        bundle.layout_runtime,
        paths.clone(),
    ))
}

pub async fn load_config_from_source_bundle(
    root_dir: &Path,
    entry_path: &Path,
    sources: &SourceBundle,
    bundle: SourceBundleRuntimeBundle,
) -> Result<Config, LayoutConfigError> {
    let _ = entry_path;
    bundle.config_runtime.load_config(root_dir, entry_path, sources).await
}

pub fn build_source_bundle_authoring_layout_service(
    _entry_path: &Path,
    bundle: SourceBundleRuntimeBundle,
) -> Result<SourceBundleAuthoringLayoutService, LayoutConfigError> {
    Ok(SourceBundleAuthoringLayoutService::from_runtime_bundle(
        bundle.config_runtime,
        bundle.layout_runtime,
    ))
}
