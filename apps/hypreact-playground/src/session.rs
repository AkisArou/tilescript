use hypreact_config::model::Config;
use hypreact_core::command::{FocusDirection, LayoutCycleDirection, WmCommand};
use hypreact_core::host::{HostAction, dispatch_wm_command};
use hypreact_core::query::state_snapshot_for_model;
use hypreact_core::snapshot::{StateSnapshot, WindowSnapshot};
use hypreact_core::types::LayoutRef;
use hypreact_core::window_id;
use hypreact_core::wm::{DrawableSpace, WmModel};
use hypreact_core::workspace::{ensure_default_workspace, ensure_workspace};
use hypreact_core::{OutputId, WindowId, WorkspaceId};
use hypreact_scene::SceneResponse;

use crate::layout_runtime::{EvaluatedPreview, apply_layout_selection};

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
            diagnostics: Vec::new(),
            event_log: vec!["preview booted from template source bundle".to_string()],
            last_action: "boot preview".to_string(),
            actions: Vec::new(),
            error: None,
            rendered_layout_name: None,
        }
    }

    pub fn apply_loaded_config(&mut self, config: &Config) {
        apply_layout_selection(&mut self.model, config);
    }

    pub fn apply_loaded_preview(&mut self, preview: EvaluatedPreview) {
        self.scene = preview.scene;
        self.diagnostics = preview.diagnostics;
        self.error = preview.error;
        self.rendered_layout_name = preview.selected_layout_name;
    }

    pub fn apply_preview_failure(&mut self, error: String) {
        self.scene = None;
        self.error = Some(error);
    }

    pub fn workspace_names(&self) -> Vec<String> {
        self.model.workspaces.values().map(|workspace| workspace.name.clone()).collect()
    }

    pub fn active_workspace_name(&self) -> String {
        self.model
            .current_workspace_id()
            .and_then(|workspace_id| self.model.workspaces.get(workspace_id))
            .map(|workspace| workspace.name.clone())
            .unwrap_or_else(|| "none".to_string())
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

    pub fn visible_window_count(&self) -> usize {
        self.visible_windows().len()
    }

    pub fn state_snapshot(&self) -> StateSnapshot {
        state_snapshot_for_model(&self.model)
    }

    pub fn session_summary_rows(&self) -> Vec<(String, String)> {
        let snapshot = self.state_snapshot();
        let output = snapshot.current_output().cloned();
        let workspace = snapshot.current_workspace().cloned();

        vec![
            (
                "workspace id".to_string(),
                workspace
                    .as_ref()
                    .map(|workspace| workspace.id.as_str().to_string())
                    .unwrap_or_else(|| "none".to_string()),
            ),
            (
                "output".to_string(),
                output
                    .as_ref()
                    .map(|output| output.name.clone())
                    .unwrap_or_else(|| "none".to_string()),
            ),
            (
                "visible ids".to_string(),
                if snapshot.visible_window_ids.is_empty() {
                    "none".to_string()
                } else {
                    snapshot
                        .visible_window_ids
                        .iter()
                        .map(|id| id.as_str().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                },
            ),
        ]
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

        self.actions = actions.clone();
        self.last_action = label.clone();

        for action in actions {
            apply_host_action(&mut self.model, action, config);
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

    insert_demo_window(&mut model, "win-terminal", &workspace_one, &output_id, "Terminal", "foot");
    insert_demo_window(&mut model, "win-browser", &workspace_one, &output_id, "Browser", "zen");
    insert_demo_window(&mut model, "win-editor", &workspace_one, &output_id, "Editor", "zed");
    insert_demo_window(&mut model, "win-chat", &workspace_two, &output_id, "Chat", "discord");
    insert_demo_window(&mut model, "win-notes", &workspace_two, &output_id, "Notes", "obsidian");

    model.set_window_focused(Some(window_id("win-editor")));
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
        HostAction::ReloadConfig | HostAction::SpawnCommand { .. } => {}
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
                model.remove_window(window_id);
                let ordered = model.ordered_focusable_window_ids_on_current_workspace(Vec::new());
                model.set_window_focused(ordered.last().cloned());
            }
        }
        HostAction::FocusDirection { direction } => {
            focus_window_by_direction(model, direction);
        }
        HostAction::SwapDirection { .. }
        | HostAction::MoveDirection { .. }
        | HostAction::ResizeDirection { .. } => {}
    }
}

fn activate_workspace(model: &mut WmModel, workspace_id: &str) {
    match workspace_id {
        "e+1" => cycle_workspace(model, 1),
        "e-1" => cycle_workspace(model, -1),
        other => model.set_current_workspace(WorkspaceId::from(other)),
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

fn focus_window_by_direction(model: &mut WmModel, direction: FocusDirection) {
    match direction {
        FocusDirection::Left | FocusDirection::Up => focus_window_by_offset(model, -1),
        FocusDirection::Right | FocusDirection::Down => focus_window_by_offset(model, 1),
    }
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
