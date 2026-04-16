use hypreact_config::model::Config;
use hypreact_core::command::{FocusDirection, LayoutCycleDirection, WmCommand};
use hypreact_core::focus::{
    FocusTree, FocusTreeWindowGeometry, preferred_focus_after_removing_window, set_focused_window,
};
use hypreact_core::host::{HostAction, dispatch_wm_command};
use hypreact_core::navigation::{
    NavigationDirection, WindowGeometryCandidate, select_directional_focus_candidate,
};
use hypreact_core::query::state_snapshot_for_model;
use hypreact_core::resize::{
    ResizeDirection, apply_resize_step, gc_resize_state, select_resize_candidate,
};
use hypreact_core::snapshot::WindowSnapshot;
use hypreact_core::types::LayoutRef;
use hypreact_core::window_id;
use hypreact_core::wm::{DrawableSpace, WindowGeometry, WmModel};
use hypreact_core::workspace::{ensure_default_workspace, ensure_workspace};
use hypreact_core::{OutputId, WindowId, WorkspaceId};
use hypreact_scene::SceneResponse;
use std::collections::BTreeMap;

use crate::layout_runtime::{
    EvaluatedPreview, apply_layout_selection, resize_step_units_for_partition,
};

struct PreviewWindowSeed {
    id_prefix: &'static str,
    title: &'static str,
    app_id: &'static str,
}

const PRIORITY_WINDOW_APPS: [PreviewWindowSeed; 2] = [
    PreviewWindowSeed {
        id_prefix: "win-preview-editor",
        title: "Playground Editor",
        app_id: "playground-editor",
    },
    PreviewWindowSeed { id_prefix: "win-binds", title: "Keybinds", app_id: "binds" },
];

const RANDOM_WINDOW_APPS: [PreviewWindowSeed; 3] = [
    PreviewWindowSeed { id_prefix: "random-nvim", title: "Editor", app_id: "nvim" },
    PreviewWindowSeed { id_prefix: "random-htop", title: "Process Monitor", app_id: "htop" },
    PreviewWindowSeed { id_prefix: "random-foot", title: "Terminal", app_id: "foot" },
];

const DEFAULT_PREVIEW_WORKSPACE: &str = "1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewDiagnostic {
    pub path: String,
    pub severity: &'static str,
    pub code: &'static str,
    pub message: String,
    pub range: String,
}

#[derive(Debug, Clone)]
pub struct PreviewSessionState {
    pub model: WmModel,
    pub scene: Option<SceneResponse>,
    pub partition_tree: Option<hypreact_core::resize::PartitionTree>,
    pub manual_layout_by_workspace: BTreeMap<WorkspaceId, LayoutRef>,
    pub diagnostics: Vec<PreviewDiagnostic>,
    pub event_log: Vec<String>,
    pub last_action: String,
    pub actions: Vec<HostAction>,
    pub error: Option<String>,
    pub rendered_layout_name: Option<String>,
}

impl PreviewSessionState {
    pub fn new() -> Self {
        Self {
            model: build_demo_model(),
            scene: None,
            partition_tree: None,
            manual_layout_by_workspace: BTreeMap::new(),
            diagnostics: Vec::new(),
            event_log: vec!["preview booted from starter source bundle".to_string()],
            last_action: "boot preview".to_string(),
            actions: Vec::new(),
            error: None,
            rendered_layout_name: None,
        }
    }

    pub fn apply_loaded_config(&mut self, config: &Config) {
        apply_layout_selection(&mut self.model, config);
        for (workspace_id, layout) in &self.manual_layout_by_workspace {
            self.model.set_workspace_effective_layout(workspace_id.clone(), Some(layout.clone()));
        }
    }

    pub fn apply_loaded_preview(&mut self, preview: EvaluatedPreview) {
        self.model.set_focus_tree_value(preview.focus_tree.clone());
        self.scene = preview.scene;
        self.partition_tree = preview.partition_tree;
        self.diagnostics = preview.diagnostics;
        self.error = preview.error;
        self.rendered_layout_name = preview.selected_layout_name;
    }

    pub fn apply_preview_failure(&mut self, error: String) {
        self.error = Some(error);
    }

    pub fn active_workspace_name(&self) -> String {
        self.model
            .current_workspace_id()
            .and_then(|workspace_id| self.model.workspaces.get(workspace_id))
            .map(|workspace| workspace.name.clone())
            .unwrap_or_else(|| "none".to_string())
    }

    pub fn workspace_names(&self) -> Vec<String> {
        self.model.workspaces.values().map(|workspace| workspace.name.clone()).collect()
    }

    pub fn active_layout_name(&self) -> String {
        self.model
            .current_workspace_id()
            .and_then(|workspace_id| self.model.workspaces.get(workspace_id))
            .and_then(|workspace| workspace.effective_layout.as_ref())
            .map(|layout| layout.name.clone())
            .unwrap_or_else(|| "none".to_string())
    }

    pub fn focused_window_id(&self) -> Option<WindowId> {
        self.model.focused_window_id().cloned()
    }

    pub fn visible_windows(&self) -> Vec<WindowSnapshot> {
        let snapshot = state_snapshot_for_model(&self.model);
        snapshot
            .windows
            .into_iter()
            .filter(|window| snapshot.visible_window_ids.iter().any(|id| id == &window.id))
            .collect()
    }

    pub fn window_name(&self, window_id: &WindowId) -> String {
        self.model
            .windows
            .get(window_id)
            .map(|window| {
                let title = window.title.as_deref().unwrap_or(window_id.as_str());
                window
                    .app_id
                    .as_deref()
                    .map(|app_id| format!("{app_id} ({title})"))
                    .unwrap_or_else(|| title.to_string())
            })
            .unwrap_or_else(|| window_id.as_str().to_string())
    }

    pub fn select_workspace(&mut self, workspace_name: &str) {
        let command =
            WmCommand::SelectWorkspace { workspace_id: WorkspaceId::from(workspace_name) };
        self.apply_command(command, None);
    }

    pub fn set_layout(&mut self, layout_name: String) {
        self.apply_command(WmCommand::SetLayout { name: layout_name }, None);
    }

    pub fn set_focus(&mut self, window_id: WindowId) {
        self.apply_command(WmCommand::FocusWindow { window_id }, None);
    }

    pub fn apply_command(&mut self, command: WmCommand, config: Option<&Config>) {
        let label = display_command_label(&command);
        let actions = dispatch_wm_command(command.clone());
        let previous_workspace_id = self.model.current_workspace_id().cloned();

        self.actions = actions.clone();
        self.last_action = label.clone();

        for action in actions {
            apply_host_action_with_session(self, action, config);
        }

        let current_workspace_id = self.model.current_workspace_id().cloned();
        if current_workspace_id != previous_workspace_id {
            prune_empty_preview_workspaces(&mut self.model, previous_workspace_id.as_ref());
        }

        self.push_log(format!(
            "{} -> focused={} workspace={} layout={}",
            label,
            self.focused_window_id()
                .as_ref()
                .map(|window_id| self.window_name(window_id))
                .unwrap_or_else(|| "none".to_string()),
            self.active_workspace_name(),
            self.active_layout_name(),
        ));
    }

    fn push_log(&mut self, line: String) {
        self.event_log.push(line);
        if self.event_log.len() > 24 {
            let keep_from = self.event_log.len().saturating_sub(24);
            self.event_log = self.event_log.split_off(keep_from);
        }
    }
}

fn build_demo_model() -> WmModel {
    let mut model = WmModel::default();
    let output_id = OutputId::from("preview-output");
    let workspace_one = ensure_default_workspace(&mut model, "1");
    let workspace_two = ensure_workspace(&mut model, "2");

    model.upsert_output(output_id.clone(), "Preview", 1440, 900, Some(workspace_one.clone()));
    model.attach_workspace_to_output(workspace_one.clone(), output_id.clone());
    model.attach_workspace_to_output(workspace_two.clone(), output_id.clone());
    model.set_current_output(output_id.clone());
    model.set_current_workspace(workspace_one.clone());
    model.set_workspace_layout_space(
        workspace_one.clone(),
        Some(DrawableSpace { width: 1240, height: 760 }),
    );
    model.set_workspace_layout_space(
        workspace_two.clone(),
        Some(DrawableSpace { width: 1240, height: 760 }),
    );

    insert_demo_window(
        &mut model,
        "win-preview-editor",
        &workspace_one,
        &output_id,
        "Playground Editor",
        "playground-editor",
    );
    insert_demo_window(&mut model, "win-binds", &workspace_one, &output_id, "Keybinds", "binds");
    insert_demo_window(
        &mut model,
        "win-secondary-foot",
        &workspace_two,
        &output_id,
        "Terminal",
        "foot",
    );
    insert_demo_window(
        &mut model,
        "win-secondary-htop",
        &workspace_two,
        &output_id,
        "Process Monitor",
        "htop",
    );

    model.set_window_focused(Some(window_id("win-preview-editor")));
    model
}

fn insert_demo_window(
    model: &mut WmModel,
    id: &str,
    workspace_id: &WorkspaceId,
    output_id: &OutputId,
    title: &str,
    app_id: &str,
) {
    let window_id = window_id(id);
    model.insert_window(window_id.clone(), Some(workspace_id.clone()), Some(output_id.clone()));
    model.set_window_mapped(window_id.clone(), true);
    model.set_window_identity(
        window_id,
        Some(title.to_string()),
        Some(app_id.to_string()),
        Some(app_id.to_string()),
        Some(app_id.to_string()),
        None,
        None,
        false,
    );
}

fn apply_host_action(model: &mut WmModel, action: HostAction, config: Option<&Config>) {
    match action {
        HostAction::ReloadConfig => {}
        HostAction::SpawnCommand { command } => {
            if command == "$openRandom" || command == "$randomWindow" {
                spawn_random_window(model);
            }
        }
        HostAction::SetLayout { name } => {
            if let Some(workspace_id) = model.current_workspace_id().cloned() {
                model.set_workspace_effective_layout(workspace_id, Some(LayoutRef { name }));
            }
        }
        HostAction::CycleLayout { direction } => {
            cycle_layout(model, config, direction.unwrap_or(LayoutCycleDirection::Next));
        }
        HostAction::ActivateWorkspace { workspace_id } => {
            activate_workspace(model, &workspace_id);
        }
        HostAction::ToggleFloating => {
            if let Some(window_id) = model.focused_window_id().cloned() {
                let enabled =
                    model.windows.get(&window_id).map(|window| window.floating).unwrap_or(false);
                model.set_window_floating(window_id, !enabled);
            }
        }
        HostAction::ToggleFullscreen => {
            if let Some(window_id) = model.focused_window_id().cloned() {
                let enabled =
                    model.windows.get(&window_id).map(|window| window.fullscreen).unwrap_or(false);
                model.set_window_fullscreen(window_id, !enabled);
            }
        }
        HostAction::FocusWindow { window_id } => {
            model.set_window_focused(Some(WindowId::from(window_id.as_str())));
        }
        HostAction::FocusNextWindow => {
            focus_window_by_offset(model, 1);
        }
        HostAction::FocusPreviousWindow => {
            focus_window_by_offset(model, -1);
        }
        HostAction::AssignFocusedWindowToWorkspace { workspace } => {
            if let Some(window_id) = model.focused_window_id().cloned() {
                model.set_window_workspace(
                    window_id,
                    Some(WorkspaceId::from(workspace.to_string())),
                );
            }
        }
        HostAction::ToggleAssignFocusedWindowToWorkspace { workspace } => {
            if let Some(window_id) = model.focused_window_id().cloned() {
                let target = WorkspaceId::from(workspace.to_string());
                let already_on_target =
                    model.windows.get(&window_id).and_then(|window| window.workspace_id.as_ref())
                        == Some(&target);
                let next_workspace = if already_on_target {
                    model.current_workspace_id().cloned()
                } else {
                    Some(target)
                };
                model.set_window_workspace(window_id, next_workspace);
            }
        }
        HostAction::CloseFocusedWindow => {
            if let Some(window_id) = model.focused_window_id().cloned() {
                let next_focus =
                    preferred_focus_after_removing_window(model, &window_id, Vec::new());
                model.remove_window(window_id);
                model.set_window_focused(next_focus);
            }
        }
        HostAction::FocusDirection { direction } => {
            let _ = config;
            focus_window_by_direction(model, direction, None);
        }
        HostAction::SwapDirection { .. } => {}
        HostAction::MoveDirection { direction } => {
            let _ = config;
            move_focused_window_by_direction(model, direction, None);
        }
        HostAction::ResizeDirection { direction } => {
            resize_focused_window_by_direction(model, direction, config);
        }
    }
}

fn apply_host_action_with_session(
    state: &mut PreviewSessionState,
    action: HostAction,
    config: Option<&Config>,
) {
    if let HostAction::SetLayout { name } = &action {
        if let Some(workspace_id) = state.model.current_workspace_id().cloned() {
            state.manual_layout_by_workspace.insert(workspace_id, LayoutRef { name: name.clone() });
        }
    }

    match action {
        HostAction::FocusDirection { direction } => {
            focus_preview_window_by_direction(state, direction);
        }
        HostAction::MoveDirection { direction } => {
            move_preview_window_by_direction(state, direction);
        }
        HostAction::ResizeDirection { direction } => {
            let _ = config;
            resize_preview_window_by_direction(state, direction);
        }
        other => apply_host_action(&mut state.model, other, config),
    }
}

fn activate_workspace(model: &mut WmModel, workspace_id: &str) {
    match workspace_id {
        "e+1" => cycle_workspace(model, 1),
        "e-1" => cycle_workspace(model, -1),
        other => {
            let target = ensure_preview_workspace(model, other);
            model.set_current_workspace(target);
            let preferred_focus = model.preferred_focus_window_on_current_workspace(Vec::new());
            model.set_window_focused(preferred_focus);
        }
    }
}

fn cycle_workspace(model: &mut WmModel, delta: isize) {
    let workspace_ids = model.workspaces.keys().cloned().collect::<Vec<_>>();
    if workspace_ids.is_empty() {
        return;
    }

    let current_index = model
        .current_workspace_id()
        .and_then(|current| workspace_ids.iter().position(|candidate| candidate == current))
        .unwrap_or(0) as isize;
    let len = workspace_ids.len() as isize;
    let next_index = (current_index + delta).rem_euclid(len) as usize;
    model.set_current_workspace(workspace_ids[next_index].clone());
    let preferred_focus = model.preferred_focus_window_on_current_workspace(Vec::new());
    model.set_window_focused(preferred_focus);
}

fn focus_window_by_offset(model: &mut WmModel, delta: isize) {
    let ordered = model.ordered_focusable_window_ids_on_current_workspace(Vec::new());
    if ordered.is_empty() {
        return;
    }
    let current_index = model
        .focused_window_id()
        .and_then(|focused| ordered.iter().position(|candidate| candidate == focused))
        .unwrap_or(0) as isize;
    let len = ordered.len() as isize;
    let next_index = (current_index + delta).rem_euclid(len) as usize;
    model.set_window_focused(Some(ordered[next_index].clone()));
}

fn focus_window_by_direction(
    model: &mut WmModel,
    direction: FocusDirection,
    scene: Option<&SceneResponse>,
) {
    let Some(candidates) = directional_candidates(model, scene) else {
        match direction {
            FocusDirection::Left | FocusDirection::Up => focus_window_by_offset(model, -1),
            FocusDirection::Right | FocusDirection::Down => focus_window_by_offset(model, 1),
        }
        return;
    };

    let next = select_directional_focus_candidate(
        &candidates,
        model.focused_window_id().cloned(),
        navigation_direction(direction),
        &model.last_focused_window_id_by_scope,
        model.focus_tree.as_ref(),
    );
    let _ = set_focused_window(model, next);
}

pub fn focus_preview_window_by_direction(
    state: &mut PreviewSessionState,
    direction: FocusDirection,
) {
    focus_window_by_direction(&mut state.model, direction, state.scene.as_ref());
}

fn move_focused_window_by_direction(
    model: &mut WmModel,
    direction: FocusDirection,
    scene: Option<&SceneResponse>,
) {
    let Some(focused_id) = model.focused_window_id().cloned() else {
        return;
    };
    let Some(candidates) = directional_candidates(model, scene) else {
        return;
    };

    let Some(target_id) = select_directional_focus_candidate(
        &candidates,
        Some(focused_id.clone()),
        navigation_direction(direction),
        &model.last_focused_window_id_by_scope,
        model.focus_tree.as_ref(),
    ) else {
        return;
    };

    if target_id != focused_id && model.move_tiled_window(&focused_id, &target_id) {
        model.set_window_focused(Some(focused_id));
    }
}

pub fn move_preview_window_by_direction(
    state: &mut PreviewSessionState,
    direction: FocusDirection,
) {
    move_focused_window_by_direction(&mut state.model, direction, state.scene.as_ref());
}

fn resize_focused_window_by_direction(
    _model: &mut WmModel,
    direction: FocusDirection,
    _config: Option<&Config>,
) {
    let _ = resize_direction(direction);
}

pub fn resize_preview_window_by_direction(
    state: &mut PreviewSessionState,
    direction: FocusDirection,
) {
    let Some(workspace_id) = state.model.current_workspace_id().cloned() else {
        return;
    };
    let Some(focused_window_id) = state.model.focused_window_id().cloned() else {
        return;
    };
    let Some(partition_tree) = state.partition_tree.clone() else {
        return;
    };

    let resize_state = state.model.workspace_resize_state_mut(&workspace_id);
    gc_resize_state(resize_state, &partition_tree);
    let Some(candidate) =
        select_resize_candidate(&partition_tree, &focused_window_id, resize_direction(direction))
    else {
        return;
    };

    let step_units =
        resize_step_units_for_partition(&partition_tree, &candidate.partition_id, 96.0);
    let _ = apply_resize_step(resize_state, &partition_tree, &candidate, step_units);
}

fn directional_candidates(
    model: &WmModel,
    scene: Option<&SceneResponse>,
) -> Option<Vec<WindowGeometryCandidate>> {
    let scene = scene?;
    let window_geometries = scene
        .root
        .window_nodes()
        .into_iter()
        .filter_map(|node| {
            let hypreact_scene::LayoutSnapshotNode::Window {
                window_id: Some(window_id), rect, ..
            } = node
            else {
                return None;
            };
            model.window_is_focus_cycle_candidate(window_id).then_some(FocusTreeWindowGeometry {
                window_id: window_id.clone(),
                geometry: WindowGeometry {
                    x: rect.x.round() as i32,
                    y: rect.y.round() as i32,
                    width: rect.width.round() as i32,
                    height: rect.height.round() as i32,
                },
            })
        })
        .collect::<Vec<_>>();

    if window_geometries.is_empty() {
        return None;
    }

    let focus_tree = FocusTree::from_window_geometries(&window_geometries);

    Some(
        window_geometries
            .into_iter()
            .map(|entry| WindowGeometryCandidate {
                window_id: entry.window_id.clone(),
                geometry: entry.geometry,
                scope_path: focus_tree.scope_path(&entry.window_id).unwrap_or(&[]).to_vec(),
            })
            .collect(),
    )
}

fn navigation_direction(direction: FocusDirection) -> NavigationDirection {
    match direction {
        FocusDirection::Left => NavigationDirection::Left,
        FocusDirection::Right => NavigationDirection::Right,
        FocusDirection::Up => NavigationDirection::Up,
        FocusDirection::Down => NavigationDirection::Down,
    }
}

fn resize_direction(direction: FocusDirection) -> ResizeDirection {
    match direction {
        FocusDirection::Left => ResizeDirection::Left,
        FocusDirection::Right => ResizeDirection::Right,
        FocusDirection::Up => ResizeDirection::Up,
        FocusDirection::Down => ResizeDirection::Down,
    }
}

fn spawn_random_window(model: &mut WmModel) {
    let workspace_id = model.current_workspace_id().cloned();
    let output_id = model.current_output_id().cloned();
    let Some(selection) = select_next_spawn_window(model, workspace_id.as_ref()) else {
        return;
    };

    let window_id =
        if PRIORITY_WINDOW_APPS.iter().any(|candidate| candidate.app_id == selection.app_id) {
            window_id(selection.id_prefix)
        } else {
            let next_index = next_random_window_index(model, selection.id_prefix);
            window_id(format!("{}-{next_index}", selection.id_prefix))
        };

    model.insert_window(window_id.clone(), workspace_id, output_id.clone());
    model.set_window_mapped(window_id.clone(), true);
    model.set_window_identity(
        window_id.clone(),
        Some(selection.title.to_string()),
        Some(selection.app_id.to_string()),
        Some(selection.app_id.to_string()),
        Some(selection.app_id.to_string()),
        None,
        None,
        false,
    );
    model.set_window_focused(Some(window_id));
}

fn select_next_spawn_window<'a>(
    model: &WmModel,
    workspace_id: Option<&WorkspaceId>,
) -> Option<&'a PreviewWindowSeed>
where
    'static: 'a,
{
    for candidate in &PRIORITY_WINDOW_APPS {
        if !workspace_has_app(model, workspace_id, candidate.app_id) {
            return Some(candidate);
        }
    }

    let current_count = workspace_window_count(model, workspace_id);
    RANDOM_WINDOW_APPS.get(current_count % RANDOM_WINDOW_APPS.len())
}

fn workspace_has_app(model: &WmModel, workspace_id: Option<&WorkspaceId>, app_id: &str) -> bool {
    model.windows.values().any(|window| {
        window.workspace_id.as_ref() == workspace_id && window.app_id.as_deref() == Some(app_id)
    })
}

fn workspace_window_count(model: &WmModel, workspace_id: Option<&WorkspaceId>) -> usize {
    model.windows.values().filter(|window| window.workspace_id.as_ref() == workspace_id).count()
}

fn ensure_preview_workspace(model: &mut WmModel, workspace_name: &str) -> WorkspaceId {
    let workspace_id = WorkspaceId::from(workspace_name);
    if model.workspaces.contains_key(&workspace_id) {
        return workspace_id;
    }

    let layout_template = model
        .current_workspace_id()
        .and_then(|workspace_id| model.workspaces.get(workspace_id))
        .cloned()
        .or_else(|| model.workspaces.get(&WorkspaceId::from(DEFAULT_PREVIEW_WORKSPACE)).cloned());

    let workspace_id = ensure_workspace(model, workspace_name.to_string());

    if let Some(output_id) = model.current_output_id().cloned() {
        model.attach_workspace_to_output(workspace_id.clone(), output_id);
    }

    if let Some(template) = layout_template {
        model.set_workspace_layout_space(workspace_id.clone(), template.layout_space);
        model.set_workspace_effective_layout(workspace_id.clone(), template.effective_layout);
    }

    workspace_id
}

fn prune_empty_preview_workspaces(
    model: &mut WmModel,
    previous_workspace_id: Option<&WorkspaceId>,
) {
    ensure_preview_workspace(model, DEFAULT_PREVIEW_WORKSPACE);

    let empty_workspace_ids = model
        .workspaces
        .keys()
        .filter(|workspace_id| workspace_id.as_str() != DEFAULT_PREVIEW_WORKSPACE)
        .filter(|workspace_id| Some(*workspace_id) == previous_workspace_id)
        .filter(|workspace_id| {
            !model
                .windows
                .values()
                .any(|window| window.workspace_id.as_ref() == Some(*workspace_id))
        })
        .cloned()
        .collect::<Vec<_>>();

    for workspace_id in empty_workspace_ids {
        model.workspaces.remove(&workspace_id);
        model.tiled_window_order_by_workspace.remove(&workspace_id);
        model.gc_resize_state_for_known_workspaces();
    }
}

fn next_random_window_index(model: &WmModel, prefix: &str) -> usize {
    model
        .windows
        .keys()
        .filter_map(|window_id| {
            let raw = window_id.as_str();
            raw.strip_prefix(prefix)
                .and_then(|suffix| suffix.strip_prefix('-'))
                .and_then(|suffix| suffix.parse::<usize>().ok())
        })
        .max()
        .map(|index| index + 1)
        .unwrap_or(1)
}

fn cycle_layout(model: &mut WmModel, config: Option<&Config>, direction: LayoutCycleDirection) {
    let Some(config) = config else {
        return;
    };
    let Some(current_workspace_id) = model.current_workspace_id().cloned() else {
        return;
    };

    let layouts = config.layouts.iter().map(|layout| layout.name.clone()).collect::<Vec<_>>();
    if layouts.is_empty() {
        return;
    }

    let current_name = model
        .workspaces
        .get(&current_workspace_id)
        .and_then(|workspace| workspace.effective_layout.as_ref())
        .map(|layout| layout.name.as_str());
    let current_index = current_name
        .and_then(|name| layouts.iter().position(|candidate| candidate == name))
        .unwrap_or(0);

    let next_index = match direction {
        LayoutCycleDirection::Next => (current_index + 1) % layouts.len(),
        LayoutCycleDirection::Previous => current_index.checked_sub(1).unwrap_or(layouts.len() - 1),
    };

    model.set_workspace_effective_layout(
        current_workspace_id,
        Some(LayoutRef { name: layouts[next_index].clone() }),
    );
}

fn display_command_label(command: &WmCommand) -> String {
    match command {
        WmCommand::ReloadConfig => "reload config".to_string(),
        WmCommand::SetLayout { name } => format!("set layout {name}"),
        WmCommand::CycleLayout { .. } => "cycle layout".to_string(),
        WmCommand::ViewWorkspace { workspace } => format!("view workspace {workspace}"),
        WmCommand::ActivateWorkspace { workspace_id } => {
            format!("activate workspace {}", workspace_id.as_str())
        }
        WmCommand::SelectWorkspace { workspace_id } => {
            format!("select workspace {}", workspace_id.as_str())
        }
        WmCommand::SelectNextWorkspace => "select next workspace".to_string(),
        WmCommand::SelectPreviousWorkspace => "select previous workspace".to_string(),
        WmCommand::ToggleFloating => "toggle floating".to_string(),
        WmCommand::ToggleFullscreen => "toggle fullscreen".to_string(),
        WmCommand::FocusWindow { window_id } => format!("focus {}", window_id.as_str()),
        WmCommand::FocusNextWindow => "focus next window".to_string(),
        WmCommand::FocusPreviousWindow => "focus previous window".to_string(),
        WmCommand::CloseFocusedWindow => "close focused window".to_string(),
        other => format!("{other:?}"),
    }
}
