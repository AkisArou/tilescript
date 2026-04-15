use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use hypreact_config::authoring_layout::SourceBundleAuthoringLayoutService;
use hypreact_config::model::{Config, LayoutConfigError};
use hypreact_config::runtime::build_source_bundle_authoring_layout_service;
use hypreact_core::query::state_snapshot_for_model;
use hypreact_core::snapshot::WindowSnapshot;
use hypreact_core::wm::WmModel;
use hypreact_css::analysis::{CssDiagnosticCode, CssDiagnosticSeverity, analyze_stylesheet};
use hypreact_runtime_js_browser::JavaScriptBrowserRuntimeProvider;
use hypreact_scene::ast::ValidatedLayoutTree;
use hypreact_scene::pipeline::SceneCache;
use hypreact_scene::SceneResponse;

use crate::editor_files::{EDITOR_FILES, ENTRY_RUNTIME_PATH, EditorFileId, runtime_path};
use crate::session::PreviewDiagnostic;

const ROOT_DIR: &str = "/playground";

#[derive(Debug, Clone)]
pub struct EvaluatedPreview {
    pub config: Config,
    pub scene: Option<SceneResponse>,
    pub diagnostics: Vec<PreviewDiagnostic>,
    pub selected_layout_name: Option<String>,
    pub error: Option<String>,
}

pub async fn load_config_from_buffers(
    buffers: &BTreeMap<EditorFileId, String>,
) -> Result<Config, String> {
    let root_dir = PathBuf::from(ROOT_DIR);
    let entry_path = PathBuf::from(ENTRY_RUNTIME_PATH);
    let sources = source_bundle_sources(buffers);
    let maybe_service = LAYOUT_SERVICE.with_borrow_mut(|slot| slot.take());
    let service = match maybe_service {
        Some(service) => service,
        None => build_layout_service(&entry_path).map_err(|error| error.to_string())?,
    };

    let result = service
        .load_config(&root_dir, &entry_path, &sources)
        .await
        .map_err(|error| error.to_string());

    LAYOUT_SERVICE.with_borrow_mut(|slot| {
        *slot = Some(service);
    });

    result
}

pub async fn evaluate_preview_from_buffers(
    buffers: &BTreeMap<EditorFileId, String>,
    model: &WmModel,
) -> Result<EvaluatedPreview, String> {
    let root_dir = PathBuf::from(ROOT_DIR);
    let entry_path = PathBuf::from(ENTRY_RUNTIME_PATH);
    let sources = source_bundle_sources(buffers);
    let maybe_service = LAYOUT_SERVICE.with_borrow_mut(|slot| slot.take());
    let mut service = match maybe_service {
        Some(service) => service,
        None => build_layout_service(&entry_path).map_err(|error| error.to_string())?,
    };

    let result = async {
        let config = service
            .load_config(&root_dir, &entry_path, &sources)
            .await
            .map_err(|error| error.to_string())?;

        let mut preview_model = model.clone();
        apply_layout_selection(&mut preview_model, &config);

        let state_snapshot = state_snapshot_for_model(&preview_model);
        let workspace = state_snapshot
            .current_workspace()
            .cloned()
            .ok_or_else(|| "preview workspace is unavailable".to_string())?;

        let selected_layout_name = workspace.effective_layout.as_ref().map(|layout| layout.name.clone());
        let evaluation = service
            .evaluate_prepared_for_workspace(&root_dir, &sources, &config, &state_snapshot, &workspace)
            .await
            .map_err(|error| error.to_string())?;

        let diagnostics = collect_diagnostics_from_buffers(buffers);

        match evaluation {
            Some(evaluation) => {
                let workspace_windows = state_snapshot
                    .windows
                    .iter()
                    .filter(|window| window.workspace_id.as_ref() == Some(&workspace.id))
                    .filter(|window| window.output_id.as_ref() == workspace.output_id.as_ref())
                    .filter(|window| window.mapped && !window.closing && !window.mode.is_floating() && !window.mode.is_fullscreen())
                    .cloned()
                    .collect::<Vec<WindowSnapshot>>();
                let scene = build_scene(&config, &state_snapshot, &workspace, &workspace_windows, &evaluation)
                    .map_err(|error| error.to_string())?;

                Ok(EvaluatedPreview {
                    config,
                    scene: Some(scene),
                    diagnostics,
                    selected_layout_name: Some(evaluation.artifact.selected.name.clone()),
                    error: None,
                })
            }
            None => Ok(EvaluatedPreview {
                config,
                scene: None,
                diagnostics,
                selected_layout_name,
                error: None,
            }),
        }
    }
    .await;

    LAYOUT_SERVICE.with_borrow_mut(|slot| {
        *slot = Some(service);
    });

    result
}

pub fn source_bundle_sources(
    buffers: &BTreeMap<EditorFileId, String>,
) -> BTreeMap<PathBuf, String> {
    let mut sources = BTreeMap::new();
    for file in EDITOR_FILES {
        let source = buffers
            .get(&file.id)
            .cloned()
            .unwrap_or_else(|| crate::editor_files::initial_content(file.id).to_string());
        sources.insert(PathBuf::from(runtime_path(file.id)), source);
    }
    sources
}

pub fn apply_layout_selection(model: &mut WmModel, config: &Config) {
    let current_output_id = model.current_output_id().cloned();
    for workspace in model.workspaces.values_mut() {
        workspace.effective_layout = config.selected_layout_ref_for_workspace(
            &workspace.name,
            workspace.output_id.as_ref().or(current_output_id.as_ref()),
        );
    }
}

fn build_layout_service(
    entry_path: &Path,
) -> Result<SourceBundleAuthoringLayoutService, LayoutConfigError> {
    let provider = JavaScriptBrowserRuntimeProvider::new();
    let bundle = provider.build_source_bundle_runtime_bundle()?;
    build_source_bundle_authoring_layout_service(entry_path, bundle)
}

thread_local! {
    static LAYOUT_SERVICE: RefCell<Option<SourceBundleAuthoringLayoutService>> = const { RefCell::new(None) };
}

fn build_scene(
    config: &Config,
    state_snapshot: &hypreact_core::snapshot::StateSnapshot,
    workspace: &hypreact_core::snapshot::WorkspaceSnapshot,
    workspace_windows: &[WindowSnapshot],
    evaluation: &hypreact_config::authoring_layout::PreparedSourceBundleLayoutEvaluation,
) -> Result<SceneResponse, LayoutConfigError> {
    let resolved = ValidatedLayoutTree::new(evaluation.layout.clone())
        .map_err(|error| LayoutConfigError::EvaluateAuthoredConfig {
            path: PathBuf::from(&evaluation.artifact.selected.module),
            message: error.to_string(),
        })?
        .resolve(workspace_windows)
        .map_err(|error| LayoutConfigError::EvaluateAuthoredConfig {
            path: PathBuf::from(&evaluation.artifact.selected.module),
            message: error.to_string(),
        })?;

    let request = config.build_scene_request(
        state_snapshot,
        workspace,
        workspace
            .output_id
            .as_ref()
            .and_then(|output_id| state_snapshot.output_by_id(output_id))
            .or_else(|| state_snapshot.current_output()),
        resolved.root,
        &evaluation.artifact,
    )?;

    SceneCache::new().compute_layout_from_request(&request).map_err(|error| {
        LayoutConfigError::EvaluateAuthoredConfig {
            path: PathBuf::from(&evaluation.artifact.selected.module),
            message: error.to_string(),
        }
    })
}

fn collect_diagnostics_from_buffers(
    buffers: &BTreeMap<EditorFileId, String>,
) -> Vec<PreviewDiagnostic> {
    EDITOR_FILES
        .iter()
        .filter(|file| file.language == "css")
        .flat_map(|file| {
            let source = buffers
                .get(&file.id)
                .cloned()
                .unwrap_or_else(|| crate::editor_files::initial_content(file.id).to_string());
            analyze_stylesheet(&source)
                .diagnostics
                .into_iter()
                .map(move |diagnostic| PreviewDiagnostic {
                    path: file.path.to_string(),
                    severity: match diagnostic.severity {
                        CssDiagnosticSeverity::Error => "error",
                        CssDiagnosticSeverity::Warning => "warning",
                        CssDiagnosticSeverity::Information => "information",
                    },
                    code: match diagnostic.code {
                        CssDiagnosticCode::UnsupportedAtRule => "unsupportedAtRule",
                        CssDiagnosticCode::UnsupportedSelector => "unsupportedSelector",
                        CssDiagnosticCode::UnsupportedProperty => "unsupportedProperty",
                        CssDiagnosticCode::InvalidSyntax => "invalidSyntax",
                        CssDiagnosticCode::UnsupportedValue => "unsupportedValue",
                        CssDiagnosticCode::InapplicableProperty => "inapplicableProperty",
                        CssDiagnosticCode::UnknownAnimationName => "unknownAnimationName",
                        CssDiagnosticCode::UnsupportedAttributeKey => "unsupportedAttributeKey",
                    },
                    message: diagnostic.message,
                    range: format!(
                        "{}:{}-{}:{}",
                        diagnostic.range.start_line,
                        diagnostic.range.start_column,
                        diagnostic.range.end_line,
                        diagnostic.range.end_column,
                    ),
                })
        })
        .collect()
}
