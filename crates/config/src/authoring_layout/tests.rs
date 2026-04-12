use std::fs;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use hypreact_core::runtime::layout_context::LayoutEvaluationContext;
use hypreact_core::runtime::prepared_layout::{PreparedLayout, SelectedLayout};
use hypreact_core::runtime::runtime_contract::{LayoutModuleContract, PreparedLayoutRuntime};
use hypreact_core::runtime::runtime_error::{RuntimeError, RuntimeRefreshSummary};
use hypreact_core::snapshot::{OutputSnapshot, StateSnapshot, WorkspaceSnapshot};
use hypreact_core::types::LayoutRef;
use hypreact_core::{OutputId, SourceLayoutNode, WorkspaceId};
use tempfile::TempDir;

use super::*;
use crate::model::{Config, ConfigDiscoveryOptions, ConfigPaths, LayoutDefinition};
use crate::runtime::{
    AuthoringConfigRuntime, EvaluatedSourceLayout, SourceBundle, SourceBundleConfigRuntime,
    SourceBundlePreparedLayoutRuntime,
};

#[derive(Debug, Clone)]
struct StubRuntime {
    loaded: Option<PreparedLayout>,
    error_message: Option<String>,
}

impl PreparedLayoutRuntime for StubRuntime {
    type Config = Config;

    fn prepare_layout(
        &self,
        _config: &Self::Config,
        _workspace: &WorkspaceSnapshot,
    ) -> Result<Option<PreparedLayout>, RuntimeError> {
        if let Some(message) = &self.error_message {
            return Err(RuntimeError::Other { message: message.clone() });
        }

        Ok(self.loaded.clone())
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
        _prepared_layout: &PreparedLayout,
        _context: &LayoutEvaluationContext,
    ) -> Result<SourceLayoutNode, RuntimeError> {
        Ok(SourceLayoutNode::Workspace { meta: Default::default(), children: vec![] })
    }

    fn contract(&self) -> LayoutModuleContract {
        LayoutModuleContract::default()
    }
}

impl AuthoringConfigRuntime for StubRuntime {
    fn load_authored_config(&self, _path: &std::path::Path) -> Result<Config, RuntimeError> {
        Err(RuntimeError::NotImplemented("authored config loading".into()))
    }

    fn load_prepared_config(&self, _path: &std::path::Path) -> Result<Config, RuntimeError> {
        Ok(Config {
            layouts: vec![LayoutDefinition {
                name: "master-stack".into(),
                directory: "layouts/master-stack".into(),
                module: "layouts/master-stack.js".into(),
                stylesheet_path: Some("layouts/master-stack/index.css".into()),
                runtime_cache_payload: None,
            }],
            ..Config::default()
        })
    }

    fn refresh_prepared_config(
        &self,
        _authored: &std::path::Path,
        _runtime: &std::path::Path,
    ) -> Result<RuntimeRefreshSummary, RuntimeError> {
        Ok(RuntimeRefreshSummary::default())
    }

    fn rebuild_prepared_config(
        &self,
        _authored: &std::path::Path,
        _runtime: &std::path::Path,
    ) -> Result<RuntimeRefreshSummary, RuntimeError> {
        Err(RuntimeError::NotImplemented("prepared config rebuild".into()))
    }
}

#[derive(Debug, Clone, Default)]
struct StubAuthoredRuntime {
    loaded: Option<PreparedLayout>,
    error_message: Option<String>,
    config: Config,
    prepared_load_failures_remaining: Arc<AtomicUsize>,
    rebuild_count: Arc<AtomicUsize>,
}

impl PreparedLayoutRuntime for StubAuthoredRuntime {
    type Config = Config;

    fn prepare_layout(
        &self,
        _config: &Self::Config,
        _workspace: &WorkspaceSnapshot,
    ) -> Result<Option<PreparedLayout>, RuntimeError> {
        if let Some(message) = &self.error_message {
            return Err(RuntimeError::Other { message: message.clone() });
        }

        Ok(self.loaded.clone())
    }

    fn build_context(
        &self,
        state: &StateSnapshot,
        workspace: &WorkspaceSnapshot,
        artifact: Option<&PreparedLayout>,
    ) -> LayoutEvaluationContext {
        StubRuntime { loaded: None, error_message: None }.build_context(state, workspace, artifact)
    }

    fn evaluate_layout(
        &self,
        prepared_layout: &PreparedLayout,
        context: &LayoutEvaluationContext,
    ) -> Result<SourceLayoutNode, RuntimeError> {
        StubRuntime { loaded: None, error_message: None }.evaluate_layout(prepared_layout, context)
    }

    fn contract(&self) -> LayoutModuleContract {
        LayoutModuleContract::default()
    }
}

impl AuthoringConfigRuntime for StubAuthoredRuntime {
    fn load_authored_config(&self, _path: &std::path::Path) -> Result<Config, RuntimeError> {
        Ok(self.config.clone())
    }

    fn load_prepared_config(&self, _path: &std::path::Path) -> Result<Config, RuntimeError> {
        if self.prepared_load_failures_remaining.load(Ordering::SeqCst) > 0 {
            self.prepared_load_failures_remaining.fetch_sub(1, Ordering::SeqCst);
            return Err(RuntimeError::Other {
                message: "prepared config load failed".into(),
            });
        }

        Ok(self.config.clone())
    }

    fn refresh_prepared_config(
        &self,
        _authored: &std::path::Path,
        _runtime: &std::path::Path,
    ) -> Result<RuntimeRefreshSummary, RuntimeError> {
        Ok(RuntimeRefreshSummary::default())
    }

    fn rebuild_prepared_config(
        &self,
        _authored: &std::path::Path,
        _runtime: &std::path::Path,
    ) -> Result<RuntimeRefreshSummary, RuntimeError> {
        self.rebuild_count.fetch_add(1, Ordering::SeqCst);
        Ok(RuntimeRefreshSummary::default())
    }
}

#[derive(Debug, Clone, Default)]
struct StubSourceBundleRuntime {
    config: Config,
    loaded: Option<PreparedLayout>,
}

impl SourceBundleConfigRuntime for StubSourceBundleRuntime {
    fn load_config<'a>(
        &'a self,
        _root_dir: &'a std::path::Path,
        _entry_path: &'a std::path::Path,
        _sources: &'a SourceBundle,
    ) -> Pin<Box<dyn Future<Output = Result<Config, crate::model::LayoutConfigError>> + 'a>> {
        let config = self.config.clone();
        Box::pin(async move { Ok(config) })
    }
}

impl SourceBundlePreparedLayoutRuntime for StubSourceBundleRuntime {
    fn prepare_layout<'a>(
        &'a self,
        _root_dir: &'a std::path::Path,
        _sources: &'a SourceBundle,
        _config: &'a Config,
        _workspace: &'a WorkspaceSnapshot,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Option<PreparedLayout>, crate::model::LayoutConfigError>>
                + 'a,
        >,
    > {
        let loaded = self.loaded.clone();
        Box::pin(async move { Ok(loaded) })
    }

    fn build_context(
        &self,
        state: &StateSnapshot,
        workspace: &WorkspaceSnapshot,
        artifact: Option<&PreparedLayout>,
    ) -> LayoutEvaluationContext {
        state.layout_context(workspace, artifact.map(|artifact| artifact.selected.clone()))
    }

    fn evaluate_layout<'a>(
        &'a self,
        _root_dir: &'a std::path::Path,
        _sources: &'a SourceBundle,
        _artifact: &'a PreparedLayout,
        _context: &'a LayoutEvaluationContext,
    ) -> Pin<
        Box<dyn Future<Output = Result<EvaluatedSourceLayout, crate::model::LayoutConfigError>> + 'a>,
    >
    {
        Box::pin(async move {
            Ok(EvaluatedSourceLayout {
                layout: SourceLayoutNode::Workspace { meta: Default::default(), children: vec![] },
                dependencies: Default::default(),
            })
        })
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};

    struct NoopWake;

    impl Wake for NoopWake {
        fn wake(self: Arc<Self>) {}
    }

    let waker = Waker::from(Arc::new(NoopWake));
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(&waker);

    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

fn prepared_layout(name: &str, module: &str) -> PreparedLayout {
    PreparedLayout {
        selected: SelectedLayout {
            name: name.into(),
            directory: "layouts/master-stack".into(),
            module: module.into(),
        },
        runtime_payload: runtime_cache_payload(module),
        stylesheets: Default::default(),
    }
}

fn runtime_cache_payload(module: &str) -> serde_json::Value {
    serde_json::json!({
        "entry": module,
        "modules": [{
            "specifier": module,
            "source": "export default (ctx => ({ type: 'workspace', children: [] }));",
            "resolved_imports": {},
        }],
    })
}

fn workspace() -> WorkspaceSnapshot {
    WorkspaceSnapshot {
        id: WorkspaceId::from("ws-1"),
        name: "1".into(),
        output_id: Some(OutputId::from("out-1")),
        layout_space: None,
        active_workspaces: vec!["1".into()],
        focused: true,
        visible: true,
        effective_layout: Some(LayoutRef { name: "master-stack".into() }),
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
    }
}

#[test]
fn authoring_layout_service_loads_and_caches_prepared_layout() {
    let runtime = StubRuntime {
        loaded: Some(prepared_layout("master-stack", "layouts/master-stack.js")),
        error_message: None,
    };
    let mut service = AuthoringLayoutService::new(runtime.clone(), runtime);
    let config = Config {
        layouts: vec![LayoutDefinition {
            name: "master-stack".into(),
            directory: "layouts/master-stack".into(),
            module: "layouts/master-stack.js".into(),
            stylesheet_path: Some("layouts/master-stack/index.css".into()),
            runtime_cache_payload: None,
        }],
        ..Config::default()
    };

    let loaded = service.prepare_for_workspace(&config, &workspace()).unwrap().unwrap();

    assert_eq!(loaded.selected.name, "master-stack");
    assert!(service.cache().contains_key("master-stack"));
}

#[test]
fn authoring_layout_service_evaluates_prepared_layout_for_workspace() {
    let runtime = StubRuntime {
        loaded: Some(prepared_layout("master-stack", "layouts/master-stack.js")),
        error_message: None,
    };
    let mut service = AuthoringLayoutService::new(runtime.clone(), runtime);
    let config = Config {
        layouts: vec![LayoutDefinition {
            name: "master-stack".into(),
            directory: "layouts/master-stack".into(),
            module: "layouts/master-stack.js".into(),
            stylesheet_path: Some("layouts/master-stack/index.css".into()),
            runtime_cache_payload: None,
        }],
        ..Config::default()
    };

    let evaluated =
        service.evaluate_prepared_for_workspace(&config, &state(), &workspace()).unwrap().unwrap();

    assert_eq!(evaluated.artifact.selected.name, "master-stack");
    assert!(matches!(evaluated.layout, SourceLayoutNode::Workspace { .. }));
}

#[test]
fn authoring_layout_service_loads_config_from_runtime_path() {
    let temp_dir = TempDir::new().unwrap();
    let prepared_config_path = temp_dir.path().join("config.js");
    fs::write(&prepared_config_path, "export default {};").unwrap();

    let runtime = StubRuntime { loaded: None, error_message: None };
    let service = AuthoringLayoutService::new(runtime.clone(), runtime);
    let config = service.load_config(&ConfigPaths::new("unused", &prepared_config_path)).unwrap();

    assert_eq!(config.layouts[0].module, "layouts/master-stack.js");
}

#[test]
fn authoring_layout_service_discovers_config_paths_from_options() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path().join("home");
    let config_dir = home_dir.join(".config/hypreact");
    let data_dir = home_dir.join(".cache/hypreact");
    let _ = fs::create_dir_all(&config_dir);
    let _ = fs::create_dir_all(&data_dir);
    fs::write(config_dir.join("config.ts"), "export default {};").unwrap();

    let runtime = StubRuntime { loaded: None, error_message: None };
    let service = AuthoringLayoutService::new(runtime.clone(), runtime);
    let paths = service
        .discover_config_paths(ConfigDiscoveryOptions {
            home_dir: Some(home_dir.clone()),
            ..ConfigDiscoveryOptions::default()
        })
        .unwrap();

    assert!(paths.authored_config.ends_with(".config/hypreact/config.ts"));
    assert!(paths.prepared_config.ends_with(".cache/hypreact/config.js"));
}

#[test]
fn authoring_layout_service_reports_missing_layout_module_sources() {
    let runtime = StubRuntime {
        loaded: None,
        error_message: Some("layout module `layouts/missing.js` source is unavailable".into()),
    };
    let service = AuthoringLayoutService::new(runtime.clone(), runtime);
    let config = Config {
        layouts: vec![LayoutDefinition {
            name: "missing".into(),
            directory: "layouts/missing".into(),
            module: "layouts/missing.js".into(),
            stylesheet_path: Some("layouts/missing/index.css".into()),
            runtime_cache_payload: None,
        }],
        ..Config::default()
    };

    let errors = service.validate_layout_modules(&config).unwrap();

    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("missing"));
}

#[test]
fn authoring_layout_service_loads_authored_config_when_runtime_js_is_missing() {
    let project_root = TempDir::new().unwrap();
    let authored_config = Config {
        layouts: vec![LayoutDefinition {
            name: "master-stack".into(),
            directory: "layouts/master-stack".into(),
            module: "layouts/master-stack/index.js".into(),
            stylesheet_path: Some("layouts/master-stack/index.css".into()),
            runtime_cache_payload: Some(runtime_cache_payload("layouts/master-stack/index.js")),
        }],
        ..Config::default()
    };

    let runtime = StubAuthoredRuntime {
        loaded: None,
        error_message: None,
        config: authored_config,
        prepared_load_failures_remaining: Arc::new(AtomicUsize::new(0)),
        rebuild_count: Arc::new(AtomicUsize::new(0)),
    };
    let service = AuthoringLayoutService::new(runtime.clone(), runtime);
    let config = service
        .load_config(&ConfigPaths::new(
            project_root.path().join("config.ts"),
            project_root.path().join("missing-config.js"),
        ))
        .unwrap();

    assert_eq!(config.layouts.len(), 1);
    assert!(config.layouts[0].runtime_cache_payload.is_some());
}

#[test]
fn authoring_layout_service_rebuilds_prepared_config_after_load_failure() {
    let project_root = TempDir::new().unwrap();
    let authored_path = project_root.path().join("config.ts");
    let prepared_path = project_root.path().join("config.js");
    fs::write(&authored_path, "export default {};").unwrap();
    fs::write(&prepared_path, "export default {};").unwrap();

    let rebuild_count = Arc::new(AtomicUsize::new(0));
    let runtime = StubAuthoredRuntime {
        loaded: None,
        error_message: None,
        config: Config {
            layouts: vec![LayoutDefinition {
                name: "master-stack".into(),
                directory: "layouts/master-stack".into(),
                module: "layouts/master-stack/index.js".into(),
                stylesheet_path: Some("layouts/master-stack/index.css".into()),
                runtime_cache_payload: Some(runtime_cache_payload("layouts/master-stack/index.js")),
            }],
            ..Config::default()
        },
        prepared_load_failures_remaining: Arc::new(AtomicUsize::new(1)),
        rebuild_count: rebuild_count.clone(),
    };
    let service = AuthoringLayoutService::new(runtime.clone(), runtime);

    let _config = service.load_config(&ConfigPaths::new(&authored_path, &prepared_path)).unwrap();

    assert_eq!(rebuild_count.load(Ordering::SeqCst), 1);
}

#[test]
    fn source_bundle_authoring_layout_service_loads_config() {
        let runtime = StubSourceBundleRuntime {
            config: Config {
                layouts: vec![LayoutDefinition {
                    name: "master-stack".into(),
                    directory: "layouts/master-stack".into(),
                module: "layouts/master-stack/index.tsx".into(),
                stylesheet_path: Some("layouts/master-stack/index.css".into()),
                runtime_cache_payload: Some(runtime_cache_payload(
                    "layouts/master-stack/index.tsx",
                )),
            }],
            ..Config::default()
        },
        loaded: None,
    };
    let service = SourceBundleAuthoringLayoutService::from_runtime_bundle(
        Box::new(runtime.clone()),
        Box::new(runtime),
    );

    let config = block_on(service.load_config(
        std::path::Path::new("/workspace"),
        std::path::Path::new("/workspace/config.ts"),
        &SourceBundle::new(),
    ))
    .unwrap();

    assert_eq!(config.layouts.len(), 1);
}

#[test]
fn source_bundle_authoring_layout_service_evaluates_prepared_layout() {
    let runtime = StubSourceBundleRuntime {
        config: Config {
            layouts: vec![LayoutDefinition {
                name: "master-stack".into(),
                directory: "layouts/master-stack".into(),
                module: "layouts/master-stack/index.tsx".into(),
                stylesheet_path: Some("layouts/master-stack/index.css".into()),
                runtime_cache_payload: Some(runtime_cache_payload(
                    "layouts/master-stack/index.tsx",
                )),
            }],
            ..Config::default()
        },
        loaded: Some(prepared_layout("master-stack", "layouts/master-stack/index.tsx")),
    };
    let mut service = SourceBundleAuthoringLayoutService::from_runtime_bundle(
        Box::new(runtime.clone()),
        Box::new(runtime),
    );
    let config = Config {
        layouts: vec![LayoutDefinition {
            name: "master-stack".into(),
            directory: "layouts/master-stack".into(),
            module: "layouts/master-stack/index.tsx".into(),
            stylesheet_path: Some("layouts/master-stack/index.css".into()),
            runtime_cache_payload: Some(runtime_cache_payload("layouts/master-stack/index.tsx")),
        }],
        ..Config::default()
    };

    let evaluated = block_on(service.evaluate_prepared_for_workspace(
        std::path::Path::new("/workspace"),
        &SourceBundle::new(),
        &config,
        &state(),
        &workspace(),
    ))
    .unwrap()
    .unwrap();

    assert_eq!(evaluated.artifact.selected.name, "master-stack");
    assert!(matches!(evaluated.layout, SourceLayoutNode::Workspace { .. }));
}
