use std::path::{Path, PathBuf};

use hypreact_config::authoring_layout::{
    AuthoringLayoutService, AuthoringLayoutServiceError, PreparedLayoutEvaluation,
};
use hypreact_config::model::{Config, ConfigDiscoveryOptions, ConfigPaths, LayoutConfigError};
use hypreact_config::runtime::build_authoring_layout_service;
use hypreact_core::focus::preferred_focus_after_removing_window;
use hypreact_core::focus::{FocusTree, FocusTreeWindowGeometry};
use hypreact_core::navigation::WindowGeometryCandidate;
use hypreact_core::navigation::{NavigationDirection, select_directional_focus_candidate};
use hypreact_core::query::state_snapshot_for_model;
use hypreact_core::resize::{
    DEFAULT_BRANCH_SHARE_UNITS, DEFAULT_RESIZE_STEP_UNITS, MIN_BRANCH_SHARE_UNITS, PartitionAxis,
    PartitionBranch, PartitionConstraints, PartitionId, PartitionNode, PartitionTree,
    ResizeDirection, apply_resize_step, gc_resize_state, scale_authored_share_units,
    select_resize_candidate,
};
use hypreact_core::snapshot::{StateSnapshot, WorkspaceSnapshot};
use hypreact_core::wm::WindowGeometry;
use hypreact_core::wm::WmModel;
use hypreact_runtime_js::build_runtime_bundle;
use hypreact_scene::Display;
use hypreact_scene::FlexDirectionValue;
use hypreact_scene::ast::ValidatedLayoutTree;
use hypreact_scene::pipeline::SceneCache;
use hypreact_scene::{LayoutSnapshotNode, SceneResponse};

const DEFAULT_MIN_INFERRED_BRANCH_MAIN_SIZE_PX: f32 = 120.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct ResizeBehaviorConfig {
    step_px: f32,
    min_branch_main_size_px: f32,
}

impl ResizeBehaviorConfig {
    fn from_config(config: &Config) -> Self {
        Self {
            step_px: config.resize.step_px.unwrap_or(DEFAULT_RESIZE_STEP_UNITS as f32 * 8.0),
            min_branch_main_size_px: config
                .resize
                .min_branch_size_px
                .unwrap_or(DEFAULT_MIN_INFERRED_BRANCH_MAIN_SIZE_PX),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutRuntimePaths {
    pub config_paths: ConfigPaths,
}

impl LayoutRuntimePaths {
    pub fn discover(options: ConfigDiscoveryOptions) -> Result<Self, LayoutRuntimeError> {
        Ok(Self { config_paths: ConfigPaths::discover(options)? })
    }

    pub fn from_authored_config(authored_config: impl Into<PathBuf>) -> Self {
        let authored_config = authored_config.into();
        let prepared_parent = authored_config
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".hypreact-build");
        Self { config_paths: ConfigPaths::new(authored_config, prepared_parent.join("config.js")) }
    }
}

#[derive(Debug)]
pub struct LayoutRuntimeService {
    service: AuthoringLayoutService,
    paths: LayoutRuntimePaths,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadedLayoutConfig {
    pub config: Config,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutWorkspaceEvaluation {
    pub evaluation: PreparedLayoutEvaluation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutWorkspaceScene {
    pub evaluation: PreparedLayoutEvaluation,
    pub scene: SceneResponse,
    pub window_geometries: std::collections::BTreeMap<hypreact_core::WindowId, WindowGeometry>,
    pub focus_tree: FocusTree,
    pub partition_tree: PartitionTree,
    pub geometry_candidates: Vec<WindowGeometryCandidate>,
    pub ordered_window_ids: Vec<hypreact_core::WindowId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutStatusSnapshot {
    pub config_path: Option<String>,
    pub workspace_names: Option<Vec<String>>,
    pub loaded: bool,
    pub selected_layout_name: Option<String>,
    pub layout: Option<hypreact_core::SourceLayoutNode>,
    pub window_geometries: Vec<(hypreact_core::WindowId, WindowGeometry)>,
    pub ordered_window_ids: Vec<hypreact_core::WindowId>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResizeDebugSnapshot {
    pub workspace_id: Option<String>,
    pub focused_window_id: Option<String>,
    pub direction: String,
    pub partition_id: Option<String>,
    pub grow_branch_index: Option<usize>,
    pub shrink_branch_index: Option<usize>,
    pub changed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum LayoutRuntimeError {
    #[error(transparent)]
    Config(#[from] LayoutConfigError),
    #[error(transparent)]
    Service(#[from] AuthoringLayoutServiceError),
}

impl LayoutRuntimeService {
    pub fn new(paths: LayoutRuntimePaths) -> Result<Self, LayoutRuntimeError> {
        let service = build_authoring_layout_service(
            &paths.config_paths,
            build_runtime_bundle(&paths.config_paths)?,
        )?;
        Ok(Self { service, paths })
    }

    pub fn paths(&self) -> &LayoutRuntimePaths {
        &self.paths
    }

    pub fn load_config(&self) -> Result<LoadedLayoutConfig, LayoutRuntimeError> {
        Ok(LoadedLayoutConfig { config: self.service.load_config(&self.paths.config_paths)? })
    }

    pub fn load_authored_config(&self) -> Result<LoadedLayoutConfig, LayoutRuntimeError> {
        Ok(LoadedLayoutConfig {
            config: self.service.load_authored_config(&self.paths.config_paths)?,
        })
    }

    pub fn reload_config(&mut self) -> Result<LoadedLayoutConfig, LayoutRuntimeError> {
        Ok(LoadedLayoutConfig { config: self.service.reload_config()? })
    }

    pub fn evaluate_workspace(
        &mut self,
        config: &Config,
        state: &StateSnapshot,
        workspace: &WorkspaceSnapshot,
    ) -> Result<Option<LayoutWorkspaceEvaluation>, LayoutRuntimeError> {
        Ok(self
            .service
            .evaluate_prepared_for_workspace(config, state, workspace)?
            .map(|evaluation| LayoutWorkspaceEvaluation { evaluation }))
    }

    pub fn evaluate_workspace_scene(
        &mut self,
        config: &Config,
        state: &StateSnapshot,
        workspace: &WorkspaceSnapshot,
    ) -> Result<Option<LayoutWorkspaceScene>, LayoutRuntimeError> {
        let Some(evaluation) =
            self.service.evaluate_prepared_for_workspace(config, state, workspace)?
        else {
            return Ok(None);
        };

        let workspace_windows = state
            .windows
            .iter()
            .filter(|window| {
                window.workspace_id.as_ref() == Some(&workspace.id)
                    && workspace
                        .output_id
                        .as_ref()
                        .is_none_or(|output_id| window.output_id.as_ref() == Some(output_id))
                    && window.mapped
                    && !window.closing
                    && !window.mode.is_floating()
                    && !window.mode.is_fullscreen()
            })
            .cloned()
            .collect::<Vec<_>>();

        let resolved = ValidatedLayoutTree::new(evaluation.layout.clone())
            .map_err(|error| LayoutConfigError::EvaluateAuthoredConfig {
                path: self.paths.config_paths.authored_config.clone(),
                message: error.to_string(),
            })?
            .resolve(&workspace_windows)
            .map_err(|error| LayoutConfigError::EvaluateAuthoredConfig {
                path: self.paths.config_paths.authored_config.clone(),
                message: error.to_string(),
            })?;

        let request = config.build_scene_request(
            state,
            workspace,
            workspace
                .output_id
                .as_ref()
                .and_then(|output_id| state.output_by_id(output_id))
                .or_else(|| state.current_output()),
            resolved.root,
            &evaluation.artifact,
        )?;
        let scene = SceneCache::new().compute_layout_from_request(&request).map_err(|error| {
            LayoutConfigError::EvaluateAuthoredConfig {
                path: self.paths.config_paths.authored_config.clone(),
                message: error.to_string(),
            }
        })?;

        let resize_behavior = ResizeBehaviorConfig::from_config(config);
        let window_geometries = collect_window_geometries(&scene.root);
        let focus_tree = focus_tree_from_geometries(&window_geometries);
        let partition_tree = partition_tree_from_scene(&scene.root, resize_behavior);
        let geometry_candidates =
            geometry_candidates_from_focus_tree(&window_geometries, &focus_tree);
        let ordered_window_ids = ordered_window_ids_from_scene(&scene.root);

        Ok(Some(LayoutWorkspaceScene {
            evaluation,
            scene,
            window_geometries,
            focus_tree,
            partition_tree,
            geometry_candidates,
            ordered_window_ids,
        }))
    }
}

pub fn apply_layout_selection_to_model(model: &mut WmModel, config: &Config) {
    let current_output_id = model.current_output_id().cloned();

    for workspace in model.workspaces.values_mut() {
        workspace.effective_layout = config.selected_layout_ref_for_workspace(
            &workspace.name,
            workspace.output_id.as_ref().or(current_output_id.as_ref()),
        );
    }
}

pub fn layout_status_for_model(
    service: &mut LayoutRuntimeService,
    model: &mut WmModel,
) -> Result<LayoutStatusSnapshot, LayoutRuntimeError> {
    let config_path = Some(service.paths().config_paths.authored_config.display().to_string());
    let loaded = service.load_config()?;

    apply_layout_selection_to_model(model, &loaded.config);
    let snapshot = state_snapshot_for_model(model);
    let workspace = snapshot.current_workspace().cloned();

    let Some(workspace) = workspace else {
        return Ok(LayoutStatusSnapshot {
            config_path,
            workspace_names: Some(snapshot.workspace_names.clone()),
            loaded: true,
            selected_layout_name: None,
            layout: None,
            window_geometries: Vec::new(),
            ordered_window_ids: Vec::new(),
            error: None,
        });
    };

    match service.evaluate_workspace_scene(&loaded.config, &snapshot, &workspace) {
        Ok(evaluation) => {
            if let Some(evaluation) = evaluation.as_ref() {
                model.set_focus_tree_value(Some(evaluation.focus_tree.clone()));
            }

            Ok(LayoutStatusSnapshot {
                config_path,
                workspace_names: Some(snapshot.workspace_names.clone()),
                loaded: true,
                selected_layout_name: evaluation
                    .as_ref()
                    .map(|evaluation| evaluation.evaluation.artifact.selected.name.clone())
                    .or_else(|| {
                        workspace.effective_layout.as_ref().map(|layout| layout.name.clone())
                    }),
                layout: evaluation.as_ref().map(|evaluation| evaluation.evaluation.layout.clone()),
                window_geometries: evaluation
                    .as_ref()
                    .map(|evaluation| {
                        evaluation
                            .window_geometries
                            .iter()
                            .map(|(window_id, geometry)| (window_id.clone(), *geometry))
                            .collect()
                    })
                    .unwrap_or_default(),
                ordered_window_ids: evaluation
                    .as_ref()
                    .map(|evaluation| evaluation.ordered_window_ids.clone())
                    .unwrap_or_default(),
                error: None,
            })
        }
        Err(error) => Ok(LayoutStatusSnapshot {
            config_path,
            workspace_names: Some(snapshot.workspace_names.clone()),
            loaded: false,
            selected_layout_name: workspace
                .effective_layout
                .as_ref()
                .map(|layout| layout.name.clone()),
            layout: None,
            window_geometries: Vec::new(),
            ordered_window_ids: Vec::new(),
            error: Some(error.to_string()),
        }),
    }
}

pub fn placement_for_workspace(
    service: &mut LayoutRuntimeService,
    model: &WmModel,
    workspace_id: &str,
) -> Result<Vec<(hypreact_core::WindowId, WindowGeometry)>, LayoutRuntimeError> {
    let Some(target_workspace) =
        model.workspaces.keys().find(|id| id.as_str() == workspace_id).cloned()
    else {
        return Ok(Vec::new());
    };

    let mut model = model.clone();
    let target_output =
        model.workspaces.get(&target_workspace).and_then(|workspace| workspace.output_id.clone());

    model.set_current_workspace(target_workspace);
    if let Some(target_output) = target_output {
        model.set_current_output(target_output);
    }

    Ok(layout_status_for_model(service, &mut model)?.window_geometries)
}

pub fn directional_focus_candidate(
    service: &mut LayoutRuntimeService,
    model: &mut WmModel,
    direction: NavigationDirection,
) -> Result<Option<hypreact_core::WindowId>, LayoutRuntimeError> {
    let loaded = service.load_config()?;

    apply_layout_selection_to_model(model, &loaded.config);
    let snapshot = state_snapshot_for_model(model);
    let Some(workspace) = snapshot.current_workspace().cloned() else {
        return Ok(None);
    };
    let Some(scene) = service.evaluate_workspace_scene(&loaded.config, &snapshot, &workspace)?
    else {
        return Ok(None);
    };

    model.set_focus_tree_value(Some(scene.focus_tree.clone()));

    Ok(select_directional_focus_candidate(
        &scene.geometry_candidates,
        snapshot.focused_window_id,
        direction,
        &model.last_focused_window_id_by_scope,
        model.focus_tree.as_ref(),
    ))
}

pub fn close_focus_candidate(
    model: &WmModel,
    window_id: &hypreact_core::WindowId,
) -> Option<hypreact_core::WindowId> {
    preferred_focus_after_removing_window(model, window_id, Vec::new())
}

pub fn reset_model(model: &mut WmModel) {
    *model = WmModel::default();
}

pub fn upsert_output(
    model: &mut WmModel,
    output_id: hypreact_core::OutputId,
    name: String,
    logical_width: u32,
    logical_height: u32,
) {
    let current_workspace_id =
        model.outputs.get(&output_id).and_then(|existing| existing.focused_workspace_id.clone());
    model.upsert_output(output_id, name, logical_width, logical_height, current_workspace_id);
}

pub fn remove_output(model: &mut WmModel, output_id: &hypreact_core::OutputId) -> bool {
    let changed = model.outputs.contains_key(output_id);
    model.remove_output(output_id);
    changed
}

pub fn activate_workspace(
    model: &mut WmModel,
    workspace_id: hypreact_core::WorkspaceId,
    output_id: Option<hypreact_core::OutputId>,
) {
    let workspace_name = workspace_id.as_str().to_string();
    model.upsert_workspace(workspace_id.clone(), workspace_name);
    model.set_current_workspace(workspace_id.clone());

    if let Some(output_id) = output_id {
        model.set_current_output(output_id.clone());
        model.attach_workspace_to_output(workspace_id.clone(), output_id.clone());
        if let Some(output) = model.outputs.get_mut(&output_id) {
            output.focused_workspace_id = Some(workspace_id);
        }
    }
}

pub fn set_workspace_layout_space(
    model: &mut WmModel,
    workspace_id: hypreact_core::WorkspaceId,
    output_id: Option<hypreact_core::OutputId>,
    drawable_space: hypreact_core::wm::DrawableSpace,
) {
    model.upsert_workspace(workspace_id.clone(), workspace_id.as_str().to_string());
    if let Some(output_id) = output_id {
        model.attach_workspace_to_output(workspace_id.clone(), output_id);
    }
    model.set_workspace_layout_space(workspace_id, Some(drawable_space));
}

pub fn focus_window(model: &mut WmModel, window_id: Option<hypreact_core::WindowId>) {
    model.set_window_focused(window_id);
}

pub fn set_window_closing(
    model: &mut WmModel,
    window_id: &hypreact_core::WindowId,
    closing: bool,
) -> bool {
    let changed = model.windows.contains_key(window_id);
    if changed {
        model.set_window_closing(window_id.clone(), closing);
    }
    changed
}

pub fn remove_window(
    model: &mut WmModel,
    window_id: hypreact_core::WindowId,
) -> (bool, Option<hypreact_core::WindowId>) {
    let changed = model.windows.contains_key(&window_id);
    let update = hypreact_core::focus::remove_window(model, window_id, Vec::new());
    let focused_window_id = match update {
        hypreact_core::focus::FocusUpdate::Set(window_id) => window_id,
        hypreact_core::focus::FocusUpdate::Unchanged => None,
    };
    (changed, focused_window_id)
}

pub fn upsert_window(
    model: &mut WmModel,
    window_id: hypreact_core::WindowId,
    workspace_id: Option<hypreact_core::WorkspaceId>,
    output_id: Option<hypreact_core::OutputId>,
    is_xwayland: bool,
    mapped: bool,
    title: Option<String>,
    app_id: Option<String>,
    class: Option<String>,
    instance: Option<String>,
    role: Option<String>,
    window_type: Option<String>,
    urgent: bool,
    floating: bool,
    fullscreen: bool,
) -> bool {
    if !mapped {
        let changed = model.windows.contains_key(&window_id);
        if changed {
            model.remove_window(window_id);
        }
        return changed;
    }

    if let Some(workspace_id) = workspace_id.as_ref() {
        model.upsert_workspace(workspace_id.clone(), workspace_id.as_str().to_string());
    }

    let existed = model.windows.contains_key(&window_id);
    if !existed {
        model.insert_window(window_id.clone(), workspace_id.clone(), output_id.clone());
    }

    if let Some(window_model) = model.windows.get_mut(&window_id) {
        window_model.is_xwayland = is_xwayland;
        window_model.workspace_id = workspace_id;
        window_model.output_id = output_id;
        window_model.mapped = mapped;
        window_model.title = title;
        window_model.app_id = app_id;
        window_model.class = class;
        window_model.instance = instance;
        window_model.role = role;
        window_model.window_type = window_type;
        window_model.urgent = urgent;
        window_model.floating = floating;
        window_model.fullscreen = fullscreen;
    }

    true
}

pub fn move_tiled_window(
    model: &mut WmModel,
    first_window_id: &hypreact_core::WindowId,
    second_window_id: &hypreact_core::WindowId,
) -> bool {
    model.move_tiled_window(first_window_id, second_window_id)
}

pub fn resize_direction(
    service: &mut LayoutRuntimeService,
    model: &mut WmModel,
    direction: ResizeDirection,
) -> Result<bool, LayoutRuntimeError> {
    Ok(resize_direction_debug(service, model, direction)?.changed)
}

pub fn resize_direction_debug(
    service: &mut LayoutRuntimeService,
    model: &mut WmModel,
    direction: ResizeDirection,
) -> Result<ResizeDebugSnapshot, LayoutRuntimeError> {
    let Some(workspace_id) = model.current_workspace_id().cloned() else {
        return Ok(ResizeDebugSnapshot {
            workspace_id: None,
            focused_window_id: model.focused_window_id().map(|id| id.to_string()),
            direction: format!("{:?}", direction).to_lowercase(),
            partition_id: None,
            grow_branch_index: None,
            shrink_branch_index: None,
            changed: false,
        });
    };
    let Some(focused_window_id) = model.focused_window_id().cloned() else {
        return Ok(ResizeDebugSnapshot {
            workspace_id: Some(workspace_id.to_string()),
            focused_window_id: None,
            direction: format!("{:?}", direction).to_lowercase(),
            partition_id: None,
            grow_branch_index: None,
            shrink_branch_index: None,
            changed: false,
        });
    };

    let loaded = service.load_config()?;
    apply_layout_selection_to_model(model, &loaded.config);
    let snapshot = state_snapshot_for_model(model);
    let Some(workspace) = snapshot.current_workspace().cloned() else {
        return Ok(ResizeDebugSnapshot {
            workspace_id: Some(workspace_id.to_string()),
            focused_window_id: Some(focused_window_id.to_string()),
            direction: format!("{:?}", direction).to_lowercase(),
            partition_id: None,
            grow_branch_index: None,
            shrink_branch_index: None,
            changed: false,
        });
    };
    let Some(scene) = service.evaluate_workspace_scene(&loaded.config, &snapshot, &workspace)?
    else {
        return Ok(ResizeDebugSnapshot {
            workspace_id: Some(workspace_id.to_string()),
            focused_window_id: Some(focused_window_id.to_string()),
            direction: format!("{:?}", direction).to_lowercase(),
            partition_id: None,
            grow_branch_index: None,
            shrink_branch_index: None,
            changed: false,
        });
    };

    let resize_state = model.workspace_resize_state_mut(&workspace_id);
    gc_resize_state(resize_state, &scene.partition_tree);
    let Some(candidate) =
        select_resize_candidate(&scene.partition_tree, &focused_window_id, direction)
    else {
        return Ok(ResizeDebugSnapshot {
            workspace_id: Some(workspace_id.to_string()),
            focused_window_id: Some(focused_window_id.to_string()),
            direction: format!("{:?}", direction).to_lowercase(),
            partition_id: None,
            grow_branch_index: None,
            shrink_branch_index: None,
            changed: false,
        });
    };

    let resize_behavior = ResizeBehaviorConfig::from_config(&loaded.config);
    let step_units = resize_step_units_for_partition(
        &scene.partition_tree,
        &candidate.partition_id,
        resize_behavior.step_px,
    );
    let changed = apply_resize_step(resize_state, &scene.partition_tree, &candidate, step_units);

    Ok(ResizeDebugSnapshot {
        workspace_id: Some(workspace_id.to_string()),
        focused_window_id: Some(focused_window_id.to_string()),
        direction: format!("{:?}", direction).to_lowercase(),
        partition_id: Some(candidate.partition_id.0),
        grow_branch_index: Some(candidate.grow_branch_index),
        shrink_branch_index: Some(candidate.shrink_branch_index),
        changed,
    })
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
    geometries: &std::collections::BTreeMap<hypreact_core::WindowId, WindowGeometry>,
) -> FocusTree {
    let entries = geometries
        .iter()
        .map(|(window_id, geometry)| FocusTreeWindowGeometry {
            window_id: window_id.clone(),
            geometry: *geometry,
        })
        .collect::<Vec<_>>();

    FocusTree::from_window_geometries(&entries)
}

fn geometry_candidates_from_focus_tree(
    geometries: &std::collections::BTreeMap<hypreact_core::WindowId, WindowGeometry>,
    focus_tree: &FocusTree,
) -> Vec<WindowGeometryCandidate> {
    let entries = geometries
        .iter()
        .map(|(window_id, geometry)| FocusTreeWindowGeometry {
            window_id: window_id.clone(),
            geometry: *geometry,
        })
        .collect::<Vec<_>>();

    entries
        .into_iter()
        .map(|entry| WindowGeometryCandidate {
            scope_path: focus_tree
                .scope_path(&entry.window_id)
                .map(|scope_path| scope_path.to_vec())
                .unwrap_or_else(|| vec![FocusTree::workspace_scope()]),
            window_id: entry.window_id,
            geometry: entry.geometry,
        })
        .collect::<Vec<_>>()
}

fn ordered_window_ids_from_scene(root: &LayoutSnapshotNode) -> Vec<hypreact_core::WindowId> {
    let mut ids = Vec::new();
    collect_ordered_window_ids(root, &mut ids);
    ids
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

fn partition_axis_from_style(computed: &hypreact_scene::ComputedStyle) -> Option<PartitionAxis> {
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

fn resize_step_units_for_partition(
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

    if matches!(explicit_main_size, Some(hypreact_scene::SizeValue::LengthPercentage(_))) {
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

fn collect_ordered_window_ids(node: &LayoutSnapshotNode, out: &mut Vec<hypreact_core::WindowId>) {
    if let LayoutSnapshotNode::Window { window_id: Some(window_id), .. } = node {
        out.push(window_id.clone());
        return;
    }

    for child in node.children() {
        collect_ordered_window_ids(child, out);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use hypreact_core::WindowId;
    use hypreact_core::focus::FocusScopePath;
    use hypreact_core::navigation::{NavigationDirection, select_directional_focus_candidate};
    use hypreact_core::query::state_snapshot_for_model;
    use hypreact_core::wm::WmModel;
    use hypreact_core::{OutputId, WorkspaceId};

    use super::*;

    #[test]
    fn geometry_candidates_preserve_branch_memory_for_master_stack_focus() {
        let geometries = BTreeMap::from([
            (WindowId::from("master"), WindowGeometry { x: 0, y: 0, width: 600, height: 900 }),
            (WindowId::from("stack-1"), WindowGeometry { x: 600, y: 0, width: 300, height: 300 }),
            (WindowId::from("stack-2"), WindowGeometry { x: 600, y: 300, width: 300, height: 300 }),
            (WindowId::from("stack-3"), WindowGeometry { x: 600, y: 600, width: 300, height: 300 }),
        ]);

        let focus_tree = focus_tree_from_geometries(&geometries);
        let candidates = geometry_candidates_from_focus_tree(&geometries, &focus_tree);
        let mut remembered = BTreeMap::<FocusScopePath, WindowId>::new();
        let stack_three = WindowId::from("stack-3");
        let master = WindowId::from("master");

        let stack_scope_path = candidates
            .iter()
            .find(|candidate| candidate.window_id == stack_three)
            .map(|candidate| candidate.scope_path.clone())
            .expect("stack window candidate");
        for scope_key in stack_scope_path {
            remembered.insert(scope_key, stack_three.clone());
        }

        assert_eq!(
            select_directional_focus_candidate(
                &candidates,
                Some(master),
                NavigationDirection::Right,
                &remembered,
                None,
            ),
            Some(stack_three)
        );
    }

    #[test]
    fn workspace_scene_builds_focus_tree_for_only_current_workspace_windows() {
        let config_path = "/home/akisarou/projects/hypreact/test_config/config.ts";
        let mut service =
            LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(config_path))
                .expect("layout runtime service");
        let loaded = service.load_config().expect("loaded config");

        let mut model = WmModel::default();
        model.upsert_output(OutputId::from("eDP-1"), "eDP-1".to_string(), 1600, 1000, None);

        for workspace in ["1", "2"] {
            model.upsert_workspace(WorkspaceId::from(workspace), workspace.to_string());
            model.attach_workspace_to_output(WorkspaceId::from(workspace), OutputId::from("eDP-1"));
        }

        model.set_current_output(OutputId::from("eDP-1"));
        model.set_current_workspace(WorkspaceId::from("1"));

        for id in ["w1-a", "w1-b"] {
            let window_id = WindowId::from(id.to_string());
            model.insert_window(
                window_id.clone(),
                Some(WorkspaceId::from("1")),
                Some(OutputId::from("eDP-1")),
            );
            model.set_window_mapped(window_id, true);
        }

        for id in ["w2-a", "w2-b"] {
            let window_id = WindowId::from(id.to_string());
            model.insert_window(
                window_id.clone(),
                Some(WorkspaceId::from("2")),
                Some(OutputId::from("eDP-1")),
            );
            model.set_window_mapped(window_id, true);
        }

        apply_layout_selection_to_model(&mut model, &loaded.config);

        let snapshot = state_snapshot_for_model(&model);
        let workspace = snapshot.current_workspace().expect("current workspace");
        let scene = service
            .evaluate_workspace_scene(&loaded.config, &snapshot, workspace)
            .expect("scene evaluation")
            .expect("workspace scene");

        assert!(scene.focus_tree.contains_window(&WindowId::from("w1-a")));
        assert!(scene.focus_tree.contains_window(&WindowId::from("w1-b")));
        assert!(!scene.focus_tree.contains_window(&WindowId::from("w2-a")));
        assert!(!scene.focus_tree.contains_window(&WindowId::from("w2-b")));
    }

    #[test]
    fn move_tiled_window_changes_master_stack_placement_order() {
        let config_path = "/home/akisarou/projects/hypreact/test_config/config.ts";
        let mut service =
            LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(config_path))
                .expect("layout runtime service");

        let mut model = WmModel::default();
        model.upsert_output(
            OutputId::from("eDP-1"),
            "eDP-1".to_string(),
            1600,
            1000,
            Some(WorkspaceId::from("1")),
        );
        model.upsert_workspace(WorkspaceId::from("1"), "1".to_string());
        model.attach_workspace_to_output(WorkspaceId::from("1"), OutputId::from("eDP-1"));
        model.set_workspace_layout_space(
            WorkspaceId::from("1"),
            Some(hypreact_core::wm::DrawableSpace { width: 1600, height: 1000 }),
        );
        model.set_current_output(OutputId::from("eDP-1"));
        model.set_current_workspace(WorkspaceId::from("1"));

        for id in ["master", "stack"] {
            let window_id = WindowId::from(id.to_string());
            model.insert_window(
                window_id.clone(),
                Some(WorkspaceId::from("1")),
                Some(OutputId::from("eDP-1")),
            );
            model.set_window_mapped(window_id, true);
        }

        let initial = placement_for_workspace(&mut service, &model, "1")
            .expect("initial placement")
            .into_iter()
            .collect::<BTreeMap<_, _>>();

        assert!(initial[&WindowId::from("master")].x < initial[&WindowId::from("stack")].x);

        assert!(
            move_tiled_window(&mut model, &WindowId::from("master"), &WindowId::from("stack"),)
        );

        let moved = placement_for_workspace(&mut service, &model, "1")
            .expect("moved placement")
            .into_iter()
            .collect::<BTreeMap<_, _>>();

        assert!(moved[&WindowId::from("master")].x > moved[&WindowId::from("stack")].x);
    }

    #[test]
    fn workspace_scene_derives_partition_tree_for_master_stack_layout() {
        let config_path = "/home/akisarou/projects/hypreact/test_config/config.ts";
        let mut service =
            LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(config_path))
                .expect("layout runtime service");
        let loaded = service.load_config().expect("loaded config");

        let mut model = WmModel::default();
        model.upsert_output(
            OutputId::from("eDP-1"),
            "eDP-1".to_string(),
            1600,
            1000,
            Some(WorkspaceId::from("1")),
        );
        model.upsert_workspace(WorkspaceId::from("1"), "1".to_string());
        model.attach_workspace_to_output(WorkspaceId::from("1"), OutputId::from("eDP-1"));
        model.set_workspace_layout_space(
            WorkspaceId::from("1"),
            Some(hypreact_core::wm::DrawableSpace { width: 1600, height: 1000 }),
        );
        model.set_current_output(OutputId::from("eDP-1"));
        model.set_current_workspace(WorkspaceId::from("1"));

        for id in ["master", "stack-a", "stack-b"] {
            let window_id = WindowId::from(id.to_string());
            model.insert_window(
                window_id.clone(),
                Some(WorkspaceId::from("1")),
                Some(OutputId::from("eDP-1")),
            );
            model.set_window_mapped(window_id, true);
        }

        apply_layout_selection_to_model(&mut model, &loaded.config);
        let snapshot = state_snapshot_for_model(&model);
        let workspace = snapshot.current_workspace().expect("current workspace");
        let scene = service
            .evaluate_workspace_scene(&loaded.config, &snapshot, workspace)
            .expect("scene evaluation")
            .expect("workspace scene");

        assert!(scene.partition_tree.partitions.contains_key(&PartitionId::new("frame")));
        let frame = &scene.partition_tree.partitions[&PartitionId::new("frame")];
        assert_eq!(frame.axis, PartitionAxis::Horizontal);
        assert_eq!(frame.branches.len(), 2);
        assert!(
            scene
                .partition_tree
                .window_to_partition_path
                .get(&WindowId::from("master"))
                .is_some_and(|path| path == &vec![PartitionId::new("frame")])
        );
    }

    #[test]
    fn workspace_scene_tracks_nested_explicit_partitions() {
        let scene = LayoutSnapshotNode::Workspace {
            meta: hypreact_core::LayoutNodeMeta { id: Some("frame".into()), ..Default::default() },
            rect: hypreact_core::LayoutRect { x: 0.0, y: 0.0, width: 1600.0, height: 1000.0 },
            styles: Some(hypreact_scene::SceneNodeStyle {
                layout: hypreact_scene::ComputedStyle {
                    display: Some(Display::Flex),
                    flex_direction: Some(FlexDirectionValue::Row),
                    ..Default::default()
                },
            }),
            children: vec![
                LayoutSnapshotNode::Window {
                    meta: hypreact_core::LayoutNodeMeta {
                        id: Some("master".into()),
                        ..Default::default()
                    },
                    rect: hypreact_core::LayoutRect {
                        x: 0.0,
                        y: 0.0,
                        width: 960.0,
                        height: 1000.0,
                    },
                    styles: Some(hypreact_scene::SceneNodeStyle {
                        layout: hypreact_scene::ComputedStyle {
                            flex_grow: Some(3.0),
                            ..Default::default()
                        },
                    }),
                    window_id: Some(WindowId::from("master")),
                    children: vec![],
                },
                LayoutSnapshotNode::Group {
                    meta: hypreact_core::LayoutNodeMeta::default(),
                    rect: hypreact_core::LayoutRect {
                        x: 960.0,
                        y: 0.0,
                        width: 640.0,
                        height: 1000.0,
                    },
                    styles: Some(hypreact_scene::SceneNodeStyle {
                        layout: hypreact_scene::ComputedStyle {
                            display: Some(Display::Flex),
                            flex_direction: Some(FlexDirectionValue::Column),
                            flex_grow: Some(2.0),
                            ..Default::default()
                        },
                    }),
                    children: vec![
                        LayoutSnapshotNode::Window {
                            meta: hypreact_core::LayoutNodeMeta {
                                id: Some("stack-a".into()),
                                ..Default::default()
                            },
                            rect: hypreact_core::LayoutRect {
                                x: 960.0,
                                y: 0.0,
                                width: 640.0,
                                height: 500.0,
                            },
                            styles: Some(hypreact_scene::SceneNodeStyle {
                                layout: hypreact_scene::ComputedStyle {
                                    flex_grow: Some(1.0),
                                    ..Default::default()
                                },
                            }),
                            window_id: Some(WindowId::from("stack-a")),
                            children: vec![],
                        },
                        LayoutSnapshotNode::Window {
                            meta: hypreact_core::LayoutNodeMeta {
                                id: Some("stack-b".into()),
                                ..Default::default()
                            },
                            rect: hypreact_core::LayoutRect {
                                x: 960.0,
                                y: 500.0,
                                width: 640.0,
                                height: 500.0,
                            },
                            styles: Some(hypreact_scene::SceneNodeStyle {
                                layout: hypreact_scene::ComputedStyle {
                                    flex_grow: Some(1.0),
                                    ..Default::default()
                                },
                            }),
                            window_id: Some(WindowId::from("stack-b")),
                            children: vec![],
                        },
                    ],
                },
            ],
        };

        let partition_tree = partition_tree_from_scene(
            &scene,
            ResizeBehaviorConfig {
                step_px: DEFAULT_RESIZE_STEP_UNITS as f32 * 8.0,
                min_branch_main_size_px: DEFAULT_MIN_INFERRED_BRANCH_MAIN_SIZE_PX,
            },
        );

        assert!(partition_tree.partitions.contains_key(&PartitionId::new("frame")));
        let stack_path = partition_tree
            .window_to_partition_path
            .get(&WindowId::from("stack-a"))
            .expect("nested stack path");
        assert_eq!(stack_path.len(), 2);
        assert_eq!(stack_path[0], PartitionId::new("frame"));
        let nested_partition_id = stack_path[1].clone();
        let nested_partition =
            partition_tree.partitions.get(&nested_partition_id).expect("nested partition");
        assert_eq!(nested_partition.axis, PartitionAxis::Vertical);
        assert_eq!(nested_partition.branches.len(), 2);
        assert_eq!(
            partition_tree.window_to_partition_path.get(&WindowId::from("stack-b")),
            Some(&vec![PartitionId::new("frame"), nested_partition_id.clone()])
        );
        assert_eq!(
            select_resize_candidate(
                &partition_tree,
                &WindowId::from("stack-a"),
                ResizeDirection::Down,
            ),
            Some(hypreact_core::resize::ResizeCandidate {
                partition_id: nested_partition_id,
                grow_branch_index: 0,
                shrink_branch_index: 1,
            })
        );
    }

    #[test]
    fn resize_direction_updates_workspace_resize_state() {
        let config_path = "/home/akisarou/projects/hypreact/test_config/config.ts";
        let mut service =
            LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(config_path))
                .expect("layout runtime service");

        let mut model = WmModel::default();
        model.upsert_output(
            OutputId::from("eDP-1"),
            "eDP-1".to_string(),
            1600,
            1000,
            Some(WorkspaceId::from("1")),
        );
        model.upsert_workspace(WorkspaceId::from("1"), "1".to_string());
        model.attach_workspace_to_output(WorkspaceId::from("1"), OutputId::from("eDP-1"));
        model.set_workspace_layout_space(
            WorkspaceId::from("1"),
            Some(hypreact_core::wm::DrawableSpace { width: 1600, height: 1000 }),
        );
        model.set_current_output(OutputId::from("eDP-1"));
        model.set_current_workspace(WorkspaceId::from("1"));

        for id in ["master", "stack"] {
            let window_id = WindowId::from(id.to_string());
            model.insert_window(
                window_id.clone(),
                Some(WorkspaceId::from("1")),
                Some(OutputId::from("eDP-1")),
            );
            model.set_window_mapped(window_id, true);
        }
        model.set_window_focused(Some(WindowId::from("master")));

        assert!(
            resize_direction(
                &mut service,
                &mut model,
                hypreact_core::resize::ResizeDirection::Right,
            )
            .expect("resize result")
        );

        let resize_state = model.workspace_resize_state(&WorkspaceId::from("1"));
        assert_eq!(
            resize_state.adjustments_by_partition_id[&PartitionId::new("frame")].branch_shares,
            vec![40, 20]
        );

        model.set_window_focused(Some(WindowId::from("master")));
        assert!(
            resize_direction(
                &mut service,
                &mut model,
                hypreact_core::resize::ResizeDirection::Left,
            )
            .expect("reverse resize result")
        );

        let resize_state = model.workspace_resize_state(&WorkspaceId::from("1"));
        assert_eq!(
            resize_state.adjustments_by_partition_id[&PartitionId::new("frame")].branch_shares,
            vec![36, 24]
        );

        model.set_window_focused(Some(WindowId::from("stack")));
        assert!(
            resize_direction(
                &mut service,
                &mut model,
                hypreact_core::resize::ResizeDirection::Right,
            )
            .expect("stack right resize result")
        );

        let resize_state = model.workspace_resize_state(&WorkspaceId::from("1"));
        assert_eq!(
            resize_state.adjustments_by_partition_id[&PartitionId::new("frame")].branch_shares,
            vec![40, 20]
        );

        assert!(
            resize_direction(
                &mut service,
                &mut model,
                hypreact_core::resize::ResizeDirection::Left,
            )
            .expect("stack left resize result")
        );

        let resize_state = model.workspace_resize_state(&WorkspaceId::from("1"));
        assert_eq!(
            resize_state.adjustments_by_partition_id[&PartitionId::new("frame")].branch_shares,
            vec![36, 24]
        );
    }

    #[test]
    fn resize_direction_updates_nested_stack_partition_state() {
        let config_path = "/home/akisarou/projects/hypreact/test_config/config.ts";
        let mut service =
            LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(config_path))
                .expect("layout runtime service");
        let loaded = service.load_config().expect("loaded config");

        let mut model = WmModel::default();
        model.upsert_output(
            OutputId::from("eDP-1"),
            "eDP-1".to_string(),
            1600,
            1000,
            Some(WorkspaceId::from("1")),
        );
        model.upsert_workspace(WorkspaceId::from("1"), "1".to_string());
        model.attach_workspace_to_output(WorkspaceId::from("1"), OutputId::from("eDP-1"));
        model.set_workspace_layout_space(
            WorkspaceId::from("1"),
            Some(hypreact_core::wm::DrawableSpace { width: 1600, height: 1000 }),
        );
        model.set_current_output(OutputId::from("eDP-1"));
        model.set_current_workspace(WorkspaceId::from("1"));

        for id in ["master", "stack-a", "stack-b", "stack-c", "stack-d", "stack-e"] {
            let window_id = WindowId::from(id.to_string());
            model.insert_window(
                window_id.clone(),
                Some(WorkspaceId::from("1")),
                Some(OutputId::from("eDP-1")),
            );
            model.set_window_mapped(window_id, true);
        }
        model.set_window_focused(Some(WindowId::from("stack-c")));

        apply_layout_selection_to_model(&mut model, &loaded.config);
        let snapshot = state_snapshot_for_model(&model);
        let workspace = snapshot.current_workspace().expect("current workspace");
        let scene = service
            .evaluate_workspace_scene(&loaded.config, &snapshot, workspace)
            .expect("scene evaluation")
            .expect("workspace scene");
        assert!(scene.partition_tree.partitions.len() >= 2);
        assert!(
            scene
                .partition_tree
                .window_to_partition_path
                .get(&WindowId::from("stack-c"))
                .is_some_and(|path| path.len() >= 2)
        );
        assert!(
            select_resize_candidate(
                &scene.partition_tree,
                &WindowId::from("stack-c"),
                hypreact_core::resize::ResizeDirection::Down,
            )
            .is_some()
        );

        assert!(
            resize_direction(
                &mut service,
                &mut model,
                hypreact_core::resize::ResizeDirection::Down,
            )
            .expect("resize result")
        );

        let resize_state = model.workspace_resize_state(&WorkspaceId::from("1"));
        let nested_adjustment = resize_state
            .adjustments_by_partition_id
            .iter()
            .find(|(partition_id, _)| partition_id.0 != "frame")
            .map(|(_, adjustment)| adjustment)
            .expect("nested stack partition adjustment");
        assert_eq!(nested_adjustment.branch_shares.len(), 5);
        assert_eq!(nested_adjustment.branch_shares, vec![12, 12, 16, 8, 12]);

        assert!(
            resize_direction(&mut service, &mut model, hypreact_core::resize::ResizeDirection::Up,)
                .expect("reverse vertical resize result")
        );

        let resize_state = model.workspace_resize_state(&WorkspaceId::from("1"));
        let nested_adjustment = resize_state
            .adjustments_by_partition_id
            .iter()
            .find(|(partition_id, _)| partition_id.0 != "frame")
            .map(|(_, adjustment)| adjustment)
            .expect("nested stack partition adjustment after reverse resize");
        assert_eq!(nested_adjustment.branch_shares, vec![12, 8, 20, 8, 12]);
    }

    #[test]
    fn repeated_vertical_resize_stops_before_collapsing_stack_branches() {
        let config_path = "/home/akisarou/projects/hypreact/test_config/config.ts";
        let mut service =
            LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(config_path))
                .expect("layout runtime service");

        let mut model = WmModel::default();
        model.upsert_output(
            OutputId::from("eDP-1"),
            "eDP-1".to_string(),
            1600,
            1000,
            Some(WorkspaceId::from("1")),
        );
        model.upsert_workspace(WorkspaceId::from("1"), "1".to_string());
        model.attach_workspace_to_output(WorkspaceId::from("1"), OutputId::from("eDP-1"));
        model.set_workspace_layout_space(
            WorkspaceId::from("1"),
            Some(hypreact_core::wm::DrawableSpace { width: 1600, height: 1000 }),
        );
        model.set_current_output(OutputId::from("eDP-1"));
        model.set_current_workspace(WorkspaceId::from("1"));

        for id in ["master", "stack-a", "stack-b", "stack-c", "stack-d"] {
            let window_id = WindowId::from(id.to_string());
            model.insert_window(
                window_id.clone(),
                Some(WorkspaceId::from("1")),
                Some(OutputId::from("eDP-1")),
            );
            model.set_window_mapped(window_id, true);
        }
        model.set_window_focused(Some(WindowId::from("stack-b")));

        let _ = service.reload_config().expect("reloaded config");

        while resize_direction(&mut service, &mut model, hypreact_core::resize::ResizeDirection::Up)
            .expect("vertical resize result")
        {}

        let resize_state = model.workspace_resize_state(&WorkspaceId::from("1"));
        let nested_adjustment = resize_state
            .adjustments_by_partition_id
            .iter()
            .find(|(partition_id, _)| partition_id.0 != "frame")
            .map(|(_, adjustment)| adjustment)
            .expect("nested stack partition adjustment after repeated resize");

        assert!(nested_adjustment.branch_shares.iter().all(|share| *share >= 6));
    }

    #[test]
    fn resize_direction_matches_live_four_window_stack_focus_sequence() {
        let config_path = "/home/akisarou/projects/hypreact/test_config/config.ts";
        let mut service =
            LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(config_path))
                .expect("layout runtime service");

        let mut model = WmModel::default();
        model.upsert_output(
            OutputId::from("eDP-1"),
            "eDP-1".to_string(),
            1600,
            1000,
            Some(WorkspaceId::from("1")),
        );
        model.upsert_workspace(WorkspaceId::from("1"), "1".to_string());
        model.attach_workspace_to_output(WorkspaceId::from("1"), OutputId::from("eDP-1"));
        model.set_workspace_layout_space(
            WorkspaceId::from("1"),
            Some(hypreact_core::wm::DrawableSpace { width: 1600, height: 1000 }),
        );
        model.set_current_output(OutputId::from("eDP-1"));
        model.set_current_workspace(WorkspaceId::from("1"));

        for id in ["master", "stack-a", "stack-b", "stack-c"] {
            let window_id = WindowId::from(id.to_string());
            model.insert_window(
                window_id.clone(),
                Some(WorkspaceId::from("1")),
                Some(OutputId::from("eDP-1")),
            );
            model.set_window_mapped(window_id, true);
        }

        model.set_window_focused(Some(WindowId::from("stack-c")));
        let debug = resize_direction_debug(
            &mut service,
            &mut model,
            hypreact_core::resize::ResizeDirection::Right,
        )
        .expect("stack right resize debug");
        assert_eq!(
            debug,
            ResizeDebugSnapshot {
                workspace_id: Some("1".into()),
                focused_window_id: Some("stack-c".into()),
                direction: "right".into(),
                partition_id: Some("frame".into()),
                grow_branch_index: Some(0),
                shrink_branch_index: Some(1),
                changed: true,
            }
        );

        let resize_state = model.workspace_resize_state(&WorkspaceId::from("1"));
        assert_eq!(
            resize_state.adjustments_by_partition_id[&PartitionId::new("frame")].branch_shares,
            vec![40, 20]
        );

        let debug = resize_direction_debug(
            &mut service,
            &mut model,
            hypreact_core::resize::ResizeDirection::Left,
        )
        .expect("stack left resize debug");
        assert_eq!(
            debug,
            ResizeDebugSnapshot {
                workspace_id: Some("1".into()),
                focused_window_id: Some("stack-c".into()),
                direction: "left".into(),
                partition_id: Some("frame".into()),
                grow_branch_index: Some(1),
                shrink_branch_index: Some(0),
                changed: true,
            }
        );

        let resize_state = model.workspace_resize_state(&WorkspaceId::from("1"));
        assert_eq!(
            resize_state.adjustments_by_partition_id[&PartitionId::new("frame")].branch_shares,
            vec![36, 24]
        );
    }

    #[test]
    fn resize_direction_allows_horizontal_resize_for_top_stack_window() {
        let config_path = "/home/akisarou/projects/hypreact/test_config/config.ts";
        let mut service =
            LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(config_path))
                .expect("layout runtime service");

        let mut model = WmModel::default();
        model.upsert_output(
            OutputId::from("eDP-1"),
            "eDP-1".to_string(),
            1600,
            1000,
            Some(WorkspaceId::from("1")),
        );
        model.upsert_workspace(WorkspaceId::from("1"), "1".to_string());
        model.attach_workspace_to_output(WorkspaceId::from("1"), OutputId::from("eDP-1"));
        model.set_workspace_layout_space(
            WorkspaceId::from("1"),
            Some(hypreact_core::wm::DrawableSpace { width: 1600, height: 1000 }),
        );
        model.set_current_output(OutputId::from("eDP-1"));
        model.set_current_workspace(WorkspaceId::from("1"));

        for id in ["master", "stack-a", "stack-b", "stack-c"] {
            let window_id = WindowId::from(id.to_string());
            model.insert_window(
                window_id.clone(),
                Some(WorkspaceId::from("1")),
                Some(OutputId::from("eDP-1")),
            );
            model.set_window_mapped(window_id, true);
        }

        model.set_window_focused(Some(WindowId::from("stack-a")));
        let debug = resize_direction_debug(
            &mut service,
            &mut model,
            hypreact_core::resize::ResizeDirection::Right,
        )
        .expect("top stack right resize debug");

        assert_eq!(
            debug,
            ResizeDebugSnapshot {
                workspace_id: Some("1".into()),
                focused_window_id: Some("stack-a".into()),
                direction: "right".into(),
                partition_id: Some("frame".into()),
                grow_branch_index: Some(0),
                shrink_branch_index: Some(1),
                changed: true,
            }
        );
    }

    #[test]
    fn resize_direction_respects_fixed_branch_constraints() {
        let config_path = "/home/akisarou/projects/hypreact/test_config/config.ts";
        let mut service =
            LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(config_path))
                .expect("layout runtime service");

        let mut model = WmModel::default();
        for workspace_name in ["1", "2", "3", "4", "5", "6"] {
            model.upsert_workspace(WorkspaceId::from(workspace_name), workspace_name.to_string());
        }
        model.upsert_output(
            OutputId::from("eDP-1"),
            "eDP-1".to_string(),
            1600,
            1000,
            Some(WorkspaceId::from("6")),
        );
        model.attach_workspace_to_output(WorkspaceId::from("6"), OutputId::from("eDP-1"));
        model.set_workspace_layout_space(
            WorkspaceId::from("6"),
            Some(hypreact_core::wm::DrawableSpace { width: 1600, height: 1000 }),
        );
        model.set_current_output(OutputId::from("eDP-1"));
        model.set_current_workspace(WorkspaceId::from("6"));

        for id in ["master", "stack"] {
            let window_id = WindowId::from(id.to_string());
            model.insert_window(
                window_id.clone(),
                Some(WorkspaceId::from("6")),
                Some(OutputId::from("eDP-1")),
            );
            model.set_window_mapped(window_id, true);
        }
        model.set_window_focused(Some(WindowId::from("stack")));

        assert!(
            !resize_direction(
                &mut service,
                &mut model,
                hypreact_core::resize::ResizeDirection::Left,
            )
            .expect("resize result")
        );

        let resize_state = model.workspace_resize_state(&WorkspaceId::from("6"));
        assert!(resize_state.adjustments_by_partition_id.is_empty());
    }

    #[test]
    fn scene_resize_adjustments_survive_branch_reorder_and_insertion_by_branch_id() {
        let partition_id = PartitionId::new("frame");
        let initial_scene = LayoutSnapshotNode::Workspace {
            meta: hypreact_core::LayoutNodeMeta { id: Some("frame".into()), ..Default::default() },
            rect: hypreact_core::LayoutRect { x: 0.0, y: 0.0, width: 1600.0, height: 1000.0 },
            styles: Some(hypreact_scene::SceneNodeStyle {
                layout: hypreact_scene::ComputedStyle {
                    display: Some(Display::Flex),
                    flex_direction: Some(FlexDirectionValue::Row),
                    ..Default::default()
                },
            }),
            children: vec![
                LayoutSnapshotNode::Window {
                    meta: hypreact_core::LayoutNodeMeta {
                        id: Some("master".into()),
                        ..Default::default()
                    },
                    rect: hypreact_core::LayoutRect {
                        x: 0.0,
                        y: 0.0,
                        width: 960.0,
                        height: 1000.0,
                    },
                    styles: Some(hypreact_scene::SceneNodeStyle {
                        layout: hypreact_scene::ComputedStyle {
                            flex_grow: Some(3.0),
                            ..Default::default()
                        },
                    }),
                    window_id: Some(WindowId::from("master")),
                    children: vec![],
                },
                LayoutSnapshotNode::Window {
                    meta: hypreact_core::LayoutNodeMeta {
                        id: Some("stack".into()),
                        ..Default::default()
                    },
                    rect: hypreact_core::LayoutRect {
                        x: 960.0,
                        y: 0.0,
                        width: 640.0,
                        height: 1000.0,
                    },
                    styles: Some(hypreact_scene::SceneNodeStyle {
                        layout: hypreact_scene::ComputedStyle {
                            flex_grow: Some(2.0),
                            ..Default::default()
                        },
                    }),
                    window_id: Some(WindowId::from("stack")),
                    children: vec![],
                },
            ],
        };
        let reordered_scene = LayoutSnapshotNode::Workspace {
            meta: hypreact_core::LayoutNodeMeta { id: Some("frame".into()), ..Default::default() },
            rect: hypreact_core::LayoutRect { x: 0.0, y: 0.0, width: 1600.0, height: 1000.0 },
            styles: Some(hypreact_scene::SceneNodeStyle {
                layout: hypreact_scene::ComputedStyle {
                    display: Some(Display::Flex),
                    flex_direction: Some(FlexDirectionValue::Row),
                    ..Default::default()
                },
            }),
            children: vec![
                LayoutSnapshotNode::Window {
                    meta: hypreact_core::LayoutNodeMeta {
                        id: Some("stack".into()),
                        ..Default::default()
                    },
                    rect: hypreact_core::LayoutRect {
                        x: 0.0,
                        y: 0.0,
                        width: 640.0,
                        height: 1000.0,
                    },
                    styles: Some(hypreact_scene::SceneNodeStyle {
                        layout: hypreact_scene::ComputedStyle {
                            flex_grow: Some(2.0),
                            ..Default::default()
                        },
                    }),
                    window_id: Some(WindowId::from("stack")),
                    children: vec![],
                },
                LayoutSnapshotNode::Window {
                    meta: hypreact_core::LayoutNodeMeta {
                        id: Some("extra".into()),
                        ..Default::default()
                    },
                    rect: hypreact_core::LayoutRect {
                        x: 640.0,
                        y: 0.0,
                        width: 160.0,
                        height: 1000.0,
                    },
                    styles: Some(hypreact_scene::SceneNodeStyle {
                        layout: hypreact_scene::ComputedStyle {
                            flex_grow: Some(1.0),
                            ..Default::default()
                        },
                    }),
                    window_id: Some(WindowId::from("extra")),
                    children: vec![],
                },
                LayoutSnapshotNode::Window {
                    meta: hypreact_core::LayoutNodeMeta {
                        id: Some("master".into()),
                        ..Default::default()
                    },
                    rect: hypreact_core::LayoutRect {
                        x: 800.0,
                        y: 0.0,
                        width: 800.0,
                        height: 1000.0,
                    },
                    styles: Some(hypreact_scene::SceneNodeStyle {
                        layout: hypreact_scene::ComputedStyle {
                            flex_grow: Some(3.0),
                            ..Default::default()
                        },
                    }),
                    window_id: Some(WindowId::from("master")),
                    children: vec![],
                },
            ],
        };

        let initial_tree = partition_tree_from_scene(
            &initial_scene,
            ResizeBehaviorConfig {
                step_px: DEFAULT_RESIZE_STEP_UNITS as f32 * 8.0,
                min_branch_main_size_px: DEFAULT_MIN_INFERRED_BRANCH_MAIN_SIZE_PX,
            },
        );
        let reordered_tree = partition_tree_from_scene(
            &reordered_scene,
            ResizeBehaviorConfig {
                step_px: DEFAULT_RESIZE_STEP_UNITS as f32 * 8.0,
                min_branch_main_size_px: DEFAULT_MIN_INFERRED_BRANCH_MAIN_SIZE_PX,
            },
        );
        let mut resize_state = hypreact_core::resize::WorkspaceResizeState::default();
        let candidate = select_resize_candidate(
            &initial_tree,
            &WindowId::from("master"),
            ResizeDirection::Right,
        )
        .expect("initial resize candidate");

        assert!(apply_resize_step(
            &mut resize_state,
            &initial_tree,
            &candidate,
            DEFAULT_RESIZE_STEP_UNITS,
        ));
        assert_eq!(
            resize_state.adjustments_by_partition_id[&partition_id].branch_shares,
            vec![48, 12]
        );

        let adjustment = resize_state
            .adjustments_by_partition_id
            .get(&partition_id)
            .expect("persisted adjustment");
        let reordered_partition =
            reordered_tree.partitions.get(&partition_id).expect("reordered partition");
        let reordered_branch_ids = reordered_partition
            .branches
            .iter()
            .map(|branch| branch.branch_id.clone())
            .collect::<Vec<_>>();
        let reordered_defaults = reordered_partition
            .branches
            .iter()
            .map(|branch| branch.default_share)
            .collect::<Vec<_>>();

        assert_eq!(
            hypreact_core::resize::reconciled_branch_shares(
                adjustment,
                &reordered_branch_ids,
                &reordered_defaults,
            ),
            vec![12, 12, 48]
        );
    }

    #[test]
    fn flex_partition_is_inferred_from_display_and_direction() {
        let scene = LayoutSnapshotNode::Group {
            meta: hypreact_core::LayoutNodeMeta { id: Some("frame".into()), ..Default::default() },
            rect: hypreact_core::LayoutRect { x: 0.0, y: 0.0, width: 1000.0, height: 700.0 },
            styles: Some(hypreact_scene::SceneNodeStyle {
                layout: hypreact_scene::ComputedStyle {
                    display: Some(Display::Flex),
                    flex_direction: Some(FlexDirectionValue::Row),
                    ..Default::default()
                },
            }),
            children: vec![
                LayoutSnapshotNode::Window {
                    meta: hypreact_core::LayoutNodeMeta {
                        id: Some("left".into()),
                        ..Default::default()
                    },
                    rect: hypreact_core::LayoutRect { x: 0.0, y: 0.0, width: 500.0, height: 700.0 },
                    styles: None,
                    window_id: Some(WindowId::from("left")),
                    children: vec![],
                },
                LayoutSnapshotNode::Window {
                    meta: hypreact_core::LayoutNodeMeta {
                        id: Some("right".into()),
                        ..Default::default()
                    },
                    rect: hypreact_core::LayoutRect {
                        x: 500.0,
                        y: 0.0,
                        width: 500.0,
                        height: 700.0,
                    },
                    styles: None,
                    window_id: Some(WindowId::from("right")),
                    children: vec![],
                },
            ],
        };

        let partition_tree = partition_tree_from_scene(
            &scene,
            ResizeBehaviorConfig {
                step_px: DEFAULT_RESIZE_STEP_UNITS as f32 * 8.0,
                min_branch_main_size_px: DEFAULT_MIN_INFERRED_BRANCH_MAIN_SIZE_PX,
            },
        );

        assert!(partition_tree.partitions.contains_key(&PartitionId::new("frame")));
        assert_eq!(
            select_resize_candidate(
                &partition_tree,
                &WindowId::from("left"),
                ResizeDirection::Right
            ),
            Some(hypreact_core::resize::ResizeCandidate {
                partition_id: PartitionId::new("frame"),
                grow_branch_index: 0,
                shrink_branch_index: 1,
            })
        );
    }
}
