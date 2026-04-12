use std::path::{Path, PathBuf};

use hypreact_config::authoring_layout::{
    AuthoringLayoutService, AuthoringLayoutServiceError, PreparedLayoutEvaluation,
};
use hypreact_config::model::{Config, ConfigDiscoveryOptions, ConfigPaths, LayoutConfigError};
use hypreact_config::runtime::build_authoring_layout_service;
use hypreact_core::focus::{FocusTree, FocusTreeWindowGeometry};
use hypreact_core::navigation::WindowGeometryCandidate;
use hypreact_core::snapshot::{StateSnapshot, WorkspaceSnapshot};
use hypreact_core::wm::WindowGeometry;
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
