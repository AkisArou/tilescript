use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use hypreact_config::authoring_layout::SourceBundleAuthoringLayoutService;
use hypreact_config::model::{Config, LayoutConfigError};
use hypreact_config::runtime::build_source_bundle_authoring_layout_service;
use hypreact_core::focus::{FocusTree, FocusTreeWindowGeometry};
use hypreact_core::navigation::WindowGeometryCandidate;
use hypreact_core::query::state_snapshot_for_model;
use hypreact_core::resize::{
    DEFAULT_BRANCH_SHARE_UNITS, DEFAULT_RESIZE_STEP_UNITS, MIN_BRANCH_SHARE_UNITS, PartitionAxis,
    PartitionBranch, PartitionConstraints, PartitionId, PartitionNode, PartitionTree,
    scale_authored_share_units,
};
use hypreact_core::snapshot::WindowSnapshot;
use hypreact_core::wm::{WindowGeometry, WmModel};
use hypreact_css::analysis::{CssDiagnosticCode, CssDiagnosticSeverity, analyze_stylesheet};
use hypreact_runtime_js_browser::JavaScriptBrowserRuntimeProvider;
use hypreact_scene::ast::ValidatedLayoutTree;
use hypreact_scene::pipeline::SceneCache;
use hypreact_scene::{Display, FlexDirectionValue, LayoutSnapshotNode, SceneResponse, SizeValue};

use crate::editor_files::{
    DynamicLayoutFileSet, EDITOR_FILES, ENTRY_RUNTIME_PATH, EditorFileKey,
    initial_content_for_key, iter_dynamic_files, runtime_path,
};
use crate::session::PreviewDiagnostic;

const ROOT_DIR: &str = "/playground";

#[derive(Debug, Clone)]
pub struct EvaluatedPreview {
    pub config: Config,
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
    buffers: &BTreeMap<EditorFileKey, String>,
    dynamic_layouts: &[DynamicLayoutFileSet],
) -> Result<Config, String> {
    let root_dir = PathBuf::from(ROOT_DIR);
    let entry_path = PathBuf::from(ENTRY_RUNTIME_PATH);
    let sources = source_bundle_sources(buffers, dynamic_layouts);
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
    buffers: &BTreeMap<EditorFileKey, String>,
    dynamic_layouts: &[DynamicLayoutFileSet],
    model: &WmModel,
    manual_layouts: &BTreeMap<hypreact_core::WorkspaceId, hypreact_core::types::LayoutRef>,
) -> Result<EvaluatedPreview, String> {
    let root_dir = PathBuf::from(ROOT_DIR);
    let entry_path = PathBuf::from(ENTRY_RUNTIME_PATH);
    let sources = source_bundle_sources(buffers, dynamic_layouts);
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
        for (workspace_id, layout) in manual_layouts {
            preview_model.set_workspace_effective_layout(workspace_id.clone(), Some(layout.clone()));
        }

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

        let diagnostics = collect_diagnostics_from_buffers(buffers, dynamic_layouts);

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
                let window_geometries = collect_window_geometries(&scene.root);
                let focus_tree = focus_tree_from_geometries(&window_geometries);
                let partition_tree =
                    partition_tree_from_scene(&scene.root, ResizeBehaviorConfig::from_config(&config));

                Ok(EvaluatedPreview {
                    config,
                    scene: Some(scene),
                    focus_tree: Some(focus_tree),
                    partition_tree: Some(partition_tree),
                    diagnostics,
                    selected_layout_name: Some(evaluation.artifact.selected.name.clone()),
                    error: None,
                })
            }
            None => Ok(EvaluatedPreview {
                config,
                scene: None,
                focus_tree: None,
                partition_tree: None,
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
    buffers: &BTreeMap<EditorFileKey, String>,
    dynamic_layouts: &[DynamicLayoutFileSet],
) -> BTreeMap<PathBuf, String> {
    let mut sources = BTreeMap::new();
    for file in EDITOR_FILES {
        let file_key = EditorFileKey::Static(file.id);
        let source = buffers
            .get(&file_key)
            .cloned()
            .unwrap_or_else(|| initial_content_for_key(&file_key, dynamic_layouts));
        sources.insert(PathBuf::from(runtime_path(&file_key, dynamic_layouts)), source);
    }

    for file in iter_dynamic_files(dynamic_layouts) {
        let source = buffers
            .get(&file.key)
            .cloned()
            .unwrap_or_else(|| file.initial_content.clone());
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
    buffers: &BTreeMap<EditorFileKey, String>,
    dynamic_layouts: &[DynamicLayoutFileSet],
) -> Vec<PreviewDiagnostic> {
    EDITOR_FILES
        .iter()
        .filter(|file| file.language == "css")
        .flat_map(|file| {
            let file_key = EditorFileKey::Static(file.id);
            let source = buffers
                .get(&file_key)
                .cloned()
                .unwrap_or_else(|| initial_content_for_key(&file_key, dynamic_layouts));
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
        .chain(
            iter_dynamic_files(dynamic_layouts)
                .filter(|file| file.language == "css")
                .flat_map(|file| {
                    let source = buffers
                        .get(&file.key)
                        .cloned()
                        .unwrap_or_else(|| file.initial_content.clone());
                    analyze_stylesheet(&source)
                        .diagnostics
                        .into_iter()
                        .map(move |diagnostic| PreviewDiagnostic {
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

fn collect_window_geometries(
    root: &LayoutSnapshotNode,
) -> std::collections::BTreeMap<hypreact_core::WindowId, WindowGeometry> {
    let mut geometries = std::collections::BTreeMap::new();
    collect_window_geometries_inner(root, &mut geometries);
    geometries
}

fn collect_window_geometries_inner(
    node: &LayoutSnapshotNode,
    out: &mut std::collections::BTreeMap<hypreact_core::WindowId, WindowGeometry>,
) {
    if let LayoutSnapshotNode::Window { window_id: Some(window_id), rect, .. } = node {
        out.insert(
            window_id.clone(),
            WindowGeometry {
                x: rect.x.round() as i32,
                y: rect.y.round() as i32,
                width: rect.width.round() as i32,
                height: rect.height.round() as i32,
            },
        );
    }

    for child in node.children() {
        collect_window_geometries_inner(child, out);
    }
}

fn focus_tree_from_geometries(
    window_geometries: &std::collections::BTreeMap<hypreact_core::WindowId, WindowGeometry>,
) -> FocusTree {
    FocusTree::from_window_geometries(
        &window_geometries
            .iter()
            .map(|(window_id, geometry)| FocusTreeWindowGeometry {
                window_id: window_id.clone(),
                geometry: *geometry,
            })
            .collect::<Vec<_>>(),
    )
}

#[allow(dead_code)]
fn geometry_candidates_from_focus_tree(
    window_geometries: &std::collections::BTreeMap<hypreact_core::WindowId, WindowGeometry>,
    focus_tree: &FocusTree,
) -> Vec<WindowGeometryCandidate> {
    window_geometries
        .iter()
        .map(|(window_id, geometry)| WindowGeometryCandidate {
            window_id: window_id.clone(),
            geometry: *geometry,
            scope_path: focus_tree.scope_path(window_id).unwrap_or(&[]).to_vec(),
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
) -> Vec<hypreact_core::WindowId> {
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
    child_windows: &[hypreact_core::WindowId],
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

fn fallback_branch_id(parent: &LayoutSnapshotNode, child: &LayoutSnapshotNode, index: usize) -> String {
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

fn partition_axis_from_style(computed: &hypreact_scene::ComputedStyle) -> Option<PartitionAxis> {
    (computed.display == Some(Display::Flex)).then(|| match computed.flex_direction {
        Some(FlexDirectionValue::Column) | Some(FlexDirectionValue::ColumnReverse) => {
            PartitionAxis::Vertical
        }
        _ => PartitionAxis::Horizontal,
    })
}

fn partition_is_adjustable(node: &LayoutSnapshotNode) -> bool {
    let Some(axis) = node.styles().and_then(|styles| partition_axis_from_style(&styles.layout)) else {
        return false;
    };

    let resizable_children =
        node.children().iter().filter(|child| !branch_is_fixed(child, Some(axis))).count();

    resizable_children >= 2
}

fn inferred_branch_constraints(node: &LayoutSnapshotNode, axis: Option<PartitionAxis>) -> PartitionConstraints {
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
    partition_rect: hypreact_core::LayoutRect,
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
