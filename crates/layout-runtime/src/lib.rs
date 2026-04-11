use std::path::{Path, PathBuf};

use hypreact_config::authoring_layout::{
    AuthoringLayoutService, AuthoringLayoutServiceError, PreparedLayoutEvaluation,
};
use hypreact_config::model::{Config, ConfigDiscoveryOptions, ConfigPaths, LayoutConfigError};
use hypreact_config::runtime::build_authoring_layout_service;
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

        let resolved = ValidatedLayoutTree::new(evaluation.layout.clone())
            .map_err(|error| LayoutConfigError::EvaluateAuthoredConfig {
                path: self.paths.config_paths.authored_config.clone(),
                message: error.to_string(),
            })?
            .resolve(&state.windows)
            .map_err(|error| LayoutConfigError::EvaluateAuthoredConfig {
                path: self.paths.config_paths.authored_config.clone(),
                message: error.to_string(),
            })?;

        let request = config.build_scene_request(
            state.current_workspace().unwrap_or(workspace),
            state.current_output(),
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
        let geometry_candidates =
            geometry_candidates_for_workspace(&window_geometries, workspace.id.as_str());
        let ordered_window_ids = ordered_window_ids_from_scene(&scene.root);

        Ok(Some(LayoutWorkspaceScene {
            evaluation,
            scene,
            window_geometries,
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

fn geometry_candidates_for_workspace(
    geometries: &std::collections::BTreeMap<hypreact_core::WindowId, WindowGeometry>,
    _workspace_id: &str,
) -> Vec<WindowGeometryCandidate> {
    geometries
        .iter()
        .map(|(window_id, geometry)| WindowGeometryCandidate {
            window_id: window_id.clone(),
            geometry: *geometry,
            scope_path: vec![hypreact_core::focus::FocusScopePath::workspace()],
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
