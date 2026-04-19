use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use tilescript_config::authoring_layout::SourceBundleAuthoringLayoutService;
use tilescript_config::model::{Config, LayoutConfigError};
use tilescript_config::runtime::build_source_bundle_authoring_layout_service;
use tilescript_core::focus::{FocusTree, FocusTreeWindowGeometry};
use tilescript_core::navigation::WindowGeometryCandidate;
use tilescript_core::query::state_snapshot_for_model;
use tilescript_core::resize::{
    DEFAULT_BRANCH_SHARE_UNITS, DEFAULT_RESIZE_STEP_UNITS, MIN_BRANCH_SHARE_UNITS, PartitionAxis,
    PartitionBranch, PartitionConstraints, PartitionId, PartitionNode, PartitionTree,
    scale_authored_share_units,
};
use tilescript_core::runtime::prepared_layout::{
    PreparedLayout, PreparedStylesheets, SelectedLayout,
};
use tilescript_core::runtime::runtime_kind::RuntimeKind;
use tilescript_core::snapshot::WindowSnapshot;
use tilescript_core::wm::{WindowGeometry, WmModel};
use tilescript_core::{LayoutNodeMeta, RemainingTake, SlotTake, SourceLayoutNode};
use tilescript_css::analysis::{CssDiagnosticCode, CssDiagnosticSeverity, analyze_stylesheet};
use tilescript_runtime_js_browser::JavaScriptBrowserRuntimeProvider;
use tilescript_runtime_lua_browser::LuaBrowserRuntimeProvider;
use tilescript_scene::ast::ValidatedLayoutTree;
use tilescript_scene::pipeline::SceneCache;
use tilescript_scene::{Display, FlexDirectionValue, LayoutSnapshotNode, SceneResponse, SizeValue};

use crate::editor_files::{
    AuthoringLanguage, DynamicLayoutFileSet, EditorFileKey, entry_runtime_path,
    file_layout_language, initial_content_for_key, iter_dynamic_files, runtime_path, static_files,
};
use crate::session::PreviewDiagnostic;

const ROOT_DIR: &str = "/playground";
const FALLBACK_LAYOUT_STYLESHEET: &str =
    "workspace { display: flex; width: 100%; height: 100%; } window { flex: 1 1 0; }";

#[derive(Debug, Clone)]
pub struct EvaluatedPreview {
    pub scene: Option<SceneResponse>,
    pub focus_tree: Option<FocusTree>,
    pub partition_tree: Option<PartitionTree>,
    pub diagnostics: Vec<PreviewDiagnostic>,
    pub selected_layout_name: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ResizeBehaviorConfig {
    step_px: f32,
    min_branch_main_size_px: f32,
}

impl ResizeBehaviorConfig {
    fn from_config(config: &Config) -> Self {
        Self {
            step_px: config.resize.step_px.unwrap_or(DEFAULT_RESIZE_STEP_UNITS as f32 * 8.0),
            min_branch_main_size_px: config.resize.min_branch_size_px.unwrap_or(120.0),
        }
    }
}

pub async fn load_config_from_buffers(
    language: AuthoringLanguage,
    buffers: &BTreeMap<EditorFileKey, String>,
    dynamic_layouts: &[DynamicLayoutFileSet],
) -> Result<Config, String> {
    let root_dir = PathBuf::from(ROOT_DIR);
    let entry_path = PathBuf::from(entry_runtime_path(language));
    let sources = source_bundle_sources(language, buffers, dynamic_layouts);
    let maybe_service = LAYOUT_SERVICES.with_borrow_mut(|slot| slot.remove(&language));
    let mut service = match maybe_service {
        Some(service) => service,
        None => build_layout_service(language, &entry_path).map_err(|error| error.to_string())?,
    };

    let result = service
        .load_config(&root_dir, &entry_path, &sources)
        .await
        .map_err(|error| error.to_string());

    LAYOUT_SERVICES.with_borrow_mut(|slot| {
        slot.insert(language, service);
    });

    result
}

pub async fn evaluate_preview_from_buffers(
    language: AuthoringLanguage,
    buffers: &BTreeMap<EditorFileKey, String>,
    dynamic_layouts: &[DynamicLayoutFileSet],
    config: &Config,
    model: &WmModel,
    manual_layouts: &BTreeMap<tilescript_core::WorkspaceId, tilescript_core::types::LayoutRef>,
    preserve_last_scene_on_error: bool,
) -> Result<EvaluatedPreview, String> {
    let root_dir = PathBuf::from(ROOT_DIR);
    let entry_path = PathBuf::from(entry_runtime_path(language));
    let sources = source_bundle_sources(language, buffers, dynamic_layouts);
    let maybe_service = LAYOUT_SERVICES.with_borrow_mut(|slot| slot.remove(&language));
    let mut service = match maybe_service {
        Some(service) => service,
        None => build_layout_service(language, &entry_path).map_err(|error| error.to_string())?,
    };

    let result = async {
        let mut preview_model = model.clone();
        apply_layout_selection(&mut preview_model, config);
        for (workspace_id, layout) in manual_layouts {
            preview_model
                .set_workspace_effective_layout(workspace_id.clone(), Some(layout.clone()));
        }

        let state_snapshot = state_snapshot_for_model(&preview_model);
        let workspace = state_snapshot
            .current_workspace()
            .cloned()
            .ok_or_else(|| "preview workspace is unavailable".to_string())?;

        let selected_layout_name =
            workspace.effective_layout.as_ref().map(|layout| layout.name.clone());
        let evaluation = service
            .evaluate_prepared_for_workspace(
                &root_dir,
                &sources,
                config,
                &state_snapshot,
                &workspace,
            )
            .await;

        let mut diagnostics = collect_diagnostics_from_buffers(language, buffers, dynamic_layouts);

        match evaluation {
            Ok(Some(evaluation)) => {
                let workspace_windows = workspace_windows(&state_snapshot, &workspace);
                let scene = match build_scene(
                    config,
                    &state_snapshot,
                    &workspace,
                    &workspace_windows,
                    &evaluation,
                ) {
                    Ok(scene) => scene,
                    Err(error) => {
                        let error_message = error.to_string();
                        diagnostics.push(layout_error_diagnostic_with_fallback(
                            Some(&evaluation.artifact.selected.module),
                            error_message.clone(),
                            false,
                        ));
                        if preserve_last_scene_on_error {
                            return Ok(EvaluatedPreview {
                                scene: None,
                                focus_tree: None,
                                partition_tree: None,
                                diagnostics,
                                selected_layout_name: Some(evaluation.artifact.selected.name.clone()),
                                error: Some(error_message),
                            });
                        }
                        build_fallback_scene(
                            language,
                            config,
                            &state_snapshot,
                            &workspace,
                            Some(&evaluation.artifact.selected),
                            &workspace_windows,
                        )?
                    }
                };
                Ok(preview_from_scene(
                    config.clone(),
                    scene,
                    diagnostics,
                    Some(evaluation.artifact.selected.name.clone()),
                ))
            }
            Ok(None) => Ok(EvaluatedPreview {
                scene: None,
                focus_tree: None,
                partition_tree: None,
                diagnostics,
                selected_layout_name,
                error: None,
            }),
            Err(error) => {
                diagnostics.push(layout_error_diagnostic_with_fallback(
                    Some(entry_runtime_path(language)),
                    error.to_string(),
                    !preserve_last_scene_on_error,
                ));

                if preserve_last_scene_on_error {
                    return Ok(EvaluatedPreview {
                        scene: None,
                        focus_tree: None,
                        partition_tree: None,
                        diagnostics,
                        selected_layout_name,
                        error: Some(error.to_string()),
                    });
                }

                let workspace_windows = workspace_windows(&state_snapshot, &workspace);
                let selected_layout = selected_layout_for_workspace(language, &workspace, None);
                let scene = build_fallback_scene(
                    language,
                    config,
                    &state_snapshot,
                    &workspace,
                    Some(&selected_layout),
                    &workspace_windows,
                )?;

                Ok(preview_from_scene(config.clone(), scene, diagnostics, selected_layout_name))
            }
        }
    }
    .await;

    LAYOUT_SERVICES.with_borrow_mut(|slot| {
        slot.insert(language, service);
    });

    result
}

pub fn source_bundle_sources(
    language: AuthoringLanguage,
    buffers: &BTreeMap<EditorFileKey, String>,
    dynamic_layouts: &[DynamicLayoutFileSet],
) -> BTreeMap<PathBuf, String> {
    let mut sources = BTreeMap::new();
    for file in static_files(language) {
        let file_key = file.key();
        let source = buffers
            .get(&file_key)
            .cloned()
            .unwrap_or_else(|| initial_content_for_key(&file_key, dynamic_layouts));
        sources.insert(PathBuf::from(runtime_path(&file_key, dynamic_layouts)), source);
    }

    for file in iter_dynamic_files(dynamic_layouts)
        .filter(|file| file_layout_language(&file.key, dynamic_layouts) == Some(language))
    {
        let source =
            buffers.get(&file.key).cloned().unwrap_or_else(|| file.initial_content.clone());
        sources.insert(PathBuf::from(runtime_path(&file.key, dynamic_layouts)), source);
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
    language: AuthoringLanguage,
    entry_path: &Path,
) -> Result<SourceBundleAuthoringLayoutService, LayoutConfigError> {
    let bundle = match language {
        AuthoringLanguage::JavaScript => {
            JavaScriptBrowserRuntimeProvider::new().build_source_bundle_runtime_bundle()?
        }
        AuthoringLanguage::Lua | AuthoringLanguage::Fennel => {
            LuaBrowserRuntimeProvider::new().build_source_bundle_runtime_bundle()?
        }
    };
    build_source_bundle_authoring_layout_service(entry_path, bundle)
}

thread_local! {
    static LAYOUT_SERVICES: RefCell<BTreeMap<AuthoringLanguage, SourceBundleAuthoringLayoutService>> = const { RefCell::new(BTreeMap::new()) };
}

fn build_scene(
    config: &Config,
    state_snapshot: &tilescript_core::snapshot::StateSnapshot,
    workspace: &tilescript_core::snapshot::WorkspaceSnapshot,
    workspace_windows: &[WindowSnapshot],
    evaluation: &tilescript_config::authoring_layout::PreparedSourceBundleLayoutEvaluation,
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

fn workspace_windows(
    state_snapshot: &tilescript_core::snapshot::StateSnapshot,
    workspace: &tilescript_core::snapshot::WorkspaceSnapshot,
) -> Vec<WindowSnapshot> {
    state_snapshot
        .windows
        .iter()
        .filter(|window| window.workspace_id.as_ref() == Some(&workspace.id))
        .filter(|window| window.output_id.as_ref() == workspace.output_id.as_ref())
        .filter(|window| {
            window.mapped
                && !window.closing
                && !window.mode.is_floating()
                && !window.mode.is_fullscreen()
        })
        .cloned()
        .collect()
}

fn build_fallback_scene(
    language: AuthoringLanguage,
    config: &Config,
    state_snapshot: &tilescript_core::snapshot::StateSnapshot,
    workspace: &tilescript_core::snapshot::WorkspaceSnapshot,
    selected_layout: Option<&SelectedLayout>,
    workspace_windows: &[WindowSnapshot],
) -> Result<SceneResponse, String> {
    let artifact = PreparedLayout {
        selected: selected_layout
            .cloned()
            .unwrap_or_else(|| selected_layout_for_workspace(language, workspace, None)),
        runtime_payload: serde_json::Value::Null,
        stylesheets: PreparedStylesheets {
            global: None,
            layout: Some(tilescript_core::runtime::prepared_layout::PreparedStylesheet {
                path: format!("{}/fallback.css", artifact_style_directory(workspace)),
                source: FALLBACK_LAYOUT_STYLESHEET.to_string(),
            }),
        },
        dependencies: vec![],
    };

    let resolved = ValidatedLayoutTree::new(fallback_source_layout())
        .map_err(|error| error.to_string())?
        .resolve(workspace_windows)
        .map_err(|error| error.to_string())?;

    let request = config
        .build_scene_request(
            state_snapshot,
            workspace,
            workspace
                .output_id
                .as_ref()
                .and_then(|output_id| state_snapshot.output_by_id(output_id))
                .or_else(|| state_snapshot.current_output()),
            resolved.root,
            &artifact,
        )
        .map_err(|error| error.to_string())?;

    SceneCache::new().compute_layout_from_request(&request).map_err(|error| error.to_string())
}

fn selected_layout_for_workspace(
    language: AuthoringLanguage,
    workspace: &tilescript_core::snapshot::WorkspaceSnapshot,
    fallback_module: Option<&str>,
) -> SelectedLayout {
    let name = workspace
        .effective_layout
        .as_ref()
        .map(|layout| layout.name.clone())
        .unwrap_or_else(|| "fallback".to_string());
    let directory = workspace
        .effective_layout
        .as_ref()
        .map(|layout| format!("layouts/{}", layout.name))
        .unwrap_or_else(|| "layouts/fallback".to_string());

    SelectedLayout {
        runtime: match language {
            AuthoringLanguage::JavaScript => RuntimeKind::Js,
            AuthoringLanguage::Lua | AuthoringLanguage::Fennel => RuntimeKind::Lua,
        },
        module: fallback_module.map(str::to_string).unwrap_or_else(|| match language {
            AuthoringLanguage::JavaScript => format!("{directory}/index.tsx"),
            AuthoringLanguage::Lua => format!("{directory}/index.lua"),
            AuthoringLanguage::Fennel => format!("{directory}/index.fnl"),
        }),
        name,
        directory,
    }
}

fn fallback_source_layout() -> SourceLayoutNode {
    SourceLayoutNode::Workspace {
        meta: LayoutNodeMeta::default(),
        children: vec![SourceLayoutNode::Slot {
            meta: LayoutNodeMeta::default(),
            window_match: None,
            take: SlotTake::Remaining(RemainingTake::Remaining),
        }],
    }
}

fn artifact_style_directory(workspace: &tilescript_core::snapshot::WorkspaceSnapshot) -> String {
    workspace
        .effective_layout
        .as_ref()
        .map(|layout| format!("layouts/{}", layout.name))
        .unwrap_or_else(|| "layouts/fallback".to_string())
}

fn layout_error_diagnostic_with_fallback(
    path: Option<&str>,
    message: String,
    used_fallback: bool,
) -> PreviewDiagnostic {
    PreviewDiagnostic {
        path: path.unwrap_or(ROOT_DIR).to_string(),
        severity: "error",
        code: "layoutFallback",
        message: if used_fallback {
            format!("{message}; using fallback layout")
        } else {
            message
        },
        range: "1:1-1:1".to_string(),
    }
}

fn preview_from_scene(
    config: Config,
    scene: SceneResponse,
    diagnostics: Vec<PreviewDiagnostic>,
    selected_layout_name: Option<String>,
) -> EvaluatedPreview {
    let window_geometries = collect_window_geometries(&scene.root);
    let focus_tree = FocusTree::from_window_geometries(&window_geometries);
    let partition_tree =
        partition_tree_from_scene(&scene.root, ResizeBehaviorConfig::from_config(&config));
    let error = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "layoutFallback")
        .map(|diagnostic| diagnostic.message.clone());

    EvaluatedPreview {
        scene: Some(scene),
        focus_tree: Some(focus_tree),
        partition_tree: Some(partition_tree),
        diagnostics,
        selected_layout_name,
        error,
    }
}

fn collect_diagnostics_from_buffers(
    language: AuthoringLanguage,
    buffers: &BTreeMap<EditorFileKey, String>,
    dynamic_layouts: &[DynamicLayoutFileSet],
) -> Vec<PreviewDiagnostic> {
    static_files(language)
        .iter()
        .filter(|file| file.language() == "css")
        .flat_map(|file| {
            let file_key = file.key();
            let source = buffers
                .get(&file_key)
                .cloned()
                .unwrap_or_else(|| initial_content_for_key(&file_key, dynamic_layouts));
            analyze_stylesheet(&source).diagnostics.into_iter().map(move |diagnostic| {
                PreviewDiagnostic {
                    path: file.path().to_string(),
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
                }
            })
        })
        .chain(
            iter_dynamic_files(dynamic_layouts)
                .filter(|file| file_layout_language(&file.key, dynamic_layouts) == Some(language))
                .filter(|file| file.language == "css")
                .flat_map(|file| {
                    let source = buffers
                        .get(&file.key)
                        .cloned()
                        .unwrap_or_else(|| file.initial_content.clone());
                    analyze_stylesheet(&source).diagnostics.into_iter().map(move |diagnostic| {
                        PreviewDiagnostic {
                            path: file.path.clone(),
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
                                CssDiagnosticCode::UnsupportedAttributeKey => {
                                    "unsupportedAttributeKey"
                                }
                            },
                            message: diagnostic.message,
                            range: format!(
                                "{}:{}-{}:{}",
                                diagnostic.range.start_line,
                                diagnostic.range.start_column,
                                diagnostic.range.end_line,
                                diagnostic.range.end_column,
                            ),
                        }
                    })
                }),
        )
        .collect()
}

pub fn resize_step_units_for_partition(
    partition_tree: &PartitionTree,
    partition_id: &PartitionId,
    step_px: f32,
) -> u32 {
    let Some(partition) = partition_tree.partitions.get(partition_id) else {
        return DEFAULT_RESIZE_STEP_UNITS;
    };

    let partition_main_size = match partition.axis {
        PartitionAxis::Horizontal => partition.rect.width,
        PartitionAxis::Vertical => partition.rect.height,
    };
    if !partition_main_size.is_finite() || partition_main_size <= 0.0 {
        return DEFAULT_RESIZE_STEP_UNITS;
    }

    let total_share_units = partition
        .branches
        .iter()
        .map(|branch| branch.default_share.unwrap_or(DEFAULT_BRANCH_SHARE_UNITS))
        .sum::<u32>();
    if total_share_units == 0 {
        return DEFAULT_RESIZE_STEP_UNITS;
    }

    ((step_px * total_share_units as f32) / partition_main_size).round().max(1.0) as u32
}

fn collect_window_geometries(root: &LayoutSnapshotNode) -> Vec<FocusTreeWindowGeometry> {
    let mut geometries = Vec::new();
    collect_window_geometries_inner(root, &mut geometries);
    geometries
}

fn collect_window_geometries_inner(node: &LayoutSnapshotNode, out: &mut Vec<FocusTreeWindowGeometry>) {
    if let LayoutSnapshotNode::Window { window_id: Some(window_id), rect, .. } = node {
        out.push(FocusTreeWindowGeometry {
            window_id: window_id.clone(),
            geometry: WindowGeometry {
                x: rect.x.round() as i32,
                y: rect.y.round() as i32,
                width: rect.width.round() as i32,
                height: rect.height.round() as i32,
            },
        });
    }

    for child in node.children() {
        collect_window_geometries_inner(child, out);
    }
}

#[allow(dead_code)]
fn geometry_candidates_from_focus_tree(
    window_geometries: &[FocusTreeWindowGeometry],
    focus_tree: &FocusTree,
) -> Vec<WindowGeometryCandidate> {
    window_geometries
        .iter()
        .map(|entry| WindowGeometryCandidate {
            window_id: entry.window_id.clone(),
            geometry: entry.geometry,
            scope_path: focus_tree.scope_path(&entry.window_id).unwrap_or(&[]).to_vec(),
        })
        .collect()
}

fn partition_tree_from_scene(
    root: &LayoutSnapshotNode,
    resize_behavior: ResizeBehaviorConfig,
) -> PartitionTree {
    let mut tree = PartitionTree::default();
    let mut path = Vec::new();
    collect_partitions_from_scene(root, resize_behavior, &mut tree, &mut path, true);
    tree
}

fn collect_partitions_from_scene(
    node: &LayoutSnapshotNode,
    resize_behavior: ResizeBehaviorConfig,
    tree: &mut PartitionTree,
    path: &mut Vec<PartitionId>,
    is_root: bool,
) -> Vec<tilescript_core::WindowId> {
    let path_len_before_children = path.len();
    let child_window_sets = node
        .children()
        .iter()
        .map(|child| collect_partitions_from_scene(child, resize_behavior, tree, path, false))
        .collect::<Vec<_>>();
    path.truncate(path_len_before_children);

    let descendant_window_ids = match node {
        LayoutSnapshotNode::Window { window_id: Some(window_id), .. } => vec![window_id.clone()],
        _ => child_window_sets.iter().flatten().cloned().collect::<Vec<_>>(),
    };

    let maybe_axis = node.styles().and_then(|styles| partition_axis_from_style(&styles.layout));

    if let Some(axis) = maybe_axis {
        let mut branches = node
            .children()
            .iter()
            .zip(child_window_sets.iter())
            .enumerate()
            .flat_map(|(index, (child, child_windows))| {
                partition_branches_from_child(node, child, child_windows, index)
            })
            .collect::<Vec<_>>();

        if branches.len() >= 2 {
            apply_inferred_min_shares(&mut branches, axis, node.rect(), resize_behavior);

            let partition_id = node
                .meta()
                .id
                .clone()
                .map(PartitionId::new)
                .unwrap_or_else(|| PartitionId::new(structural_partition_id(node, path)));

            let partition = PartitionNode {
                partition_id: partition_id.clone(),
                axis,
                rect: node.rect(),
                branches,
                adjustable: partition_is_adjustable(node),
            };

            if is_root {
                tree.root_partition_ids.push(partition_id.clone());
            }
            tree.partitions.insert(partition_id.clone(), partition);

            for window_id in &descendant_window_ids {
                let mut partition_path = vec![partition_id.clone()];
                if let Some(existing_path) = tree.window_to_partition_path.get(window_id) {
                    partition_path.extend(existing_path.iter().cloned());
                }
                tree.window_to_partition_path.insert(window_id.clone(), partition_path);
            }
        }
    }

    descendant_window_ids
}

fn partition_branches_from_child(
    parent: &LayoutSnapshotNode,
    child: &LayoutSnapshotNode,
    child_windows: &[tilescript_core::WindowId],
    index: usize,
) -> Vec<PartitionBranch> {
    if child_windows.is_empty() {
        return Vec::new();
    }

    if child_windows.len() > 1 && child.children().len() == 1 {
        let only_child = &child.children()[0];
        let flattenable_wrapper =
            matches!(child, LayoutSnapshotNode::Content { .. }) || child.meta().id.is_none();

        if flattenable_wrapper {
            let expanded = partition_branches_from_child(parent, only_child, child_windows, index)
                .into_iter()
                .map(|mut branch| {
                    if branch.default_share.is_none() {
                        branch.default_share = inferred_branch_default_share(child);
                    }

                    if branch.constraints.max_share.is_none() {
                        branch.constraints.max_share = inferred_max_share(child);
                    }

                    if !branch.constraints.fixed {
                        branch.constraints.fixed = branch_is_fixed(child, axis_for_parent(parent));
                    }

                    branch
                })
                .collect::<Vec<_>>();
            if expanded.len() >= 2 {
                return expanded;
            }
        }
    }

    vec![PartitionBranch {
        branch_id: branch_id_for_scene_child(parent, child, index),
        rect: child.rect(),
        descendant_window_ids: child_windows.to_vec(),
        default_share: inferred_branch_default_share(child),
        constraints: inferred_branch_constraints(child, axis_for_parent(parent)),
    }]
}

fn branch_id_for_scene_child(
    parent: &LayoutSnapshotNode,
    child: &LayoutSnapshotNode,
    index: usize,
) -> String {
    if let Some(id) = child.meta().id.as_ref().filter(|id| {
        parent.children().iter().filter(|sibling| sibling.meta().id.as_ref() == Some(*id)).count()
            == 1
    }) {
        return id.clone();
    }

    if let LayoutSnapshotNode::Window { window_id: Some(window_id), .. } = child {
        return window_id.to_string();
    }

    fallback_branch_id(parent, child, index)
}

fn fallback_branch_id(
    parent: &LayoutSnapshotNode,
    child: &LayoutSnapshotNode,
    index: usize,
) -> String {
    if let LayoutSnapshotNode::Window { window_id: Some(window_id), .. } = child {
        return window_id.to_string();
    }

    if let Some(window_id) = child.children().first().and_then(|node| match node {
        LayoutSnapshotNode::Window { window_id: Some(window_id), children, .. }
            if children.is_empty() =>
        {
            Some(window_id.to_string())
        }
        _ => None,
    }) {
        return window_id;
    }

    match parent {
        LayoutSnapshotNode::Workspace { .. } => format!("workspace-branch-{index}"),
        LayoutSnapshotNode::Group { .. } => format!("group-branch-{index}"),
        LayoutSnapshotNode::Content { .. } => format!("content-branch-{index}"),
        LayoutSnapshotNode::Window { .. } => format!("window-branch-{index}"),
    }
}

fn partition_axis_from_style(computed: &tilescript_scene::ComputedStyle) -> Option<PartitionAxis> {
    (computed.display == Some(Display::Flex)).then(|| match computed.flex_direction {
        Some(FlexDirectionValue::Column) | Some(FlexDirectionValue::ColumnReverse) => {
            PartitionAxis::Vertical
        }
        _ => PartitionAxis::Horizontal,
    })
}

fn partition_is_adjustable(node: &LayoutSnapshotNode) -> bool {
    let Some(axis) = node.styles().and_then(|styles| partition_axis_from_style(&styles.layout))
    else {
        return false;
    };

    let resizable_children =
        node.children().iter().filter(|child| !branch_is_fixed(child, Some(axis))).count();

    resizable_children >= 2
}

fn inferred_branch_constraints(
    node: &LayoutSnapshotNode,
    axis: Option<PartitionAxis>,
) -> PartitionConstraints {
    PartitionConstraints {
        min_share: None,
        max_share: inferred_max_share(node),
        fixed: axis.is_some_and(|axis| branch_is_fixed(node, Some(axis))),
    }
}

fn inferred_branch_default_share(node: &LayoutSnapshotNode) -> Option<u32> {
    let styles = effective_branch_style_node(node)?.styles()?;
    let grow = styles.layout.flex_grow.unwrap_or(1.0);
    if !grow.is_finite() || grow <= 0.0 {
        return None;
    }

    Some((grow * scale_authored_share_units(1) as f32).round().max(1.0) as u32)
}

fn inferred_max_share(_node: &LayoutSnapshotNode) -> Option<u32> {
    None
}

fn apply_inferred_min_shares(
    branches: &mut [PartitionBranch],
    axis: PartitionAxis,
    partition_rect: tilescript_core::LayoutRect,
    resize_behavior: ResizeBehaviorConfig,
) {
    let partition_main_size = match axis {
        PartitionAxis::Horizontal => partition_rect.width,
        PartitionAxis::Vertical => partition_rect.height,
    };
    if !partition_main_size.is_finite() || partition_main_size <= 0.0 {
        return;
    }

    let total_default_share = branches
        .iter()
        .map(|branch| branch.default_share.unwrap_or(DEFAULT_BRANCH_SHARE_UNITS))
        .sum::<u32>();
    if total_default_share == 0 {
        return;
    }

    let inferred_floor = ((total_default_share as f32 * resize_behavior.min_branch_main_size_px)
        / partition_main_size)
        .ceil() as u32;
    let inferred_floor = inferred_floor.max(MIN_BRANCH_SHARE_UNITS);

    for branch in branches {
        if branch.constraints.fixed {
            continue;
        }

        let default_share = branch.default_share.unwrap_or(DEFAULT_BRANCH_SHARE_UNITS);
        let max_usable_floor = default_share.saturating_sub(1).max(MIN_BRANCH_SHARE_UNITS);
        let inferred_min_share = inferred_floor.min(max_usable_floor);
        branch.constraints.min_share = Some(
            branch.constraints.min_share.unwrap_or(MIN_BRANCH_SHARE_UNITS).max(inferred_min_share),
        );
    }
}

fn effective_branch_style_node<'a>(node: &'a LayoutSnapshotNode) -> Option<&'a LayoutSnapshotNode> {
    if node.styles().is_some() {
        return Some(node);
    }

    let children = node.children();
    if is_non_semantic_branch_wrapper(node) && children.len() == 1 {
        effective_branch_style_node(&children[0])
    } else {
        None
    }
}

fn is_non_semantic_branch_wrapper(node: &LayoutSnapshotNode) -> bool {
    matches!(node, LayoutSnapshotNode::Content { .. }) || node.meta().id.is_none()
}

fn axis_for_parent(parent: &LayoutSnapshotNode) -> Option<PartitionAxis> {
    parent.styles().and_then(|styles| partition_axis_from_style(&styles.layout))
}

fn branch_is_fixed(node: &LayoutSnapshotNode, axis: Option<PartitionAxis>) -> bool {
    let Some(styles) = node.styles() else {
        return false;
    };
    let Some(axis) = axis else {
        return false;
    };

    let explicit_main_size = match axis {
        PartitionAxis::Horizontal => styles.layout.width,
        PartitionAxis::Vertical => styles.layout.height,
    };

    if matches!(explicit_main_size, Some(SizeValue::LengthPercentage(_))) {
        return true;
    }

    styles.layout.flex_grow.unwrap_or(0.0) == 0.0
}

fn structural_partition_id(node: &LayoutSnapshotNode, path: &[PartitionId]) -> String {
    let node_kind = match node {
        LayoutSnapshotNode::Workspace { .. } => "workspace",
        LayoutSnapshotNode::Group { .. } => "group",
        LayoutSnapshotNode::Content { .. } => "content",
        LayoutSnapshotNode::Window { .. } => "window",
    };

    if path.is_empty() {
        format!("{node_kind}-partition")
    } else {
        format!(
            "{}/{}-partition",
            path.iter().map(|partition_id| partition_id.0.as_str()).collect::<Vec<_>>().join("/"),
            node_kind
        )
    }
}
