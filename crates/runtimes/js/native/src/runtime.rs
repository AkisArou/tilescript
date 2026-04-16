use std::sync::{Arc, Mutex};

use hypreact_config::model::Config;
use hypreact_config::runtime::AuthoringConfigRuntime;
use hypreact_core::SourceLayoutNode;
use hypreact_core::runtime::layout_context::LayoutEvaluationContext;
use hypreact_core::runtime::prepared_layout::{PreparedLayout, SelectedLayout};
use hypreact_core::runtime::runtime_contract::{LayoutModuleContract, PreparedLayoutRuntime};
use hypreact_core::runtime::runtime_error::RuntimeError;
use hypreact_scene::ast::LayoutValidationError;
use tracing::{debug, warn};

use hypreact_runtime_js_core::loader::{InlineLayoutSourceLoader, JsLayoutSourceLoader};
use hypreact_runtime_js_core::{
    JavaScriptModule, JavaScriptModuleGraph, decode_js_layout_value, decode_runtime_graph_payload,
    encode_runtime_graph_payload,
};

use crate::module_graph_runtime::{QuickJsExecutionCache, module_graph_execution_key};

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum PreparedLayoutRuntimeError {
    #[error("layout `{name}` evaluation is not implemented yet")]
    NotImplemented { name: String },
    #[error(transparent)]
    Validation(#[from] LayoutValidationError),
    #[error("javascript evaluation failed: {message}")]
    JavaScript { message: String },
    #[error("layout module `{name}` did not provide `{export}` export")]
    MissingExport { name: String, export: String },
    #[error("layout module `{name}` export `{export}` is not callable")]
    NonCallableExport { name: String, export: String },
    #[error("js to layout conversion failed for layout `{name}`: {message}")]
    ValueConversion { name: String, message: String },
}

#[derive(Debug, Default, Clone, Copy)]
pub struct StubPreparedLayoutRuntime;

#[derive(Debug, Clone)]
pub struct QuickJsPreparedLayoutRuntime<L = InlineLayoutSourceLoader> {
    contract: LayoutModuleContract,
    loader: L,
    execution_cache: Arc<Mutex<QuickJsExecutionCache>>,
}

impl Default for QuickJsPreparedLayoutRuntime<InlineLayoutSourceLoader> {
    fn default() -> Self {
        Self {
            contract: LayoutModuleContract::default(),
            loader: InlineLayoutSourceLoader,
            execution_cache: Arc::new(Mutex::new(QuickJsExecutionCache::new(None))),
        }
    }
}

impl QuickJsPreparedLayoutRuntime<InlineLayoutSourceLoader> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<L> QuickJsPreparedLayoutRuntime<L> {
    pub fn with_loader_and_bytecode_root(
        loader: L,
        bytecode_root: Option<std::path::PathBuf>,
    ) -> Self {
        Self {
            contract: LayoutModuleContract::default(),
            loader,
            execution_cache: Arc::new(Mutex::new(QuickJsExecutionCache::new(bytecode_root))),
        }
    }

    pub fn with_loader(loader: L) -> Self {
        Self::with_loader_and_bytecode_root(loader, None)
    }

    fn reset_execution_cache(&self) -> Result<(), RuntimeError> {
        self.execution_cache
            .lock()
            .map_err(|_| RuntimeError::Other {
                message: "js execution cache mutex is poisoned".into(),
            })?
            .reset();
        Ok(())
    }

    pub fn evaluate_module_source(
        &self,
        selected_layout: &SelectedLayout,
        context: &LayoutEvaluationContext,
        source: &str,
    ) -> Result<SourceLayoutNode, PreparedLayoutRuntimeError> {
        self.evaluate_module_graph(
            selected_layout,
            context,
            &JavaScriptModuleGraph {
                entry: selected_layout.module.clone(),
                modules: vec![JavaScriptModule {
                    specifier: selected_layout.module.clone(),
                    source: format!("export default ({source});"),
                    resolved_imports: Default::default(),
                }],
            },
        )
    }

    fn evaluate_module_graph(
        &self,
        selected_layout: &SelectedLayout,
        context: &LayoutEvaluationContext,
        graph: &JavaScriptModuleGraph,
    ) -> Result<SourceLayoutNode, PreparedLayoutRuntimeError> {
        let context_value = serde_json::to_value(context).map_err(|error| {
            PreparedLayoutRuntimeError::JavaScript { message: error.to_string() }
        })?;

        let graph_key = module_graph_execution_key(graph);
        let json = self
            .execution_cache
            .lock()
            .map_err(|_| PreparedLayoutRuntimeError::JavaScript {
                message: "js execution cache mutex is poisoned".into(),
            })?
            .call_entry_export_with_json_arg(
                &graph_key,
                graph,
                &selected_layout.module,
                &self.contract.export_name,
                &context_value,
            )
            .map_err(|error| match error {
                crate::module_graph_runtime::ModuleGraphRuntimeError::JavaScript { message } => {
                    PreparedLayoutRuntimeError::JavaScript { message }
                }
                crate::module_graph_runtime::ModuleGraphRuntimeError::MissingExport {
                    name,
                    export,
                } => PreparedLayoutRuntimeError::MissingExport { name, export },
                crate::module_graph_runtime::ModuleGraphRuntimeError::NonCallableExport {
                    name,
                    export,
                } => PreparedLayoutRuntimeError::NonCallableExport { name, export },
            })?
            .ok_or_else(|| PreparedLayoutRuntimeError::ValueConversion {
                name: selected_layout.name.clone(),
                message: "layout function returned undefined".into(),
            })?;

        decode_js_layout_value(&json).map_err(|message| {
            PreparedLayoutRuntimeError::ValueConversion {
                name: selected_layout.name.clone(),
                message,
            }
        })
    }
}

impl<L: JsLayoutSourceLoader> QuickJsPreparedLayoutRuntime<L> {
    pub fn prepare_layout(
        &self,
        config: &Config,
        workspace: &hypreact_core::snapshot::WorkspaceSnapshot,
    ) -> Result<Option<PreparedLayout>, RuntimeError> {
        debug!(workspace_id = %workspace.id, workspace_name = %workspace.name, "loading runtime source for layout preparation");
        let result = self.loader.load_runtime_source(config, workspace);
        if let Err(error) = &result {
            warn!(
                %error,
                workspace_id = %workspace.id,
                workspace_name = %workspace.name,
                "failed to load runtime source for workspace"
            );
        }
        result
    }
}

impl PreparedLayoutRuntime for StubPreparedLayoutRuntime {
    type Config = Config;

    fn prepare_layout(
        &self,
        config: &Self::Config,
        workspace: &hypreact_core::snapshot::WorkspaceSnapshot,
    ) -> Result<Option<PreparedLayout>, RuntimeError> {
        Ok(config
            .resolve_selected_layout(workspace)
            .map_err(|error| RuntimeError::Config { message: error.to_string() })?
            .map(|selected| PreparedLayout {
                selected,
                runtime_payload: encode_runtime_graph_payload(
                    &JavaScriptModuleGraph { entry: String::new(), modules: Vec::new() },
                    &[],
                ),
                stylesheets: hypreact_core::runtime::prepared_layout::PreparedStylesheets::default(
                ),
                dependencies: vec![],
            }))
    }

    fn build_context(
        &self,
        state: &hypreact_core::snapshot::StateSnapshot,
        workspace: &hypreact_core::snapshot::WorkspaceSnapshot,
        artifact: Option<&PreparedLayout>,
    ) -> LayoutEvaluationContext {
        state.layout_context(workspace, artifact.map(|artifact| artifact.selected.clone()))
    }

    fn evaluate_layout(
        &self,
        loaded_layout: &PreparedLayout,
        _context: &LayoutEvaluationContext,
    ) -> Result<SourceLayoutNode, RuntimeError> {
        Err(RuntimeError::NotImplemented(format!("layout {}", loaded_layout.selected.name)))
    }

    fn contract(&self) -> LayoutModuleContract {
        LayoutModuleContract::default()
    }
}

impl<L: JsLayoutSourceLoader> PreparedLayoutRuntime for QuickJsPreparedLayoutRuntime<L> {
    type Config = Config;

    fn prepare_layout(
        &self,
        config: &Self::Config,
        workspace: &hypreact_core::snapshot::WorkspaceSnapshot,
    ) -> Result<Option<PreparedLayout>, RuntimeError> {
        QuickJsPreparedLayoutRuntime::prepare_layout(self, config, workspace)
    }

    fn build_context(
        &self,
        state: &hypreact_core::snapshot::StateSnapshot,
        workspace: &hypreact_core::snapshot::WorkspaceSnapshot,
        artifact: Option<&PreparedLayout>,
    ) -> LayoutEvaluationContext {
        state.layout_context(workspace, artifact.map(|artifact| artifact.selected.clone()))
    }

    fn evaluate_layout(
        &self,
        loaded_layout: &PreparedLayout,
        context: &LayoutEvaluationContext,
    ) -> Result<SourceLayoutNode, RuntimeError> {
        debug!(layout = %loaded_layout.selected.name, module = %loaded_layout.selected.module, "evaluating prepared layout module graph");
        let runtime_graph = decode_runtime_graph_payload(&loaded_layout.runtime_payload)?;
        let result = self.evaluate_module_graph(&loaded_layout.selected, context, &runtime_graph);

        if let Err(error) = &result {
            warn!(layout = %loaded_layout.selected.name, module = %loaded_layout.selected.module, %error, "layout evaluation failed");
        }

        result.map_err(|error| RuntimeError::Other { message: error.to_string() })
    }

    fn contract(&self) -> LayoutModuleContract {
        self.contract.clone()
    }
}

impl<L: JsLayoutSourceLoader> AuthoringConfigRuntime for QuickJsPreparedLayoutRuntime<L> {
    fn load_authored_config(&self, path: &std::path::Path) -> Result<Config, RuntimeError> {
        debug!(path = %path.display(), "loading authored config");
        let result = crate::authored::load_authored_config(path);
        if let Err(error) = &result {
            warn!(path = %path.display(), %error, "failed loading authored config");
        }
        result.map_err(|error| RuntimeError::Config { message: error.to_string() })
    }

    fn load_prepared_config(&self, path: &std::path::Path) -> Result<Config, RuntimeError> {
        debug!(path = %path.display(), "loading prepared config");
        let result = crate::authored::load_prepared_config(path);
        if let Err(error) = &result {
            warn!(path = %path.display(), %error, "failed loading prepared config");
        }
        result.map_err(|error| RuntimeError::Config { message: error.to_string() })
    }

    fn refresh_prepared_config(
        &self,
        authored: &std::path::Path,
        runtime: &std::path::Path,
    ) -> Result<hypreact_core::runtime::runtime_error::RuntimeRefreshSummary, RuntimeError> {
        debug!(authored = %authored.display(), runtime = %runtime.display(), "refreshing prepared config");
        let result = crate::authored::refresh_prepared_config(authored, runtime)
            .map(runtime_refresh_summary)
            .map_err(|error| RuntimeError::Config { message: error.to_string() });
        if result.is_ok() {
            self.reset_execution_cache()?;
        }
        result
    }

    fn rebuild_prepared_config(
        &self,
        authored: &std::path::Path,
        runtime: &std::path::Path,
    ) -> Result<hypreact_core::runtime::runtime_error::RuntimeRefreshSummary, RuntimeError> {
        debug!(authored = %authored.display(), runtime = %runtime.display(), "rebuilding prepared config");
        let result = crate::authored::rebuild_prepared_config(authored, runtime)
            .map(runtime_refresh_summary)
            .map_err(|error| RuntimeError::Config { message: error.to_string() });
        if result.is_ok() {
            self.reset_execution_cache()?;
        }
        result
    }
}

impl AuthoringConfigRuntime for StubPreparedLayoutRuntime {
    fn load_authored_config(&self, _path: &std::path::Path) -> Result<Config, RuntimeError> {
        Err(RuntimeError::NotImplemented("authored config loading".into()))
    }

    fn load_prepared_config(&self, _path: &std::path::Path) -> Result<Config, RuntimeError> {
        Err(RuntimeError::NotImplemented("runtime config loading".into()))
    }

    fn refresh_prepared_config(
        &self,
        _authored: &std::path::Path,
        _runtime: &std::path::Path,
    ) -> Result<hypreact_core::runtime::runtime_error::RuntimeRefreshSummary, RuntimeError> {
        Err(RuntimeError::NotImplemented("prepared config refresh".into()))
    }

    fn rebuild_prepared_config(
        &self,
        _authored: &std::path::Path,
        _runtime: &std::path::Path,
    ) -> Result<hypreact_core::runtime::runtime_error::RuntimeRefreshSummary, RuntimeError> {
        Err(RuntimeError::NotImplemented("prepared config rebuild".into()))
    }
}

fn runtime_refresh_summary(
    update: crate::authored::JsRuntimeCacheUpdate,
) -> hypreact_core::runtime::runtime_error::RuntimeRefreshSummary {
    hypreact_core::runtime::runtime_error::RuntimeRefreshSummary {
        refreshed_files: update.rebuilt_files + update.copied_stylesheets,
        pruned_files: update.pruned_files,
    }
}
