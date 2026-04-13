use hypreact_config::model::Config;
use hypreact_config::runtime::AuthoringConfigRuntime;
use hypreact_core::runtime::layout_context::LayoutEvaluationContext;
use hypreact_core::runtime::prepared_layout::{PreparedLayout, SelectedLayout};
use hypreact_core::runtime::runtime_contract::{LayoutModuleContract, PreparedLayoutRuntime};
use hypreact_core::runtime::runtime_error::RuntimeError;
use hypreact_core::SourceLayoutNode;
use hypreact_scene::ast::LayoutValidationError;
use tracing::{debug, warn};

use crate::loader::{InlineLayoutSourceLoader, JsLayoutSourceLoader};
use crate::module_graph_runtime::call_entry_export_with_json_arg;
use crate::{
    decode_js_layout_value, decode_runtime_graph_payload, encode_runtime_graph_payload,
    JavaScriptModule, JavaScriptModuleGraph,
};

#[cfg(test)]
use crate::loader::FsLayoutSourceLoader;

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
}

impl Default for QuickJsPreparedLayoutRuntime<InlineLayoutSourceLoader> {
    fn default() -> Self {
        Self {
            contract: LayoutModuleContract::default(),
            loader: InlineLayoutSourceLoader,
        }
    }
}

impl QuickJsPreparedLayoutRuntime<InlineLayoutSourceLoader> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<L> QuickJsPreparedLayoutRuntime<L> {
    pub fn with_loader(loader: L) -> Self {
        Self {
            contract: LayoutModuleContract::default(),
            loader,
        }
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
            PreparedLayoutRuntimeError::JavaScript {
                message: error.to_string(),
            }
        })?;

        let json = call_entry_export_with_json_arg(
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
            .map_err(|error| RuntimeError::Config {
                message: error.to_string(),
            })?
            .map(|selected| PreparedLayout {
                selected,
                runtime_payload: encode_runtime_graph_payload(&JavaScriptModuleGraph {
                    entry: String::new(),
                    modules: Vec::new(),
                }),
                stylesheets: hypreact_core::runtime::prepared_layout::PreparedStylesheets::default(
                ),
            }))
    }

    fn build_context(
        &self,
        state: &hypreact_core::snapshot::StateSnapshot,
        workspace: &hypreact_core::snapshot::WorkspaceSnapshot,
        artifact: Option<&PreparedLayout>,
    ) -> LayoutEvaluationContext {
        state.layout_context(
            workspace,
            artifact.map(|artifact| artifact.selected.clone()),
        )
    }

    fn evaluate_layout(
        &self,
        loaded_layout: &PreparedLayout,
        _context: &LayoutEvaluationContext,
    ) -> Result<SourceLayoutNode, RuntimeError> {
        Err(RuntimeError::NotImplemented(format!(
            "layout {}",
            loaded_layout.selected.name
        )))
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
        state.layout_context(
            workspace,
            artifact.map(|artifact| artifact.selected.clone()),
        )
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

        result.map_err(|error| RuntimeError::Other {
            message: error.to_string(),
        })
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
        result.map_err(|error| RuntimeError::Config {
            message: error.to_string(),
        })
    }

    fn load_prepared_config(&self, path: &std::path::Path) -> Result<Config, RuntimeError> {
        debug!(path = %path.display(), "loading prepared config");
        let result = crate::authored::load_prepared_config(path);
        if let Err(error) = &result {
            warn!(path = %path.display(), %error, "failed loading prepared config");
        }
        result.map_err(|error| RuntimeError::Config {
            message: error.to_string(),
        })
    }

    fn refresh_prepared_config(
        &self,
        authored: &std::path::Path,
        runtime: &std::path::Path,
    ) -> Result<hypreact_core::runtime::runtime_error::RuntimeRefreshSummary, RuntimeError> {
        debug!(authored = %authored.display(), runtime = %runtime.display(), "refreshing prepared config");
        crate::authored::refresh_prepared_config(authored, runtime)
            .map(runtime_refresh_summary)
            .map_err(|error| RuntimeError::Config {
                message: error.to_string(),
            })
    }

    fn rebuild_prepared_config(
        &self,
        authored: &std::path::Path,
        runtime: &std::path::Path,
    ) -> Result<hypreact_core::runtime::runtime_error::RuntimeRefreshSummary, RuntimeError> {
        debug!(authored = %authored.display(), runtime = %runtime.display(), "rebuilding prepared config");
        crate::authored::rebuild_prepared_config(authored, runtime)
            .map(runtime_refresh_summary)
            .map_err(|error| RuntimeError::Config {
                message: error.to_string(),
            })
    }
}

impl AuthoringConfigRuntime for StubPreparedLayoutRuntime {
    fn load_authored_config(&self, _path: &std::path::Path) -> Result<Config, RuntimeError> {
        Err(RuntimeError::NotImplemented(
            "authored config loading".into(),
        ))
    }

    fn load_prepared_config(&self, _path: &std::path::Path) -> Result<Config, RuntimeError> {
        Err(RuntimeError::NotImplemented(
            "runtime config loading".into(),
        ))
    }

    fn refresh_prepared_config(
        &self,
        _authored: &std::path::Path,
        _runtime: &std::path::Path,
    ) -> Result<hypreact_core::runtime::runtime_error::RuntimeRefreshSummary, RuntimeError> {
        Err(RuntimeError::NotImplemented(
            "prepared config refresh".into(),
        ))
    }

    fn rebuild_prepared_config(
        &self,
        _authored: &std::path::Path,
        _runtime: &std::path::Path,
    ) -> Result<hypreact_core::runtime::runtime_error::RuntimeRefreshSummary, RuntimeError> {
        Err(RuntimeError::NotImplemented(
            "prepared config rebuild".into(),
        ))
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;

    use crate::{decode_runtime_graph_payload, JavaScriptModule, JavaScriptModuleGraph};
    use hypreact_config::model::{Config, LayoutDefinition};
    use hypreact_core::snapshot::{OutputSnapshot, StateSnapshot, WorkspaceSnapshot};
    use hypreact_core::types::LayoutRef;
    use hypreact_core::{OutputId, SlotTake, WorkspaceId};
    use serde_json::json;

    use super::*;
    use crate::authored::{load_prepared_config, rebuild_prepared_config};
    use crate::module_graph_runtime::call_entry_export_with_json_arg;

    fn workspace() -> WorkspaceSnapshot {
        WorkspaceSnapshot {
            id: WorkspaceId::from("ws-1"),
            name: "1".into(),
            output_id: Some(OutputId::from("out-1")),
            layout_space: None,
            active_workspaces: vec!["1".into()],
            focused: true,
            visible: true,
            effective_layout: Some(LayoutRef {
                name: "master-stack".into(),
            }),
        }
    }

    fn state() -> StateSnapshot {
        StateSnapshot {
            focused_window_id: None,
            current_output_id: Some(OutputId::from("out-1")),
            current_workspace_id: Some(WorkspaceId::from("ws-1")),
            outputs: vec![OutputSnapshot {
                id: OutputId::from("out-1"),
                name: "HDMI-A-1".into(),
                logical_width: 1920,
                logical_height: 1080,
                scale: 1,
                enabled: true,
                current_workspace_id: Some(WorkspaceId::from("ws-1")),
            }],
            workspaces: vec![workspace()],
            windows: vec![],
            visible_window_ids: vec![],
            workspace_names: vec!["1".into()],
            resize_state: hypreact_core::resize::ResizeState::default(),
        }
    }

    #[test]
    fn quickjs_runtime_exposes_default_export_contract() {
        let runtime = QuickJsPreparedLayoutRuntime::new();
        assert_eq!(runtime.contract().export_name, "default");
    }

    #[test]
    fn quickjs_runtime_decodes_js_layout_object_into_normalized_tree() {
        let runtime = QuickJsPreparedLayoutRuntime::new();
        let layout = runtime
            .evaluate_module_source(
                &SelectedLayout {
                    name: "master-stack".into(),
                    directory: "layouts/master-stack".into(),
                    module: "layouts/master-stack.js".into(),
                },
                &state().layout_context(&workspace(), None),
                "ctx => ({ type: 'workspace', children: [{ type: 'window', match: 'app_id=\"firefox\"' }] })",
            )
            .unwrap();

        assert!(matches!(layout, SourceLayoutNode::Workspace { .. }));
    }

    #[test]
    fn decode_authored_layout_node_preserves_jsx_props_metadata() {
        let value = json!({
            "type": "workspace",
            "props": { "id": "root" },
            "children": [{
                "type": "group",
                "props": { "id": "frame" },
                "children": [{
                    "type": "slot",
                    "props": { "id": "master", "class": "master-slot", "take": 1 },
                    "children": []
                }]
            }]
        });

        let decoded = decode_js_layout_value(&value).unwrap();

        let SourceLayoutNode::Workspace { meta, children } = decoded else {
            panic!("expected workspace root");
        };
        assert_eq!(meta.id.as_deref(), Some("root"));

        let SourceLayoutNode::Group {
            meta: group_meta,
            children: group_children,
        } = &children[0]
        else {
            panic!("expected frame group");
        };
        assert_eq!(group_meta.id.as_deref(), Some("frame"));

        let SourceLayoutNode::Slot { meta, take, .. } = &group_children[0] else {
            panic!("expected master slot");
        };
        assert_eq!(meta.id.as_deref(), Some("master"));
        assert_eq!(meta.class, vec!["master-slot".to_owned()]);
        assert_eq!(*take, SlotTake::Count(1));
    }

    #[test]
    fn quickjs_authoring_layout_service_works_with_filesystem_loader() {
        let temp_dir = std::env::temp_dir();
        let module_path = temp_dir.join("spiders-runtime-service-test.js");
        fs::write(
            &module_path,
            "ctx => ({ type: 'workspace', children: [{ type: 'window', id: 'main' }] })",
        )
        .unwrap();

        let runtime = QuickJsPreparedLayoutRuntime::with_loader(FsLayoutSourceLoader);
        let config = Config {
            layouts: vec![LayoutDefinition {
                name: "master-stack".into(),
                directory: "layouts/master-stack".into(),
                module: module_path.to_string_lossy().into_owned(),
                stylesheet_path: Some("layouts/master-stack/index.css".into()),
                runtime_cache_payload: None,
            }],
            ..Config::default()
        };

        let loaded = runtime
            .prepare_layout(&config, &workspace())
            .unwrap()
            .unwrap();
        let layout = runtime
            .evaluate_layout(
                &loaded,
                &state().layout_context(&workspace(), Some(loaded.selected.clone())),
            )
            .unwrap();

        assert_eq!(loaded.selected.name, "master-stack");
        assert!(matches!(layout, SourceLayoutNode::Workspace { .. }));

        let _ = fs::remove_file(module_path);
    }

    #[test]
    fn module_graph_runtime_loads_sdk_jsx_runtime_virtual_module() {
        let result = call_entry_export_with_json_arg(
            &JavaScriptModuleGraph {
                entry: "layouts/master-stack/index.js".into(),
                modules: vec![
                    JavaScriptModule {
                        specifier: "layouts/master-stack/index.js".into(),
                        source: "import { sp } from \"@hypreact/sdk/jsx-runtime\"; export default function layout(ctx) { return sp(\"workspace\", { id: \"root\" }); }".into(),
                        resolved_imports: BTreeMap::from([(
                            "@hypreact/sdk/jsx-runtime".into(),
                            "@hypreact/sdk/jsx-runtime".into(),
                        )]),
                    },
                    JavaScriptModule {
                        specifier: "@hypreact/sdk/jsx-runtime".into(),
                        source: include_str!("../../../../packages/sdk/js/src/jsx-runtime.js")
                            .into(),
                        resolved_imports: BTreeMap::new(),
                    },
                ],
            },
            "layouts/master-stack/index.js",
            "default",
            &json!({}),
        );

        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn prepared_test_config_master_stack_layout_evaluates() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let authored_config = repo_root.join("test_config/test_config/config.ts");
        let runtime_root = std::env::temp_dir().join(format!(
            "hypreact-runtime-genymotion-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let runtime_entry = runtime_root.join("config.js");

        fs::create_dir_all(&runtime_root).unwrap();
        rebuild_prepared_config(&authored_config, &runtime_entry).unwrap();
        let config = load_prepared_config(&runtime_entry).unwrap();
        let runtime = QuickJsPreparedLayoutRuntime::with_loader(FsLayoutSourceLoader);
        let workspace = WorkspaceSnapshot {
            effective_layout: Some(LayoutRef {
                name: "master-stack".into(),
            }),
            ..workspace()
        };

        let loaded = runtime
            .prepare_layout(&config, &workspace)
            .unwrap()
            .unwrap();
        let graph = decode_runtime_graph_payload(&loaded.runtime_payload).unwrap();

        assert!(graph
            .modules
            .iter()
            .any(|module| { module.specifier == "@hypreact/sdk/jsx-runtime" }));

        let layout = runtime.evaluate_layout(
            &loaded,
            &state().layout_context(&workspace, Some(loaded.selected.clone())),
        );

        assert!(layout.is_ok(), "{layout:?}");
    }

    #[test]
    fn checked_in_prepared_test_config_master_stack_layout_evaluates() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let authored_config = repo_root.join("test_config/test_config/config.ts");
        let runtime_root = std::env::temp_dir().join(format!(
            "hypreact-runtime-genymotion-checked-in-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let prepared_config = runtime_root.join("config.js");
        fs::create_dir_all(&runtime_root).unwrap();
        rebuild_prepared_config(&authored_config, &prepared_config).unwrap();
        let config = load_prepared_config(&prepared_config).unwrap();
        let runtime = QuickJsPreparedLayoutRuntime::with_loader(FsLayoutSourceLoader);
        let workspace = WorkspaceSnapshot {
            effective_layout: Some(LayoutRef {
                name: "master-stack".into(),
            }),
            ..workspace()
        };

        let loaded = runtime
            .prepare_layout(&config, &workspace)
            .unwrap()
            .unwrap();
        let graph = decode_runtime_graph_payload(&loaded.runtime_payload).unwrap();

        assert!(graph
            .modules
            .iter()
            .any(|module| { module.specifier == "@hypreact/sdk/jsx-runtime" }));

        let layout = runtime.evaluate_layout(
            &loaded,
            &state().layout_context(&workspace, Some(loaded.selected.clone())),
        );

        assert!(layout.is_ok(), "{layout:?}");
    }

    #[test]
    fn prepared_template_config_loads() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let authored_config = repo_root.join("test_config/test_config/config.ts");
        let runtime_root = std::env::temp_dir().join(format!(
            "hypreact-runtime-template-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let runtime_entry = runtime_root.join("config.js");

        fs::create_dir_all(&runtime_root).unwrap();
        rebuild_prepared_config(&authored_config, &runtime_entry).unwrap();

        let config = load_prepared_config(&runtime_entry);

        assert!(config.is_ok(), "{config:?}");
    }
}
