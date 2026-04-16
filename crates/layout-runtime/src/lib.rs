use std::collections::BTreeSet;
use std::fs;
#[cfg(target_family = "unix")]
use std::os::fd::RawFd;
#[cfg(target_family = "unix")]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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
use hypreact_core::runtime::artifact_state::{
    ArtifactGraph, ArtifactKey, ArtifactRecord, ArtifactRegistry,
};
use hypreact_core::runtime::prepared_layout::{
    PreparedLayout, PreparedStylesheets, SelectedLayout,
};
use hypreact_core::runtime::runtime_kind::RuntimeKind;
use hypreact_core::snapshot::{StateSnapshot, WorkspaceSnapshot};
use hypreact_core::wm::WindowGeometry;
use hypreact_core::wm::WmModel;
use hypreact_core::{LayoutNodeMeta, RemainingTake, SlotTake, SourceLayoutNode};
use hypreact_css::analysis::{
    CssAnalysis, CssDiagnosticCode, CssDiagnosticSeverity, analyze_stylesheet,
};
use hypreact_scene::Display;
use hypreact_scene::FlexDirectionValue;
use hypreact_scene::ast::ValidatedLayoutTree;
use hypreact_scene::pipeline::SceneCache;
use hypreact_scene::{LayoutSnapshotNode, SceneResponse};
use tracing::{debug, info};

use hypreact_runtime_js_native::{decode_runtime_graph_payload, module_graph_execution_key};
use hypreact_runtime_lua_native::{
    lua_bytecode_artifact_key, lua_compiled_source_artifact_key, lua_executable_artifact_key,
};

mod runtime_factory;
mod source_watcher;

use source_watcher::SourceWatcher;

const DEFAULT_MIN_INFERRED_BRANCH_MAIN_SIZE_PX: f32 = 120.0;
const FALLBACK_LAYOUT_STYLESHEET: &str =
    "workspace { display: flex; width: 100%; height: 100%; } window { flex: 1 1 0; }";

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
    loaded_config: Option<Config>,
    watched_files: BTreeSet<PathBuf>,
    watched_fingerprints: Vec<(PathBuf, String)>,
    source_watcher: Option<SourceWatcher>,
    stylesheet_analysis_cache: ArtifactRegistry<CssAnalysis>,
    artifact_graph: ArtifactGraph,
    last_reload_at: Option<Instant>,
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
    pub evaluation: Option<PreparedLayoutEvaluation>,
    pub scene: SceneResponse,
    pub window_geometries: std::collections::BTreeMap<hypreact_core::WindowId, WindowGeometry>,
    pub focus_tree: FocusTree,
    pub partition_tree: PartitionTree,
    pub geometry_candidates: Vec<WindowGeometryCandidate>,
    pub ordered_window_ids: Vec<hypreact_core::WindowId>,
    pub diagnostics: Vec<LayoutDiagnostic>,
    pub error: Option<String>,
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
    pub diagnostics: Vec<LayoutDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutDiagnostic {
    pub source: String,
    pub severity: String,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub range: LayoutDiagnosticRange,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutDiagnosticRange {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
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
    #[cfg(target_family = "unix")]
    #[error("source watcher failed: {0}")]
    Watcher(std::io::Error),
}

impl LayoutRuntimeService {
    pub fn new(paths: LayoutRuntimePaths) -> Result<Self, LayoutRuntimeError> {
        let service = build_authoring_layout_service(
            &paths.config_paths,
            runtime_factory::build_runtime_bundle(&paths.config_paths)?,
        )?;
        let watched_files =
            watched_source_paths(&service, None, &paths.config_paths.authored_config);
        let watched_fingerprints = snapshot_watched_fingerprints(&watched_files);
        Ok(Self {
            service,
            paths,
            loaded_config: None,
            watched_files,
            watched_fingerprints,
            source_watcher: None,
            stylesheet_analysis_cache: ArtifactRegistry::new(),
            artifact_graph: ArtifactGraph::new(),
            last_reload_at: None,
        })
    }

    pub fn paths(&self) -> &LayoutRuntimePaths {
        &self.paths
    }

    fn ensure_loaded_config(&mut self) -> Result<LoadedLayoutConfig, LayoutRuntimeError> {
        if let Some(config) = self.loaded_config.clone() {
            return Ok(LoadedLayoutConfig { config });
        }

        self.load_config()
    }

    pub fn load_config(&mut self) -> Result<LoadedLayoutConfig, LayoutRuntimeError> {
        let loaded =
            LoadedLayoutConfig { config: self.service.load_config(&self.paths.config_paths)? };
        self.loaded_config = Some(loaded.config.clone());
        self.refresh_watched_paths_and_hashes();
        Ok(loaded)
    }

    pub fn load_authored_config(&self) -> Result<LoadedLayoutConfig, LayoutRuntimeError> {
        Ok(LoadedLayoutConfig {
            config: self.service.load_authored_config(&self.paths.config_paths)?,
        })
    }

    pub fn reload_config(&mut self) -> Result<LoadedLayoutConfig, LayoutRuntimeError> {
        let loaded = LoadedLayoutConfig { config: self.service.reload_config()? };
        self.invalidate_layout_style_artifacts();
        self.loaded_config = Some(loaded.config.clone());
        self.refresh_watched_paths_and_hashes();
        self.last_reload_at = Some(Instant::now());
        Ok(loaded)
    }

    #[cfg(target_family = "unix")]
    pub fn source_change_fd(&mut self) -> Result<RawFd, LayoutRuntimeError> {
        self.ensure_source_watcher()?;
        Ok(self.source_watcher.as_ref().expect("source watcher initialized").signal_fd())
    }

    #[cfg(not(target_family = "unix"))]
    pub fn source_change_fd(&mut self) -> Result<i32, LayoutRuntimeError> {
        Ok(-1)
    }

    pub fn drain_source_changes(&mut self) -> Result<bool, LayoutRuntimeError> {
        #[cfg(target_family = "unix")]
        self.drain_source_change_events()?;

        if self.last_reload_at.is_some_and(|instant| instant.elapsed() < Duration::from_millis(120))
        {
            debug!("live-reload poll skipped due to debounce window");
            return Ok(false);
        }

        let current_fingerprints = snapshot_watched_fingerprints(&self.watched_files);
        let prepared_cache_stale = authored_sources_newer_than_prepared_cache(
            &self.paths.config_paths.authored_config,
            &self.paths.config_paths.prepared_config,
        );
        debug!(
            watched_paths = ?self.watched_files,
            watched_fingerprint_count = current_fingerprints.len(),
            "live-reload poll snapshot"
        );
        if current_fingerprints == self.watched_fingerprints && !prepared_cache_stale {
            return Ok(false);
        }

        if prepared_cache_stale {
            info!("live-reload detected stale prepared cache relative to authored source tree");
        }

        for (path, old_fingerprint, new_fingerprint) in
            diff_watched_fingerprints(&self.watched_fingerprints, &current_fingerprints)
        {
            info!(
                path = %path.display(),
                old_fingerprint,
                new_fingerprint,
                "live-reload detected watched source change"
            );
        }

        self.watched_fingerprints = current_fingerprints;
        let _ = self.reload_config()?;
        info!("live-reload reloaded config after watched source change");
        Ok(true)
    }

    pub fn evaluate_workspace(
        &mut self,
        config: &Config,
        state: &StateSnapshot,
        workspace: &WorkspaceSnapshot,
    ) -> Result<Option<LayoutWorkspaceEvaluation>, LayoutRuntimeError> {
        let result = self
            .service
            .evaluate_prepared_for_workspace(config, state, workspace)?
            .map(|evaluation| LayoutWorkspaceEvaluation { evaluation });
        self.refresh_watched_paths();
        Ok(result)
    }

    pub fn evaluate_workspace_scene(
        &mut self,
        config: &Config,
        state: &StateSnapshot,
        workspace: &WorkspaceSnapshot,
    ) -> Result<Option<LayoutWorkspaceScene>, LayoutRuntimeError> {
        let workspace_windows = workspace_windows(state, workspace);
        let selected_layout = selected_layout_for_workspace(workspace, None);

        let Some(evaluation) =
            (match self.service.evaluate_prepared_for_workspace(config, state, workspace) {
                Ok(evaluation) => evaluation,
                Err(error) => {
                    let diagnostic = layout_failure_diagnostic(
                        authoring_layout_service_error_path(&error)
                            .or(Some(self.paths.config_paths.authored_config.as_path())),
                        error.to_string(),
                    );
                    let scene = build_fallback_scene(
                        config,
                        state,
                        workspace,
                        Some(&selected_layout),
                        &workspace_windows,
                    )?;
                    return Ok(Some(layout_workspace_scene(
                        config,
                        None,
                        scene,
                        vec![diagnostic.clone()],
                        Some(diagnostic.message),
                    )));
                }
            })
        else {
            return Ok(None);
        };

        match build_scene_from_layout(
            config,
            state,
            workspace,
            &evaluation.artifact,
            evaluation.layout.clone(),
            &workspace_windows,
            std::path::Path::new(&evaluation.artifact.selected.module),
        ) {
            Ok(scene) => {
                Ok(Some(layout_workspace_scene(config, Some(evaluation), scene, Vec::new(), None)))
            }
            Err(error) => {
                let diagnostic = scene_failure_diagnostic(self, &evaluation.artifact, &error);
                let scene = build_fallback_scene(
                    config,
                    state,
                    workspace,
                    Some(&evaluation.artifact.selected),
                    &workspace_windows,
                )?;
                Ok(Some(layout_workspace_scene(
                    config,
                    Some(evaluation),
                    scene,
                    vec![diagnostic.clone()],
                    Some(diagnostic.message),
                )))
            }
        }
    }

    fn refresh_watched_paths(&mut self) {
        self.watched_files = watched_source_paths(
            &self.service,
            self.loaded_config.as_ref(),
            &self.paths.config_paths.authored_config,
        );
        self.rebuild_source_watcher_if_supported();
        debug!(watched_paths = ?self.watched_files, "refreshed live-reload watched paths");
    }

    fn refresh_watched_paths_without_rebuilding_watcher(&mut self) {
        self.watched_files = watched_source_paths(
            &self.service,
            self.loaded_config.as_ref(),
            &self.paths.config_paths.authored_config,
        );
        debug!(watched_paths = ?self.watched_files, "refreshed live-reload watched paths");
    }

    fn rebuild_source_watcher_if_supported(&mut self) {
        #[cfg(target_family = "unix")]
        if let Err(error) = self.rebuild_source_watcher() {
            debug!(error = %error, "failed to rebuild source watcher");
        }
    }

    fn invalidate_layout_style_artifacts(&mut self) {
        let layout_keys = self
            .artifact_graph
            .dependents_of(&ArtifactKey::config("authoring-config"))
            .cloned()
            .collect::<Vec<_>>();
        for key in layout_keys {
            self.artifact_graph.invalidate_dependents_of(&key, &mut self.stylesheet_analysis_cache);
            self.artifact_graph.remove(&key);
        }
        self.stylesheet_analysis_cache.clear();
        self.artifact_graph.clear();
    }

    fn refresh_watched_paths_and_hashes(&mut self) {
        self.refresh_watched_paths();
        self.watched_fingerprints = snapshot_watched_fingerprints(&self.watched_files);
    }
}

#[cfg(target_family = "unix")]
impl LayoutRuntimeService {
    fn ensure_source_watcher(&mut self) -> Result<(), LayoutRuntimeError> {
        if self.source_watcher.is_some() {
            return Ok(());
        }

        self.source_watcher =
            Some(SourceWatcher::new(&self.watched_files).map_err(LayoutRuntimeError::Watcher)?);
        Ok(())
    }

    fn rebuild_source_watcher(&mut self) -> Result<(), LayoutRuntimeError> {
        self.source_watcher =
            Some(SourceWatcher::new(&self.watched_files).map_err(LayoutRuntimeError::Watcher)?);
        Ok(())
    }

    fn drain_source_change_events(&mut self) -> Result<(), LayoutRuntimeError> {
        self.ensure_source_watcher()?;
        if let Some(watcher) = self.source_watcher.as_mut() {
            let had_event = watcher.drain().map_err(LayoutRuntimeError::Watcher)?;
            if had_event {
                self.refresh_watched_paths_without_rebuilding_watcher();
                self.rebuild_source_watcher_if_supported();
            }
        }
        Ok(())
    }
}

fn authored_sources_newer_than_prepared_cache(
    authored_config: &Path,
    prepared_config: &Path,
) -> bool {
    let Some(authored_root) = authored_config.parent() else {
        return false;
    };
    let Some(prepared_root) = prepared_config.parent() else {
        return false;
    };

    let newest_authored = newest_tree_timestamp(authored_root, &[".hypreact-build", ".sdk"]);
    let newest_prepared =
        newest_tree_timestamp(prepared_root, &[".quickjs-bytecode", ".lua-bytecode"]);
    match (newest_authored, newest_prepared) {
        (Some(authored), Some(prepared)) => authored > prepared,
        (Some(_), None) => true,
        _ => false,
    }
}

fn newest_tree_timestamp(root: &Path, excluded_dirs: &[&str]) -> Option<std::time::SystemTime> {
    let mut newest = None;
    collect_newest_tree_timestamp(root, excluded_dirs, &mut newest);
    newest
}

fn collect_newest_tree_timestamp(
    root: &Path,
    excluded_dirs: &[&str],
    newest: &mut Option<std::time::SystemTime>,
) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if excluded_dirs.iter().any(|excluded| *excluded == name) {
                continue;
            }
            collect_newest_tree_timestamp(&path, excluded_dirs, newest);
            continue;
        }

        if let Ok(modified) = entry.metadata().and_then(|metadata| metadata.modified()) {
            if newest.is_none_or(|current| modified > current) {
                *newest = Some(modified);
            }
        }
    }
}

fn watched_source_paths(
    service: &AuthoringLayoutService,
    config: Option<&Config>,
    authored_config: &Path,
) -> BTreeSet<PathBuf> {
    let mut watched = BTreeSet::new();
    watched.extend(recursive_authored_source_paths(authored_config));
    let authored_root = authored_config.parent().unwrap_or_else(|| Path::new("."));

    if let Some(config) = config {
        if let Some(path) = config.global_stylesheet_path.as_ref() {
            if let Some(path) = resolve_authored_watch_path(authored_root, path) {
                watched.insert(path);
            }
        }

        for layout in &config.layouts {
            if let Some(path) = layout.stylesheet_path.as_ref() {
                if let Some(path) = resolve_authored_watch_path(authored_root, path) {
                    watched.insert(path);
                }
            }
        }
    }

    for layout in service.cached_layouts() {
        for dependency in &layout.dependencies {
            if let Some(path) = resolve_authored_watch_path(authored_root, &dependency.path) {
                watched.insert(path);
            }
        }
    }

    watched
}

fn resolve_authored_watch_path(authored_root: &Path, path: &str) -> Option<PathBuf> {
    let path = PathBuf::from(path);
    let resolved = if path.is_absolute() { path } else { authored_root.join(path) };

    if path_uses_runtime_cache_dir(&resolved) || !resolved.exists() {
        return None;
    }

    Some(resolved)
}

fn path_uses_runtime_cache_dir(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(component, std::path::Component::Normal(name) if name == ".hypreact-build" || name == ".sdk")
    })
}

fn recursive_authored_source_paths(authored_config: &Path) -> BTreeSet<PathBuf> {
    let mut watched = BTreeSet::new();
    let Some(root) = authored_config.parent() else {
        watched.insert(authored_config.to_path_buf());
        return watched;
    };

    collect_authored_source_paths(root, &mut watched);
    watched.insert(authored_config.to_path_buf());
    watched
}

fn collect_authored_source_paths(root: &Path, watched: &mut BTreeSet<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == ".hypreact-build" || name == ".sdk" {
                continue;
            }
            collect_authored_source_paths(&path, watched);
            continue;
        }

        watched.insert(path);
    }
}

fn snapshot_watched_fingerprints(watched_files: &BTreeSet<PathBuf>) -> Vec<(PathBuf, String)> {
    watched_files
        .iter()
        .filter_map(|path| {
            fs::metadata(path).ok().and_then(|metadata| {
                let modified = metadata.modified().ok()?;
                let modified = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
                #[cfg(target_family = "unix")]
                let changed = format!("{}:{}", metadata.ctime(), metadata.ctime_nsec());
                #[cfg(not(target_family = "unix"))]
                let changed = String::new();
                Some((
                    path.clone(),
                    format!("{}:{}:{}", metadata.len(), modified.as_nanos(), changed),
                ))
            })
        })
        .collect()
}

fn diff_watched_fingerprints(
    previous: &[(PathBuf, String)],
    current: &[(PathBuf, String)],
) -> Vec<(PathBuf, String, String)> {
    let previous = previous.iter().cloned().collect::<std::collections::BTreeMap<_, _>>();
    current
        .iter()
        .filter_map(|(path, new_hash)| {
            previous.get(path).and_then(|old_hash| {
                (old_hash != new_hash).then(|| (path.clone(), old_hash.clone(), new_hash.clone()))
            })
        })
        .collect()
}

#[cfg(test)]
mod runtime_watch_tests {
    use super::*;
    use hypreact_core::WorkspaceId;
    use std::thread;

    #[test]
    fn poll_watches_authored_source_root_except_runtime_cache_dirs() {
        let root = tempfile::TempDir::new().unwrap();
        let authored = root.path().join("config.lua");
        let layout_dir = root.path().join("layouts/master-stack");
        let build_dir = root.path().join(".hypreact-build/layouts/master-stack");
        let sdk_dir = root.path().join(".sdk/runtime");
        fs::create_dir_all(&layout_dir).unwrap();
        fs::create_dir_all(&build_dir).unwrap();
        fs::create_dir_all(&sdk_dir).unwrap();
        fs::write(
            &authored,
            "return { defaultLayout = 'master-stack', layoutRules = { { index = 0, layout = 'master-stack' } } }",
        )
        .unwrap();
        fs::write(
            layout_dir.join("index.lua"),
            "local h = require('hypreact') return function(ctx) return h.workspace({ id = 'root' }) { h.slot({ id = 'main' }) } end",
        )
        .unwrap();
        fs::write(layout_dir.join("index.css"), ".master { flex: 1; }").unwrap();
        fs::write(root.path().join("unrelated.ts"), "export default 1;").unwrap();
        fs::write(build_dir.join("index.js"), "export default 1;").unwrap();
        fs::write(sdk_dir.join("generated.lua"), "return {}").unwrap();

        let mut service =
            LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(&authored)).unwrap();
        let loaded = service.reload_config().unwrap();
        let mut model = WmModel::default();
        model.upsert_workspace(WorkspaceId::from("1"), "1".to_string());
        model.set_current_workspace(WorkspaceId::from("1"));
        let snapshot = state_snapshot_for_model(&model);
        let workspace = snapshot.current_workspace().cloned().unwrap();
        let _ = service.evaluate_workspace_scene(&loaded.config, &snapshot, &workspace);

        assert!(service.watched_files.contains(&authored));
        assert!(service.watched_files.contains(&layout_dir.join("index.lua")));
        assert!(service.watched_files.contains(&layout_dir.join("index.css")));
        assert!(service.watched_files.contains(&root.path().join("unrelated.ts")));
        assert!(!service.watched_files.contains(&build_dir.join("index.js")));
        assert!(!service.watched_files.contains(&sdk_dir.join("generated.lua")));
    }

    #[test]
    fn poll_debounces_immediate_reloads() {
        let root = tempfile::TempDir::new().unwrap();
        let authored = root.path().join("config.lua");
        let layout_dir = root.path().join("layouts/master-stack");
        fs::create_dir_all(&layout_dir).unwrap();
        fs::write(
            &authored,
            "return { defaultLayout = 'master-stack', layoutRules = { { index = 0, layout = 'master-stack' } } }",
        )
        .unwrap();
        fs::write(
            layout_dir.join("index.lua"),
            "local h = require('hypreact') return function(ctx) return h.workspace({ id = 'root' }) { h.slot({ id = 'main' }) } end",
        )
        .unwrap();

        let mut service =
            LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(&authored)).unwrap();
        let _ = service.reload_config().unwrap();
        fs::write(
            layout_dir.join("index.lua"),
            "local h = require('hypreact') return function(ctx) return h.workspace({ id = 'changed' }) { h.slot({ id = 'main' }) } end",
        )
        .unwrap();

        assert!(!service.drain_source_changes().unwrap());

        thread::sleep(Duration::from_millis(130));
        assert!(service.drain_source_changes().unwrap());
    }

    #[test]
    fn poll_detects_same_length_edit_after_timestamp_resolution_window() {
        let root = tempfile::TempDir::new().unwrap();
        let authored = root.path().join("config.lua");
        let layout_dir = root.path().join("layouts/master-stack");
        fs::create_dir_all(&layout_dir).unwrap();
        fs::write(
            &authored,
            "return { defaultLayout = 'master-stack', layoutRules = { { index = 0, layout = 'master-stack' } } }",
        )
        .unwrap();
        let layout_path = layout_dir.join("index.lua");
        fs::write(&layout_path, "return { take = 1 }").unwrap();

        let mut service =
            LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(&authored)).unwrap();
        let _ = service.load_config().unwrap();

        std::thread::sleep(Duration::from_secs(1));
        fs::write(&layout_path, "return { take = 2 }").unwrap();

        assert!(service.drain_source_changes().unwrap());
    }

    #[test]
    fn watcher_expands_coverage_after_new_child_directory_event() {
        let root = tempfile::TempDir::new().unwrap();
        let authored = root.path().join("config.lua");
        let layout_dir = root.path().join("layouts/master-stack");
        fs::create_dir_all(&layout_dir).unwrap();
        fs::write(
            &authored,
            "return { defaultLayout = 'master-stack', layoutRules = { { index = 0, layout = 'master-stack' } } }",
        )
        .unwrap();
        fs::write(layout_dir.join("index.lua"), "return { take = 1 }").unwrap();

        let mut service =
            LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(&authored)).unwrap();
        let _ = service.load_config().unwrap();

        let nested_dir = layout_dir.join("nested");
        fs::create_dir(&nested_dir).unwrap();
        std::thread::sleep(Duration::from_millis(130));

        assert!(!service.drain_source_changes().unwrap());

        fs::write(nested_dir.join("child.lua"), "return { nested = true }").unwrap();
        std::thread::sleep(Duration::from_millis(130));

        assert!(service.drain_source_changes().unwrap());
    }

    #[test]
    fn js_config_watch_set_excludes_prepared_runtime_artifacts() {
        let config_path =
            PathBuf::from("/home/akisarou/projects/hypreact/dev/test-config/config.ts");
        let mut service =
            LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(&config_path))
                .unwrap();

        let _ = service.load_config().unwrap();

        assert!(
            service
                .watched_files
                .contains(&config_path.parent().unwrap().join("layouts/master-stack/index.tsx"))
        );
        assert!(
            service
                .watched_files
                .contains(&config_path.parent().unwrap().join("layouts/master-stack/index.css"))
        );
        assert!(!service.watched_files.iter().any(|path| path
            .components()
            .any(|component| matches!(component, std::path::Component::Normal(name) if name == ".hypreact-build"))));
        assert!(
            !service
                .watched_files
                .iter()
                .any(|path| path == Path::new("layouts/master-stack/index.js"))
        );
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
    let loaded = service.ensure_loaded_config()?;

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
            diagnostics: Vec::new(),
        });
    };

    match service.evaluate_workspace_scene(&loaded.config, &snapshot, &workspace) {
        Ok(evaluation) => {
            let diagnostics = evaluation
                .as_ref()
                .map(|evaluation| {
                    let mut diagnostics = evaluation
                        .evaluation
                        .as_ref()
                        .map(|evaluation| {
                            record_stylesheet_dependencies(service, &evaluation.artifact);
                            record_js_runtime_dependencies(service, &evaluation.artifact);
                            record_lua_runtime_dependencies(service, &evaluation.artifact);
                            diagnostics_for_stylesheets(service, &evaluation.artifact.stylesheets)
                        })
                        .unwrap_or_default();
                    diagnostics.extend(evaluation.diagnostics.clone());
                    diagnostics
                })
                .unwrap_or_default();
            if let Some(evaluation) = evaluation.as_ref() {
                model.set_focus_tree_value(Some(evaluation.focus_tree.clone()));
            }

            Ok(LayoutStatusSnapshot {
                config_path,
                workspace_names: Some(snapshot.workspace_names.clone()),
                loaded: true,
                selected_layout_name: evaluation
                    .as_ref()
                    .and_then(|evaluation| {
                        evaluation
                            .evaluation
                            .as_ref()
                            .map(|evaluation| evaluation.artifact.selected.name.clone())
                    })
                    .or_else(|| {
                        workspace.effective_layout.as_ref().map(|layout| layout.name.clone())
                    }),
                layout: evaluation.as_ref().and_then(|evaluation| {
                    evaluation.evaluation.as_ref().map(|evaluation| evaluation.layout.clone())
                }),
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
                error: evaluation.as_ref().and_then(|evaluation| evaluation.error.clone()),
                diagnostics,
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
            diagnostics: Vec::new(),
        }),
    }
}

fn layout_workspace_scene(
    config: &Config,
    evaluation: Option<PreparedLayoutEvaluation>,
    scene: SceneResponse,
    diagnostics: Vec<LayoutDiagnostic>,
    error: Option<String>,
) -> LayoutWorkspaceScene {
    let resize_behavior = ResizeBehaviorConfig::from_config(config);
    let window_geometries = collect_window_geometries(&scene.root);
    let focus_tree = focus_tree_from_geometries(&window_geometries);
    let partition_tree = partition_tree_from_scene(&scene.root, resize_behavior);
    let geometry_candidates = geometry_candidates_from_focus_tree(&window_geometries, &focus_tree);
    let ordered_window_ids = ordered_window_ids_from_scene(&scene.root);

    LayoutWorkspaceScene {
        evaluation,
        scene,
        window_geometries: window_geometries
            .iter()
            .map(|entry| (entry.window_id.clone(), entry.geometry))
            .collect(),
        focus_tree,
        partition_tree,
        geometry_candidates,
        ordered_window_ids,
        diagnostics,
        error,
    }
}

fn workspace_windows(
    state: &StateSnapshot,
    workspace: &WorkspaceSnapshot,
) -> Vec<hypreact_core::snapshot::WindowSnapshot> {
    state
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
        .collect()
}

fn build_scene_from_layout(
    config: &Config,
    state: &StateSnapshot,
    workspace: &WorkspaceSnapshot,
    artifact: &PreparedLayout,
    layout: SourceLayoutNode,
    workspace_windows: &[hypreact_core::snapshot::WindowSnapshot],
    error_path: &Path,
) -> Result<SceneResponse, LayoutConfigError> {
    let resolved = ValidatedLayoutTree::new(layout)
        .map_err(|error| LayoutConfigError::EvaluateAuthoredConfig {
            path: error_path.to_path_buf(),
            message: error.to_string(),
        })?
        .resolve(workspace_windows)
        .map_err(|error| LayoutConfigError::EvaluateAuthoredConfig {
            path: error_path.to_path_buf(),
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
        artifact,
    )?;

    SceneCache::new().compute_layout_from_request(&request).map_err(|error| {
        LayoutConfigError::EvaluateAuthoredConfig {
            path: error_path.to_path_buf(),
            message: error.to_string(),
        }
    })
}

fn build_fallback_scene(
    config: &Config,
    state: &StateSnapshot,
    workspace: &WorkspaceSnapshot,
    selected_layout: Option<&SelectedLayout>,
    workspace_windows: &[hypreact_core::snapshot::WindowSnapshot],
) -> Result<SceneResponse, LayoutRuntimeError> {
    let artifact = PreparedLayout {
        selected: selected_layout
            .cloned()
            .unwrap_or_else(|| selected_layout_for_workspace(workspace, None)),
        runtime_payload: serde_json::Value::Null,
        stylesheets: PreparedStylesheets {
            global: None,
            layout: Some(hypreact_core::runtime::prepared_layout::PreparedStylesheet {
                path: format!("{}/fallback.css", artifact_style_directory(workspace)),
                source: FALLBACK_LAYOUT_STYLESHEET.to_string(),
            }),
        },
        dependencies: vec![],
    };

    build_scene_from_layout(
        config,
        state,
        workspace,
        &artifact,
        fallback_source_layout(),
        workspace_windows,
        Path::new(&artifact.selected.module),
    )
    .map_err(LayoutRuntimeError::from)
}

fn selected_layout_for_workspace(
    workspace: &WorkspaceSnapshot,
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
        runtime: hypreact_core::runtime::runtime_kind::RuntimeKind::Js,
        module: fallback_module
            .map(str::to_string)
            .unwrap_or_else(|| format!("{directory}/index.tsx")),
        name,
        directory,
    }
}

fn artifact_style_directory(workspace: &WorkspaceSnapshot) -> String {
    workspace
        .effective_layout
        .as_ref()
        .map(|layout| format!("layouts/{}", layout.name))
        .unwrap_or_else(|| "layouts/fallback".to_string())
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

fn authoring_layout_service_error_path(error: &AuthoringLayoutServiceError) -> Option<&Path> {
    match error {
        AuthoringLayoutServiceError::Config(config_error) => layout_config_error_path(config_error),
        AuthoringLayoutServiceError::Runtime(_) => None,
    }
}

fn layout_config_error_path(error: &LayoutConfigError) -> Option<&Path> {
    match error {
        LayoutConfigError::ReadConfig { path }
        | LayoutConfigError::ParseConfig { path }
        | LayoutConfigError::CompileAuthoredConfig { path, .. }
        | LayoutConfigError::EvaluateAuthoredConfig { path, .. }
        | LayoutConfigError::DecodeAuthoredConfig { path, .. } => Some(path.as_path()),
        LayoutConfigError::UnknownLayout { .. }
        | LayoutConfigError::ArtifactLayoutMismatch { .. } => None,
    }
}

fn layout_failure_diagnostic(path: Option<&Path>, message: String) -> LayoutDiagnostic {
    LayoutDiagnostic {
        source: "layout".into(),
        severity: "error".into(),
        code: "layoutFallback".into(),
        message: format!("{message}; using fallback layout"),
        path: path.map(|path| path.display().to_string()),
        range: LayoutDiagnosticRange { start_line: 1, start_column: 1, end_line: 1, end_column: 1 },
    }
}

fn scene_failure_diagnostic(
    service: &mut LayoutRuntimeService,
    artifact: &PreparedLayout,
    error: &LayoutConfigError,
) -> LayoutDiagnostic {
    if let LayoutConfigError::EvaluateAuthoredConfig { message, .. } = error {
        if let Some(diagnostic) = css_scene_failure_diagnostic(service, artifact, message) {
            return diagnostic;
        }
    }

    layout_failure_diagnostic(
        Some(std::path::Path::new(&artifact.selected.module)),
        error.to_string(),
    )
}

fn css_scene_failure_diagnostic(
    service: &mut LayoutRuntimeService,
    artifact: &PreparedLayout,
    scene_error_message: &str,
) -> Option<LayoutDiagnostic> {
    let stylesheet = artifact.stylesheets.layout.as_ref()?;
    let diagnostic = stylesheet_analysis(service, &stylesheet.path, &stylesheet.source)
        .diagnostics
        .clone()
        .into_iter()
        .find(|diagnostic| matches!(diagnostic.severity, CssDiagnosticSeverity::Error))?;

    let message = if scene_error_message.contains(&diagnostic.message) {
        format!("{scene_error_message}; using fallback layout")
    } else {
        format!("{}; using fallback layout", diagnostic.message)
    };

    Some(LayoutDiagnostic {
        source: "css".into(),
        severity: match diagnostic.severity {
            CssDiagnosticSeverity::Error => "error",
            CssDiagnosticSeverity::Warning => "warning",
            CssDiagnosticSeverity::Information => "information",
        }
        .into(),
        code: match diagnostic.code {
            CssDiagnosticCode::UnsupportedAtRule => "unsupportedAtRule",
            CssDiagnosticCode::UnsupportedSelector => "unsupportedSelector",
            CssDiagnosticCode::UnsupportedProperty => "unsupportedProperty",
            CssDiagnosticCode::InvalidSyntax => "invalidSyntax",
            CssDiagnosticCode::UnsupportedValue => "unsupportedValue",
            CssDiagnosticCode::InapplicableProperty => "inapplicableProperty",
            CssDiagnosticCode::UnknownAnimationName => "unknownAnimationName",
            CssDiagnosticCode::UnsupportedAttributeKey => "unsupportedAttributeKey",
        }
        .into(),
        message,
        path: Some(stylesheet.path.clone()),
        range: LayoutDiagnosticRange {
            start_line: diagnostic.range.start_line,
            start_column: diagnostic.range.start_column,
            end_line: diagnostic.range.end_line,
            end_column: diagnostic.range.end_column,
        },
    })
}

fn diagnostics_for_stylesheets(
    service: &mut LayoutRuntimeService,
    stylesheets: &hypreact_core::runtime::prepared_layout::PreparedStylesheets,
) -> Vec<LayoutDiagnostic> {
    let mut diagnostics = Vec::new();

    if let Some(stylesheet) = stylesheets.global.as_ref() {
        diagnostics.extend(layout_diagnostics_from_stylesheet(
            service,
            &stylesheet.source,
            Some(stylesheet.path.as_str()),
        ));
    }

    if let Some(stylesheet) = stylesheets.layout.as_ref() {
        diagnostics.extend(layout_diagnostics_from_stylesheet(
            service,
            &stylesheet.source,
            Some(stylesheet.path.as_str()),
        ));
    }

    diagnostics
}

fn layout_diagnostics_from_stylesheet(
    service: &mut LayoutRuntimeService,
    source: &str,
    path: Option<&str>,
) -> Vec<LayoutDiagnostic> {
    stylesheet_analysis(service, path.unwrap_or("<inline-css>"), source)
        .diagnostics
        .clone()
        .into_iter()
        .map(|diagnostic| LayoutDiagnostic {
            source: "css".into(),
            severity: match diagnostic.severity {
                CssDiagnosticSeverity::Error => "error",
                CssDiagnosticSeverity::Warning => "warning",
                CssDiagnosticSeverity::Information => "information",
            }
            .into(),
            code: match diagnostic.code {
                CssDiagnosticCode::UnsupportedAtRule => "unsupportedAtRule",
                CssDiagnosticCode::UnsupportedSelector => "unsupportedSelector",
                CssDiagnosticCode::UnsupportedProperty => "unsupportedProperty",
                CssDiagnosticCode::InvalidSyntax => "invalidSyntax",
                CssDiagnosticCode::UnsupportedValue => "unsupportedValue",
                CssDiagnosticCode::InapplicableProperty => "inapplicableProperty",
                CssDiagnosticCode::UnknownAnimationName => "unknownAnimationName",
                CssDiagnosticCode::UnsupportedAttributeKey => "unsupportedAttributeKey",
            }
            .into(),
            message: diagnostic.message,
            path: path.map(str::to_string),
            range: LayoutDiagnosticRange {
                start_line: diagnostic.range.start_line,
                start_column: diagnostic.range.start_column,
                end_line: diagnostic.range.end_line,
                end_column: diagnostic.range.end_column,
            },
        })
        .collect()
}

fn stylesheet_analysis<'a>(
    service: &'a mut LayoutRuntimeService,
    cache_key: &str,
    source: &str,
) -> &'a CssAnalysis {
    let source_hash = stylesheet_source_hash(source);
    let artifact_key = ArtifactKey::stylesheet_analysis(cache_key.to_string());
    let cache_miss = service
        .stylesheet_analysis_cache
        .get(&artifact_key)
        .is_none_or(|cached| cached.fingerprint != source_hash);
    if cache_miss {
        service.stylesheet_analysis_cache.insert(
            artifact_key.clone(),
            ArtifactRecord::new(source_hash, analyze_stylesheet(source)),
        );
    }
    &service.stylesheet_analysis_cache.get(&artifact_key).expect("stylesheet analysis cached").value
}

fn stylesheet_source_hash(source: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn record_stylesheet_dependencies(service: &mut LayoutRuntimeService, artifact: &PreparedLayout) {
    let layout_key = ArtifactKey::layout(artifact.selected.name.clone());
    let mut stylesheet_keys = Vec::new();

    if let Some(stylesheet) = artifact.stylesheets.global.as_ref() {
        stylesheet_keys.push(ArtifactKey::stylesheet_analysis(stylesheet.path.clone()));
    }
    if let Some(stylesheet) = artifact.stylesheets.layout.as_ref() {
        stylesheet_keys.push(ArtifactKey::stylesheet_analysis(stylesheet.path.clone()));
    }

    service.artifact_graph.replace_edges(layout_key, stylesheet_keys);
}

fn record_js_runtime_dependencies(service: &mut LayoutRuntimeService, artifact: &PreparedLayout) {
    if artifact.selected.runtime != RuntimeKind::Js {
        return;
    }

    let Ok(graph) = decode_runtime_graph_payload(&artifact.runtime_payload) else {
        return;
    };

    let graph_key = module_graph_execution_key(&graph);
    let layout_key = ArtifactKey::layout(artifact.selected.name.clone());
    let module_graph_key = ArtifactKey::js_module_graph(graph_key.clone());
    let bytecode_key = ArtifactKey::js_bytecode(graph_key);

    let existing_stylesheet_dependents = service
        .artifact_graph
        .dependents_of(&layout_key)
        .filter(|key| {
            key.kind == hypreact_core::runtime::artifact_state::ArtifactKind::StylesheetAnalysis
        })
        .cloned()
        .collect::<Vec<_>>();

    let mut layout_dependents = existing_stylesheet_dependents;
    layout_dependents.push(module_graph_key.clone());
    service.artifact_graph.replace_edges(layout_key, layout_dependents);
    service.artifact_graph.replace_edges(module_graph_key, [bytecode_key]);
}

fn record_lua_runtime_dependencies(service: &mut LayoutRuntimeService, artifact: &PreparedLayout) {
    if artifact.selected.runtime != RuntimeKind::Lua {
        return;
    }

    let Some(source) = artifact.runtime_payload.get("source").and_then(serde_json::Value::as_str)
    else {
        return;
    };

    let source_module = artifact
        .runtime_payload
        .get("sourceModule")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(&artifact.selected.module);
    let executable_key = lua_executable_artifact_key(&artifact.selected.module, source);
    let bytecode_key = lua_bytecode_artifact_key(&artifact.selected.module, source);
    let layout_key = ArtifactKey::layout(artifact.selected.name.clone());
    let executable_artifact_key = ArtifactKey::lua_executable(executable_key.clone());
    let bytecode_artifact_key = ArtifactKey::lua_bytecode(bytecode_key);

    let existing_stylesheet_dependents = service
        .artifact_graph
        .dependents_of(&layout_key)
        .filter(|key| {
            key.kind == hypreact_core::runtime::artifact_state::ArtifactKind::StylesheetAnalysis
        })
        .cloned()
        .collect::<Vec<_>>();

    let mut layout_dependents = existing_stylesheet_dependents;
    if Path::new(source_module).extension().and_then(|ext| ext.to_str()) == Some("fnl") {
        let compiled_source_key =
            lua_compiled_source_artifact_key(&artifact.selected.module, source_module);
        let compiled_source_artifact_key = ArtifactKey::lua_compiled_source(compiled_source_key);
        layout_dependents.push(compiled_source_artifact_key.clone());
        service
            .artifact_graph
            .replace_edges(compiled_source_artifact_key, [executable_artifact_key.clone()]);
    } else {
        layout_dependents.push(executable_artifact_key.clone());
    }
    service.artifact_graph.replace_edges(layout_key, layout_dependents);
    service.artifact_graph.replace_edges(executable_artifact_key, [bytecode_artifact_key]);
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
    let loaded = service.ensure_loaded_config()?;

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

    let loaded = service.ensure_loaded_config()?;
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

fn collect_window_geometries(root: &LayoutSnapshotNode) -> Vec<FocusTreeWindowGeometry> {
    let mut geometries = Vec::new();
    collect_window_geometries_inner(root, &mut geometries);
    geometries
}

fn collect_window_geometries_inner(
    node: &LayoutSnapshotNode,
    out: &mut Vec<FocusTreeWindowGeometry>,
) {
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

fn focus_tree_from_geometries(geometries: &[FocusTreeWindowGeometry]) -> FocusTree {
    FocusTree::from_window_geometries(geometries)
}

fn geometry_candidates_from_focus_tree(
    geometries: &[FocusTreeWindowGeometry],
    focus_tree: &FocusTree,
) -> Vec<WindowGeometryCandidate> {
    geometries
        .iter()
        .map(|entry| WindowGeometryCandidate {
            scope_path: focus_tree
                .scope_path(&entry.window_id)
                .map(|scope_path| scope_path.to_vec())
                .unwrap_or_else(|| vec![FocusTree::workspace_scope()]),
            window_id: entry.window_id.clone(),
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
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;

    use hypreact_core::WindowId;
    use hypreact_core::focus::FocusScopePath;
    use hypreact_core::navigation::{NavigationDirection, select_directional_focus_candidate};
    use hypreact_core::query::state_snapshot_for_model;
    use hypreact_core::wm::WmModel;
    use hypreact_core::{OutputId, WorkspaceId};

    use super::*;

    fn isolated_test_config_path() -> PathBuf {
        let source_root = PathBuf::from("/home/akisarou/projects/hypreact/dev/test-config");
        let temp_root = tempfile::TempDir::new().expect("temp test config root");
        let root = temp_root.keep();
        copy_dir_recursively(&source_root, &root).expect("copy test config fixture");
        root.join("config.ts")
    }

    fn copy_dir_recursively(from: &Path, to: &Path) -> Result<(), std::io::Error> {
        fs::create_dir_all(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            let source = entry.path();
            let destination = to.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                copy_dir_recursively(&source, &destination)?;
            } else {
                fs::copy(&source, &destination)?;
            }
        }
        Ok(())
    }

    #[test]
    fn fallback_source_layout_matches_default_workspace_shape() {
        let layout = fallback_source_layout();

        let SourceLayoutNode::Workspace { children, .. } = layout else {
            panic!("expected workspace fallback root");
        };

        assert_eq!(children.len(), 1);
        assert!(matches!(
            &children[0],
            SourceLayoutNode::Slot {
                window_match: None,
                take: SlotTake::Remaining(RemainingTake::Remaining),
                ..
            }
        ));
    }

    #[test]
    fn layout_failure_diagnostic_reports_fallback_usage() {
        let diagnostic =
            layout_failure_diagnostic(Some(Path::new("layouts/test/index.tsx")), "boom".into());

        assert_eq!(diagnostic.source, "layout");
        assert_eq!(diagnostic.severity, "error");
        assert_eq!(diagnostic.code, "layoutFallback");
        assert_eq!(diagnostic.message, "boom; using fallback layout");
        assert_eq!(diagnostic.path.as_deref(), Some("layouts/test/index.tsx"));
        assert_eq!(
            diagnostic.range,
            LayoutDiagnosticRange { start_line: 1, start_column: 1, end_line: 1, end_column: 1 }
        );
    }

    #[test]
    fn css_failure_diagnostic_is_classified_as_css() {
        let mut service = LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(
            "/home/akisarou/projects/hypreact/dev/test-config/config.ts",
        ))
        .unwrap();
        let diagnostic = css_scene_failure_diagnostic(
            &mut service,
            &PreparedLayout {
                selected: SelectedLayout {
                    name: "test".into(),
                    runtime: hypreact_core::runtime::runtime_kind::RuntimeKind::Js,
                    directory: "layouts/test".into(),
                    module: "layouts/test/index.tsx".into(),
                },
                runtime_payload: serde_json::Value::Null,
                stylesheets: PreparedStylesheets {
                    global: None,
                    layout: Some(hypreact_core::runtime::prepared_layout::PreparedStylesheet {
                        path: "layouts/test/index.css".into(),
                        source: "slot { display: flex; }".into(),
                    }),
                },
                dependencies: vec![],
            },
            "unsupported selector `slot`",
        )
        .expect("css diagnostic");

        assert_eq!(diagnostic.source, "css");
        assert_eq!(diagnostic.code, "unsupportedSelector");
        assert!(diagnostic.message.contains("using fallback layout"));
    }

    #[test]
    fn stylesheet_analysis_cache_reuses_identical_source() {
        let mut service = LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(
            "/home/akisarou/projects/hypreact/dev/test-config/config.ts",
        ))
        .unwrap();

        let first = layout_diagnostics_from_stylesheet(
            &mut service,
            "window { text-align: center; }",
            Some("layouts/test/index.css"),
        );
        let second = layout_diagnostics_from_stylesheet(
            &mut service,
            "window { text-align: center; }",
            Some("layouts/test/index.css"),
        );

        assert_eq!(first, second);
        assert_eq!(service.stylesheet_analysis_cache.len(), 1);
    }

    #[test]
    fn records_layout_to_stylesheet_dependency_edges() {
        let mut service = LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(
            "/home/akisarou/projects/hypreact/dev/test-config/config.ts",
        ))
        .unwrap();
        let artifact = PreparedLayout {
            selected: SelectedLayout {
                name: "master-stack".into(),
                runtime: hypreact_core::runtime::runtime_kind::RuntimeKind::Js,
                directory: "layouts/master-stack".into(),
                module: "layouts/master-stack/index.tsx".into(),
            },
            runtime_payload: serde_json::Value::Null,
            stylesheets: PreparedStylesheets {
                global: Some(hypreact_core::runtime::prepared_layout::PreparedStylesheet {
                    path: "styles/global.css".into(),
                    source: "window { text-align: left; }".into(),
                }),
                layout: Some(hypreact_core::runtime::prepared_layout::PreparedStylesheet {
                    path: "layouts/master-stack/index.css".into(),
                    source: "window { text-align: center; }".into(),
                }),
            },
            dependencies: vec![],
        };

        record_stylesheet_dependencies(&mut service, &artifact);

        let dependents = service
            .artifact_graph
            .dependents_of(&ArtifactKey::layout("master-stack"))
            .cloned()
            .collect::<Vec<_>>();

        assert_eq!(dependents.len(), 2);
        assert!(dependents.contains(&ArtifactKey::stylesheet_analysis("styles/global.css")));
        assert!(
            dependents
                .contains(&ArtifactKey::stylesheet_analysis("layouts/master-stack/index.css"))
        );
    }

    #[test]
    fn records_layout_to_js_graph_and_bytecode_dependency_edges() {
        let mut service = LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(
            "/home/akisarou/projects/hypreact/dev/test-config/config.ts",
        ))
        .unwrap();
        let graph = hypreact_runtime_js_native::JavaScriptModuleGraph {
            entry: "layouts/master-stack/index.js".into(),
            modules: vec![hypreact_runtime_js_native::JavaScriptModule {
                specifier: "layouts/master-stack/index.js".into(),
                source: "export default () => ({ type: 'workspace', children: [] });".into(),
                resolved_imports: BTreeMap::new(),
            }],
        };
        let graph_execution_key = module_graph_execution_key(&graph);
        let artifact = PreparedLayout {
            selected: SelectedLayout {
                name: "master-stack".into(),
                runtime: hypreact_core::runtime::runtime_kind::RuntimeKind::Js,
                directory: "layouts/master-stack".into(),
                module: "layouts/master-stack/index.tsx".into(),
            },
            runtime_payload: hypreact_runtime_js_native::encode_runtime_graph_payload(&graph, &[]),
            stylesheets: PreparedStylesheets::default(),
            dependencies: vec![],
        };

        record_js_runtime_dependencies(&mut service, &artifact);

        let layout_dependents = service
            .artifact_graph
            .dependents_of(&ArtifactKey::layout("master-stack"))
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            layout_dependents.contains(&ArtifactKey::js_module_graph(graph_execution_key.clone()))
        );

        let graph_dependents = service
            .artifact_graph
            .dependents_of(&ArtifactKey::js_module_graph(graph_execution_key.clone()))
            .cloned()
            .collect::<Vec<_>>();
        assert!(graph_dependents.contains(&ArtifactKey::js_bytecode(graph_execution_key)));
    }

    #[test]
    fn records_layout_to_lua_executable_and_bytecode_dependency_edges() {
        let mut service = LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(
            "/home/akisarou/projects/hypreact/dev/test-config/config.ts",
        ))
        .unwrap();
        let source = "local h = require('hypreact') return function(ctx) return h.workspace({ id = 'frame' }) { h.slot({ id = 'master' }) } end";
        let executable_key = lua_executable_artifact_key("layouts/master-stack/index.lua", source);
        let bytecode_key = lua_bytecode_artifact_key("layouts/master-stack/index.lua", source);
        let artifact = PreparedLayout {
            selected: SelectedLayout {
                name: "master-stack".into(),
                runtime: hypreact_core::runtime::runtime_kind::RuntimeKind::Lua,
                directory: "layouts/master-stack".into(),
                module: "layouts/master-stack/index.lua".into(),
            },
            runtime_payload: serde_json::json!({
                "source": source,
                "sourceModule": "layouts/master-stack/index.lua",
            }),
            stylesheets: PreparedStylesheets::default(),
            dependencies: vec![],
        };

        record_lua_runtime_dependencies(&mut service, &artifact);

        let layout_dependents = service
            .artifact_graph
            .dependents_of(&ArtifactKey::layout("master-stack"))
            .cloned()
            .collect::<Vec<_>>();
        assert!(layout_dependents.contains(&ArtifactKey::lua_executable(executable_key.clone())));

        let executable_dependents = service
            .artifact_graph
            .dependents_of(&ArtifactKey::lua_executable(executable_key))
            .cloned()
            .collect::<Vec<_>>();
        assert!(executable_dependents.contains(&ArtifactKey::lua_bytecode(bytecode_key)));
    }

    #[test]
    fn records_fennel_layout_to_compiled_lua_executable_and_bytecode_dependency_edges() {
        let mut service = LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(
            "/home/akisarou/projects/hypreact/dev/test-config/config.ts",
        ))
        .unwrap();
        let compiled_source = "local h = require('hypreact') return function(ctx) return h.workspace({ id = 'frame' }) { h.slot({ id = 'master' }) } end";
        let source_module = "layouts/master-stack/index.fnl";
        let compiled_source_key =
            lua_compiled_source_artifact_key("layouts/master-stack/index.fnl", source_module);
        let executable_key =
            lua_executable_artifact_key("layouts/master-stack/index.fnl", compiled_source);
        let bytecode_key =
            lua_bytecode_artifact_key("layouts/master-stack/index.fnl", compiled_source);
        let artifact = PreparedLayout {
            selected: SelectedLayout {
                name: "master-stack".into(),
                runtime: hypreact_core::runtime::runtime_kind::RuntimeKind::Lua,
                directory: "layouts/master-stack".into(),
                module: "layouts/master-stack/index.fnl".into(),
            },
            runtime_payload: serde_json::json!({
                "source": compiled_source,
                "sourceModule": source_module,
            }),
            stylesheets: PreparedStylesheets::default(),
            dependencies: vec![],
        };

        record_lua_runtime_dependencies(&mut service, &artifact);

        let layout_dependents = service
            .artifact_graph
            .dependents_of(&ArtifactKey::layout("master-stack"))
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            layout_dependents
                .contains(&ArtifactKey::lua_compiled_source(compiled_source_key.clone()))
        );

        let compiled_dependents = service
            .artifact_graph
            .dependents_of(&ArtifactKey::lua_compiled_source(compiled_source_key))
            .cloned()
            .collect::<Vec<_>>();
        assert!(compiled_dependents.contains(&ArtifactKey::lua_executable(executable_key.clone())));

        let executable_dependents = service
            .artifact_graph
            .dependents_of(&ArtifactKey::lua_executable(executable_key))
            .cloned()
            .collect::<Vec<_>>();
        assert!(executable_dependents.contains(&ArtifactKey::lua_bytecode(bytecode_key)));
    }

    #[test]
    fn prepared_cache_staleness_ignores_lua_bytecode_cache_directory() {
        let root = tempfile::TempDir::new().unwrap();
        let authored_root = root.path().join("authored");
        let prepared_root = authored_root.join(".hypreact-build");
        let authored_config = authored_root.join("config.lua");
        let prepared_config = prepared_root.join("config.js");
        let bytecode_dir = prepared_root.join(".lua-bytecode");

        fs::create_dir_all(&bytecode_dir).unwrap();
        fs::write(&authored_config, "return {}\n").unwrap();
        fs::write(&prepared_config, "{}\n").unwrap();

        std::thread::sleep(Duration::from_secs(1));
        fs::write(bytecode_dir.join("cache.luac"), b"bytecode").unwrap();

        assert!(!authored_sources_newer_than_prepared_cache(&authored_config, &prepared_config));
    }

    #[test]
    fn geometry_candidates_preserve_branch_memory_for_master_stack_focus() {
        let geometries = vec![
            FocusTreeWindowGeometry {
                window_id: WindowId::from("master"),
                geometry: WindowGeometry { x: 0, y: 0, width: 600, height: 900 },
            },
            FocusTreeWindowGeometry {
                window_id: WindowId::from("stack-1"),
                geometry: WindowGeometry { x: 600, y: 0, width: 300, height: 300 },
            },
            FocusTreeWindowGeometry {
                window_id: WindowId::from("stack-2"),
                geometry: WindowGeometry { x: 600, y: 300, width: 300, height: 300 },
            },
            FocusTreeWindowGeometry {
                window_id: WindowId::from("stack-3"),
                geometry: WindowGeometry { x: 600, y: 600, width: 300, height: 300 },
            },
        ];

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
        let config_path = "/home/akisarou/projects/hypreact/dev/test-config/config.ts";
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
        let config_path = isolated_test_config_path();
        let mut service =
            LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(&config_path))
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
    fn removing_first_stack_window_collapses_remaining_stack_upward() {
        let config_path = isolated_test_config_path();
        let mut service =
            LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(&config_path))
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

        for id in ["win-terminal", "win-monitor", "win-editor", "win-preview-editor"] {
            let window_id = WindowId::from(id.to_string());
            model.insert_window(
                window_id.clone(),
                Some(WorkspaceId::from("1")),
                Some(OutputId::from("eDP-1")),
            );
            model.set_window_mapped(window_id, true);
        }

        let before = placement_for_workspace(&mut service, &model, "1")
            .expect("initial placement")
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        let top_stack_y = before[&WindowId::from("win-monitor")].y;

        model.remove_window(WindowId::from("win-monitor"));

        let after = placement_for_workspace(&mut service, &model, "1")
            .expect("placement after close")
            .into_iter()
            .collect::<BTreeMap<_, _>>();

        assert_eq!(after[&WindowId::from("win-editor")].y, top_stack_y);
        assert!(
            after[&WindowId::from("win-preview-editor")].y > after[&WindowId::from("win-editor")].y
        );
    }

    #[test]
    fn master_stack_take_change_updates_two_window_placement() {
        let config_path = isolated_test_config_path();
        let layout_path =
            config_path.parent().expect("config root").join("layouts/master-stack/index.tsx");

        let mut service =
            LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(&config_path))
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

        let before = placement_for_workspace(&mut service, &model, "1")
            .expect("placement before take change")
            .into_iter()
            .collect::<BTreeMap<_, _>>();

        let updated = fs::read_to_string(&layout_path)
            .expect("master-stack source")
            .replace("take={1}", "take={2}");
        fs::write(&layout_path, updated).expect("updated master-stack take");

        service.reload_config().expect("reload config after take change");

        let after = placement_for_workspace(&mut service, &model, "1")
            .expect("placement after take change")
            .into_iter()
            .collect::<BTreeMap<_, _>>();

        assert_ne!(before, after, "changing master slot take should change two-window placement");
    }

    #[test]
    fn workspace_scene_derives_partition_tree_for_master_stack_layout() {
        let config_path = isolated_test_config_path();
        let mut service =
            LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(&config_path))
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
        let config_path = isolated_test_config_path();
        let mut service =
            LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(&config_path))
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
        let config_path = isolated_test_config_path();
        let mut service =
            LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(&config_path))
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
        let config_path = isolated_test_config_path();
        let mut service =
            LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(&config_path))
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
        let config_path = isolated_test_config_path();
        let mut service =
            LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(&config_path))
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
        let config_path = "/home/akisarou/projects/hypreact/dev/test-config/config.ts";
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
        let config_path = "/home/akisarou/projects/hypreact/dev/test-config/config.ts";
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
