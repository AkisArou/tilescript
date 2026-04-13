use std::path::{Path, PathBuf};

use hypreact_config::authoring_layout::{
    AuthoringLayoutService, AuthoringLayoutServiceError, PreparedLayoutEvaluation,
};
use hypreact_config::model::{Config, ConfigDiscoveryOptions, ConfigPaths, LayoutConfigError};
use hypreact_config::runtime::build_authoring_layout_service;
use hypreact_core::focus::preferred_focus_after_removing_window;
use hypreact_core::focus::{FocusTree, FocusTreeWindowGeometry};
use hypreact_core::navigation::WindowGeometryCandidate;
use hypreact_core::navigation::{select_directional_focus_candidate, NavigationDirection};
use hypreact_core::query::state_snapshot_for_model;
use hypreact_core::snapshot::{StateSnapshot, WorkspaceSnapshot};
use hypreact_core::wm::WindowGeometry;
use hypreact_core::wm::WmModel;
use hypreact_runtime_js::build_runtime_bundle;
use hypreact_scene::ast::ValidatedLayoutTree;
use hypreact_scene::pipeline::SceneCache;
use hypreact_scene::{LayoutSnapshotNode, SceneResponse};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutRuntimePaths {
    pub config_paths: ConfigPaths,
}

impl LayoutRuntimePaths {
    pub fn discover(options: ConfigDiscoveryOptions) -> Result<Self, LayoutRuntimeError> {
        Ok(Self {
            config_paths: ConfigPaths::discover(options)?,
        })
    }

    pub fn from_authored_config(authored_config: impl Into<PathBuf>) -> Self {
        let authored_config = authored_config.into();
        let prepared_parent = authored_config
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".hypreact-build");
        Self {
            config_paths: ConfigPaths::new(authored_config, prepared_parent.join("config.js")),
        }
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
        Ok(LoadedLayoutConfig {
            config: self.service.load_config(&self.paths.config_paths)?,
        })
    }

    pub fn load_authored_config(&self) -> Result<LoadedLayoutConfig, LayoutRuntimeError> {
        Ok(LoadedLayoutConfig {
            config: self
                .service
                .load_authored_config(&self.paths.config_paths)?,
        })
    }

    pub fn reload_config(&mut self) -> Result<LoadedLayoutConfig, LayoutRuntimeError> {
        Ok(LoadedLayoutConfig {
            config: self.service.reload_config()?,
        })
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
        let Some(evaluation) = self
            .service
            .evaluate_prepared_for_workspace(config, state, workspace)?
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
            workspace,
            workspace
                .output_id
                .as_ref()
                .and_then(|output_id| state.output_by_id(output_id))
                .or_else(|| state.current_output()),
            resolved.root,
            &evaluation.artifact,
        )?;
        let scene = SceneCache::new()
            .compute_layout_from_request(&request)
            .map_err(|error| LayoutConfigError::EvaluateAuthoredConfig {
                path: self.paths.config_paths.authored_config.clone(),
                message: error.to_string(),
            })?;

        let window_geometries = collect_window_geometries(&scene.root);
        let focus_tree = focus_tree_from_geometries(&window_geometries);
        let geometry_candidates =
            geometry_candidates_from_focus_tree(&window_geometries, &focus_tree);
        let ordered_window_ids = ordered_window_ids_from_scene(&scene.root);

        Ok(Some(LayoutWorkspaceScene {
            evaluation,
            scene,
            window_geometries,
            focus_tree,
            geometry_candidates,
            ordered_window_ids,
        }))
    }
}

pub fn apply_layout_selection_to_model(model: &mut WmModel, config: &Config) {
    let current_output_id = model.current_output_id().cloned();
    let workspace_names = model.workspace_names();

    for workspace in model.workspaces.values_mut() {
        workspace.effective_layout = config.selected_layout_ref_for_workspace(
            &workspace.name,
            workspace.output_id.as_ref().or(current_output_id.as_ref()),
            &workspace_names,
        );
    }
}

pub fn layout_status_for_model(
    service: &mut LayoutRuntimeService,
    model: &mut WmModel,
) -> Result<LayoutStatusSnapshot, LayoutRuntimeError> {
    let config_path = Some(
        service
            .paths()
            .config_paths
            .authored_config
            .display()
            .to_string(),
    );
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
                        workspace
                            .effective_layout
                            .as_ref()
                            .map(|layout| layout.name.clone())
                    }),
                layout: evaluation
                    .as_ref()
                    .map(|evaluation| evaluation.evaluation.layout.clone()),
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
    let Some(target_workspace) = model
        .workspaces
        .keys()
        .find(|id| id.as_str() == workspace_id)
        .cloned()
    else {
        return Ok(Vec::new());
    };

    let mut model = model.clone();
    let target_output = model
        .workspaces
        .get(&target_workspace)
        .and_then(|workspace| workspace.output_id.clone());

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
    let current_workspace_id = model
        .outputs
        .get(&output_id)
        .and_then(|existing| existing.focused_workspace_id.clone());
    model.upsert_output(
        output_id,
        name,
        logical_width,
        logical_height,
        current_workspace_id,
    );
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
    if let LayoutSnapshotNode::Window {
        window_id: Some(window_id),
        rect,
        ..
    } = node
    {
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

fn collect_ordered_window_ids(node: &LayoutSnapshotNode, out: &mut Vec<hypreact_core::WindowId>) {
    if let LayoutSnapshotNode::Window {
        window_id: Some(window_id),
        ..
    } = node
    {
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

    use hypreact_core::focus::FocusScopePath;
    use hypreact_core::navigation::{select_directional_focus_candidate, NavigationDirection};
    use hypreact_core::query::state_snapshot_for_model;
    use hypreact_core::wm::WmModel;
    use hypreact_core::WindowId;
    use hypreact_core::{OutputId, WorkspaceId};

    use super::*;

    #[test]
    fn geometry_candidates_preserve_branch_memory_for_master_stack_focus() {
        let geometries = BTreeMap::from([
            (
                WindowId::from("master"),
                WindowGeometry {
                    x: 0,
                    y: 0,
                    width: 600,
                    height: 900,
                },
            ),
            (
                WindowId::from("stack-1"),
                WindowGeometry {
                    x: 600,
                    y: 0,
                    width: 300,
                    height: 300,
                },
            ),
            (
                WindowId::from("stack-2"),
                WindowGeometry {
                    x: 600,
                    y: 300,
                    width: 300,
                    height: 300,
                },
            ),
            (
                WindowId::from("stack-3"),
                WindowGeometry {
                    x: 600,
                    y: 600,
                    width: 300,
                    height: 300,
                },
            ),
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
        let config_path = "/home/akisarou/projects/hypreact/test_config/test_config/config.ts";
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
            None,
        );

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

        let workspace_names = model.workspace_names();
        for workspace in model.workspaces.values_mut() {
            workspace.effective_layout = loaded
                .config
                .layout_selection
                .per_workspace
                .get(
                    workspace_names
                        .iter()
                        .position(|name| name == &workspace.name)
                        .unwrap(),
                )
                .cloned()
                .map(|name| hypreact_core::types::LayoutRef { name });
        }

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
}
