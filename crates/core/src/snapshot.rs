use serde::{Deserialize, Serialize};

use crate::resize::{ResizeState, WorkspaceResizeState};
use crate::runtime::layout_context::{
    LayoutEvaluationContext, LayoutMonitorContext, LayoutStateContext, LayoutWindowContext,
    LayoutWorkspaceContext,
};
use crate::runtime::prepared_layout::SelectedLayout;
#[cfg(test)]
use crate::runtime::runtime_kind::RuntimeKind;
use crate::types::{LayoutRef, WindowMode, WindowShell};
use crate::wm::DrawableSpace;
use crate::{LayoutSpace, OutputId, WindowId, WorkspaceId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowSnapshot {
    pub id: WindowId,
    pub shell: WindowShell,
    pub app_id: Option<String>,
    pub title: Option<String>,
    pub class: Option<String>,
    pub instance: Option<String>,
    pub role: Option<String>,
    pub window_type: Option<String>,
    pub mapped: bool,
    #[serde(default)]
    pub mode: WindowMode,
    pub focused: bool,
    pub urgent: bool,
    #[serde(default)]
    pub closing: bool,
    pub output_id: Option<OutputId>,
    pub workspace_id: Option<WorkspaceId>,
    pub workspaces: Vec<String>,
}

impl WindowSnapshot {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    pub id: WorkspaceId,
    pub name: String,
    pub output_id: Option<OutputId>,
    pub layout_space: Option<DrawableSpace>,
    pub active_workspaces: Vec<String>,
    pub focused: bool,
    pub visible: bool,
    pub effective_layout: Option<LayoutRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputSnapshot {
    pub id: OutputId,
    pub name: String,
    pub logical_width: u32,
    pub logical_height: u32,
    pub scale: u32,
    pub enabled: bool,
    pub current_workspace_id: Option<WorkspaceId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub focused_window_id: Option<WindowId>,
    pub current_output_id: Option<OutputId>,
    pub current_workspace_id: Option<WorkspaceId>,
    pub outputs: Vec<OutputSnapshot>,
    pub workspaces: Vec<WorkspaceSnapshot>,
    pub windows: Vec<WindowSnapshot>,
    pub visible_window_ids: Vec<WindowId>,
    pub workspace_names: Vec<String>,
    #[serde(default)]
    pub resize_state: ResizeState,
}

impl StateSnapshot {
    pub fn current_workspace(&self) -> Option<&WorkspaceSnapshot> {
        self.current_workspace_id.as_ref().and_then(|workspace_id| {
            self.workspaces.iter().find(|workspace| &workspace.id == workspace_id)
        })
    }

    pub fn current_output(&self) -> Option<&OutputSnapshot> {
        self.current_output_id
            .as_ref()
            .and_then(|output_id| self.outputs.iter().find(|output| &output.id == output_id))
    }

    pub fn output_by_id(&self, output_id: &OutputId) -> Option<&OutputSnapshot> {
        self.outputs.iter().find(|output| &output.id == output_id)
    }

    pub fn workspace_resize_state(&self, workspace_id: &WorkspaceId) -> WorkspaceResizeState {
        self.resize_state.by_workspace_id.get(workspace_id).cloned().unwrap_or_default()
    }

    pub fn filtered_for_output(
        &self,
        visible_window_ids: &[WindowId],
        output_id: &OutputId,
    ) -> Option<StateSnapshot> {
        let mut snapshot = self.clone();
        snapshot.current_output_id = Some(output_id.clone());
        snapshot.windows.retain(|window| {
            visible_window_ids.iter().any(|id| id == &window.id)
                && window.output_id.as_ref() == Some(output_id)
        });
        snapshot.visible_window_ids =
            snapshot.windows.iter().map(|window| window.id.clone()).collect();
        snapshot.workspaces.iter_mut().for_each(|workspace| {
            workspace.focused = workspace.output_id.as_ref() == Some(output_id);
            workspace.visible = workspace.output_id.as_ref() == Some(output_id);
        });

        let output_workspace = snapshot
            .workspaces
            .iter()
            .find(|workspace| workspace.output_id.as_ref() == Some(output_id))
            .cloned()?;
        snapshot.current_workspace_id = Some(output_workspace.id);

        Some(snapshot)
    }

    fn windows_for_workspace(&self, workspace: &WorkspaceSnapshot) -> Vec<WindowSnapshot> {
        let mut windows = self
            .windows
            .iter()
            .filter(|window| {
                let matches_workspace = window.workspace_id.as_ref() == Some(&workspace.id);
                let matches_output = workspace
                    .output_id
                    .as_ref()
                    .is_none_or(|output_id| window.output_id.as_ref() == Some(output_id));
                let is_visible = self.visible_window_ids.is_empty()
                    || self.visible_window_ids.iter().any(|id| id == &window.id);
                let is_layout_eligible = window.mapped
                    && !window.closing
                    && !window.mode.is_floating()
                    && !window.mode.is_fullscreen();

                matches_workspace && matches_output && is_visible && is_layout_eligible
            })
            .cloned()
            .collect::<Vec<_>>();

        windows.sort_by_key(|window| {
            self.windows
                .iter()
                .position(|candidate| candidate.id == window.id)
                .unwrap_or(self.windows.len())
        });

        windows
    }

    fn layout_space_for_workspace(&self, workspace: &WorkspaceSnapshot) -> LayoutSpace {
        if let Some(layout_space) = workspace.layout_space {
            return LayoutSpace {
                width: layout_space.width as f32,
                height: layout_space.height as f32,
            };
        }

        let output = workspace
            .output_id
            .as_ref()
            .and_then(|output_id| self.output_by_id(output_id))
            .or_else(|| self.current_output());

        LayoutSpace {
            width: output.map(|output| output.logical_width as f32).unwrap_or_default(),
            height: output.map(|output| output.logical_height as f32).unwrap_or_default(),
        }
    }

    pub fn layout_context(
        &self,
        workspace: &WorkspaceSnapshot,
        selected_layout: Option<SelectedLayout>,
    ) -> LayoutEvaluationContext {
        let windows = self.windows_for_workspace(workspace);
        let output = workspace
            .output_id
            .as_ref()
            .and_then(|output_id| self.output_by_id(output_id))
            .or_else(|| self.current_output())
            .cloned();
        let layout_space = workspace.layout_space;
        let selected_layout_name = selected_layout.as_ref().map(|layout| layout.name.clone());

        LayoutEvaluationContext {
            monitor: LayoutMonitorContext {
                name: output.as_ref().map(|output| output.name.clone()).unwrap_or_default(),
                width: layout_space
                    .map(|layout_space| layout_space.width as u32)
                    .or_else(|| output.as_ref().map(|output| output.logical_width))
                    .unwrap_or(0),
                height: layout_space
                    .map(|layout_space| layout_space.height as u32)
                    .or_else(|| output.as_ref().map(|output| output.logical_height))
                    .unwrap_or(0),
                scale: output.as_ref().map(|output| output.scale),
            },
            workspace: LayoutWorkspaceContext {
                name: workspace.name.clone(),
                workspaces: workspace.active_workspaces.clone(),
                window_count: windows.len(),
            },
            windows: windows
                .iter()
                .map(|window| LayoutWindowContext {
                    id: window.id.clone(),
                    app_id: window.app_id.clone(),
                    title: window.title.clone(),
                    class: window.class.clone(),
                    instance: window.instance.clone(),
                    role: window.role.clone(),
                    shell: Some(match window.shell {
                        WindowShell::Wayland => "wayland".into(),
                        WindowShell::Xwayland => "xwayland".into(),
                    }),
                    window_type: window.window_type.clone(),
                    floating: window.mode.is_floating(),
                    fullscreen: window.mode.is_fullscreen(),
                    focused: window.focused,
                })
                .collect(),
            state: Some(LayoutStateContext {
                focused_window_id: self.focused_window_id.clone(),
                current_output_id: self.current_output_id.clone(),
                current_workspace_id: self.current_workspace_id.clone(),
                visible_window_ids: self.visible_window_ids.clone(),
                workspace_names: self.workspace_names.clone(),
                selected_layout_name: selected_layout_name.clone(),
                resize_state: self.workspace_resize_state(&workspace.id),
            }),
            workspace_id: workspace.id.clone(),
            output,
            selected_layout_name,
            space: self.layout_space_for_workspace(workspace),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_snapshot_resolves_current_workspace_output_and_space() {
        let state = StateSnapshot {
            focused_window_id: None,
            current_output_id: Some(OutputId::from("out-1")),
            current_workspace_id: Some(WorkspaceId::from("ws-1")),
            outputs: vec![OutputSnapshot {
                id: OutputId::from("out-1"),
                name: "HDMI-A-1".into(),
                logical_width: 1920,
                logical_height: 1080,
                scale: 1,
                enabled: true,
                current_workspace_id: Some(WorkspaceId::from("ws-1")),
            }],
            workspaces: vec![WorkspaceSnapshot {
                id: WorkspaceId::from("ws-1"),
                name: "1".into(),
                output_id: Some(OutputId::from("out-1")),
                layout_space: None,
                active_workspaces: vec!["1".into()],
                focused: true,
                visible: true,
                effective_layout: Some(LayoutRef { name: "master-stack".into() }),
            }],
            windows: vec![],
            visible_window_ids: vec![],
            workspace_names: vec!["1".into()],
            resize_state: crate::resize::ResizeState::default(),
        };

        let workspace = state.current_workspace().unwrap();
        let output = state.current_output().unwrap();
        let space = state.layout_space_for_workspace(workspace);

        assert_eq!(workspace.id, WorkspaceId::from("ws-1"));
        assert_eq!(output.id, OutputId::from("out-1"));
        assert_eq!(space.width, 1920.0);
        assert_eq!(space.height, 1080.0);
    }

    #[test]
    fn state_snapshot_builds_layout_evaluation_context() {
        let state = StateSnapshot {
            focused_window_id: None,
            current_output_id: Some(OutputId::from("out-1")),
            current_workspace_id: Some(WorkspaceId::from("ws-1")),
            outputs: vec![OutputSnapshot {
                id: OutputId::from("out-1"),
                name: "HDMI-A-1".into(),
                logical_width: 1920,
                logical_height: 1080,
                scale: 1,
                enabled: true,
                current_workspace_id: Some(WorkspaceId::from("ws-1")),
            }],
            workspaces: vec![WorkspaceSnapshot {
                id: WorkspaceId::from("ws-1"),
                name: "1".into(),
                output_id: Some(OutputId::from("out-1")),
                layout_space: Some(DrawableSpace { width: 1920, height: 1040 }),
                active_workspaces: vec!["1".into()],
                focused: true,
                visible: true,
                effective_layout: Some(LayoutRef { name: "master-stack".into() }),
            }],
            windows: vec![],
            visible_window_ids: vec![],
            workspace_names: vec!["1".into()],
            resize_state: crate::resize::ResizeState::default(),
        };
        let workspace = state.current_workspace().unwrap();

        let context = state.layout_context(
            workspace,
            Some(SelectedLayout {
                name: "master-stack".into(),
                runtime: RuntimeKind::Js,
                directory: "layouts/master-stack".into(),
                module: "layouts/master-stack.js".into(),
            }),
        );

        assert_eq!(context.workspace_id, WorkspaceId::from("ws-1"));
        assert_eq!(context.output.unwrap().id, OutputId::from("out-1"));
        assert_eq!(context.space.width, 1920.0);
        assert_eq!(context.space.height, 1040.0);
        assert_eq!(context.monitor.height, 1040);
        assert_eq!(context.workspace.name, "1");
        assert_eq!(context.workspace.window_count, 0);
    }

    #[test]
    fn state_snapshot_layout_context_falls_back_to_current_output() {
        let state = StateSnapshot {
            focused_window_id: None,
            current_output_id: Some(OutputId::from("out-1")),
            current_workspace_id: Some(WorkspaceId::from("ws-1")),
            outputs: vec![OutputSnapshot {
                id: OutputId::from("out-1"),
                name: "HDMI-A-1".into(),
                logical_width: 1920,
                logical_height: 1080,
                scale: 1,
                enabled: true,
                current_workspace_id: Some(WorkspaceId::from("ws-1")),
            }],
            workspaces: vec![WorkspaceSnapshot {
                id: WorkspaceId::from("ws-1"),
                name: "1".into(),
                output_id: None,
                layout_space: None,
                active_workspaces: vec!["1".into()],
                focused: true,
                visible: true,
                effective_layout: Some(LayoutRef { name: "master-stack".into() }),
            }],
            windows: vec![],
            visible_window_ids: vec![],
            workspace_names: vec!["1".into()],
            resize_state: crate::resize::ResizeState::default(),
        };
        let workspace = state.current_workspace().unwrap();

        let context = state.layout_context(
            workspace,
            Some(SelectedLayout {
                name: "master-stack".into(),
                runtime: RuntimeKind::Js,
                directory: "layouts/master-stack".into(),
                module: "layouts/master-stack.js".into(),
            }),
        );

        assert_eq!(context.output.unwrap().id, OutputId::from("out-1"));
        assert_eq!(context.space.width, 1920.0);
        assert_eq!(context.space.height, 1080.0);
    }

    #[test]
    fn state_snapshot_filters_windows_for_workspace_visibility() {
        let state = StateSnapshot {
            focused_window_id: Some(WindowId::from("w1")),
            current_output_id: Some(OutputId::from("out-1")),
            current_workspace_id: Some(WorkspaceId::from("ws-1")),
            outputs: vec![OutputSnapshot {
                id: OutputId::from("out-1"),
                name: "HDMI-A-1".into(),
                logical_width: 1920,
                logical_height: 1080,
                scale: 1,
                enabled: true,
                current_workspace_id: Some(WorkspaceId::from("ws-1")),
            }],
            workspaces: vec![WorkspaceSnapshot {
                id: WorkspaceId::from("ws-1"),
                name: "1".into(),
                output_id: Some(OutputId::from("out-1")),
                layout_space: None,
                active_workspaces: vec!["1".into()],
                focused: true,
                visible: true,
                effective_layout: Some(LayoutRef { name: "master-stack".into() }),
            }],
            windows: vec![
                WindowSnapshot {
                    id: WindowId::from("w1"),
                    shell: WindowShell::Wayland,
                    app_id: Some("firefox".into()),
                    title: Some("Firefox".into()),
                    class: None,
                    instance: None,
                    role: None,
                    window_type: None,
                    mapped: true,
                    mode: WindowMode::Tiled,
                    focused: true,
                    urgent: false,
                    closing: false,
                    output_id: Some(OutputId::from("out-1")),
                    workspace_id: Some(WorkspaceId::from("ws-1")),
                    workspaces: vec!["1".into()],
                },
                WindowSnapshot {
                    id: WindowId::from("w2"),
                    shell: WindowShell::Wayland,
                    app_id: Some("alacritty".into()),
                    title: Some("Terminal".into()),
                    class: None,
                    instance: None,
                    role: None,
                    window_type: None,
                    mapped: true,
                    mode: WindowMode::Tiled,
                    focused: false,
                    urgent: false,
                    closing: false,
                    output_id: Some(OutputId::from("out-1")),
                    workspace_id: Some(WorkspaceId::from("ws-1")),
                    workspaces: vec!["1".into()],
                },
                WindowSnapshot {
                    id: WindowId::from("w3"),
                    shell: WindowShell::Wayland,
                    app_id: Some("discord".into()),
                    title: Some("Discord".into()),
                    class: None,
                    instance: None,
                    role: None,
                    window_type: None,
                    mapped: true,
                    mode: WindowMode::Tiled,
                    focused: false,
                    urgent: false,
                    closing: false,
                    output_id: Some(OutputId::from("out-2")),
                    workspace_id: Some(WorkspaceId::from("ws-2")),
                    workspaces: vec!["2".into()],
                },
            ],
            visible_window_ids: vec![WindowId::from("w1")],
            workspace_names: vec!["1".into(), "2".into()],
            resize_state: crate::resize::ResizeState::default(),
        };

        let windows = state.windows_for_workspace(state.current_workspace().unwrap());

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].id, WindowId::from("w1"));
    }

    #[test]
    fn state_snapshot_can_be_filtered_for_specific_output() {
        let state = StateSnapshot {
            focused_window_id: Some(WindowId::from("w1")),
            current_output_id: Some(OutputId::from("out-1")),
            current_workspace_id: Some(WorkspaceId::from("ws-1")),
            outputs: vec![
                OutputSnapshot {
                    id: OutputId::from("out-1"),
                    name: "HDMI-A-1".into(),
                    logical_width: 1920,
                    logical_height: 1080,
                    scale: 1,
                    enabled: true,
                    current_workspace_id: Some(WorkspaceId::from("ws-1")),
                },
                OutputSnapshot {
                    id: OutputId::from("out-2"),
                    name: "DP-1".into(),
                    logical_width: 1280,
                    logical_height: 720,
                    scale: 1,
                    enabled: true,
                    current_workspace_id: Some(WorkspaceId::from("ws-2")),
                },
            ],
            workspaces: vec![
                WorkspaceSnapshot {
                    id: WorkspaceId::from("ws-1"),
                    name: "1".into(),
                    output_id: Some(OutputId::from("out-1")),
                    layout_space: None,
                    active_workspaces: vec!["1".into()],
                    focused: true,
                    visible: true,
                    effective_layout: Some(LayoutRef { name: "master-stack".into() }),
                },
                WorkspaceSnapshot {
                    id: WorkspaceId::from("ws-2"),
                    name: "2".into(),
                    output_id: Some(OutputId::from("out-2")),
                    layout_space: None,
                    active_workspaces: vec!["2".into()],
                    focused: false,
                    visible: true,
                    effective_layout: Some(LayoutRef { name: "master-stack".into() }),
                },
            ],
            windows: vec![
                WindowSnapshot {
                    id: WindowId::from("w1"),
                    shell: WindowShell::Wayland,
                    app_id: None,
                    title: None,
                    class: None,
                    instance: None,
                    role: None,
                    window_type: None,
                    mapped: true,
                    mode: WindowMode::Tiled,
                    focused: true,
                    urgent: false,
                    closing: false,
                    output_id: Some(OutputId::from("out-1")),
                    workspace_id: Some(WorkspaceId::from("ws-1")),
                    workspaces: vec!["1".into()],
                },
                WindowSnapshot {
                    id: WindowId::from("w2"),
                    shell: WindowShell::Wayland,
                    app_id: None,
                    title: None,
                    class: None,
                    instance: None,
                    role: None,
                    window_type: None,
                    mapped: true,
                    mode: WindowMode::Tiled,
                    focused: false,
                    urgent: false,
                    closing: false,
                    output_id: Some(OutputId::from("out-2")),
                    workspace_id: Some(WorkspaceId::from("ws-2")),
                    workspaces: vec!["2".into()],
                },
            ],
            visible_window_ids: vec![WindowId::from("w1"), WindowId::from("w2")],
            workspace_names: vec!["1".into(), "2".into()],
            resize_state: crate::resize::ResizeState::default(),
        };

        let filtered = state
            .filtered_for_output(&state.visible_window_ids, &OutputId::from("out-2"))
            .expect("filtered state should exist for output");

        assert_eq!(filtered.current_output_id, Some(OutputId::from("out-2")));
        assert_eq!(filtered.current_workspace_id, Some(WorkspaceId::from("ws-2")));
        assert_eq!(filtered.visible_window_ids, vec![WindowId::from("w2")]);
        assert_eq!(filtered.windows.len(), 1);
        assert_eq!(filtered.windows[0].id, WindowId::from("w2"));
    }

    #[test]
    fn layout_context_excludes_floating_and_fullscreen_windows() {
        let state = StateSnapshot {
            focused_window_id: Some(WindowId::from("tiled")),
            current_output_id: Some(OutputId::from("out-1")),
            current_workspace_id: Some(WorkspaceId::from("ws-1")),
            outputs: vec![OutputSnapshot {
                id: OutputId::from("out-1"),
                name: "HDMI-A-1".into(),
                logical_width: 1920,
                logical_height: 1080,
                scale: 1,
                enabled: true,
                current_workspace_id: Some(WorkspaceId::from("ws-1")),
            }],
            workspaces: vec![WorkspaceSnapshot {
                id: WorkspaceId::from("ws-1"),
                name: "1".into(),
                output_id: Some(OutputId::from("out-1")),
                layout_space: None,
                active_workspaces: vec!["1".into()],
                focused: true,
                visible: true,
                effective_layout: Some(LayoutRef { name: "master-stack".into() }),
            }],
            windows: vec![
                WindowSnapshot {
                    id: WindowId::from("tiled"),
                    shell: WindowShell::Wayland,
                    app_id: Some("foot".into()),
                    title: Some("Terminal".into()),
                    class: None,
                    instance: None,
                    role: None,
                    window_type: None,
                    mapped: true,
                    mode: WindowMode::Tiled,
                    focused: true,
                    urgent: false,
                    closing: false,
                    output_id: Some(OutputId::from("out-1")),
                    workspace_id: Some(WorkspaceId::from("ws-1")),
                    workspaces: vec!["1".into()],
                },
                WindowSnapshot {
                    id: WindowId::from("floating"),
                    shell: WindowShell::Wayland,
                    app_id: Some("pavucontrol".into()),
                    title: Some("Volume".into()),
                    class: None,
                    instance: None,
                    role: None,
                    window_type: None,
                    mapped: true,
                    mode: WindowMode::Floating { rect: None },
                    focused: false,
                    urgent: false,
                    closing: false,
                    output_id: Some(OutputId::from("out-1")),
                    workspace_id: Some(WorkspaceId::from("ws-1")),
                    workspaces: vec!["1".into()],
                },
                WindowSnapshot {
                    id: WindowId::from("fullscreen"),
                    shell: WindowShell::Wayland,
                    app_id: Some("mpv".into()),
                    title: Some("Video".into()),
                    class: None,
                    instance: None,
                    role: None,
                    window_type: None,
                    mapped: true,
                    mode: WindowMode::Fullscreen,
                    focused: false,
                    urgent: false,
                    closing: false,
                    output_id: Some(OutputId::from("out-1")),
                    workspace_id: Some(WorkspaceId::from("ws-1")),
                    workspaces: vec!["1".into()],
                },
            ],
            visible_window_ids: vec![
                WindowId::from("tiled"),
                WindowId::from("floating"),
                WindowId::from("fullscreen"),
            ],
            workspace_names: vec!["1".into()],
            resize_state: crate::resize::ResizeState::default(),
        };

        let workspace = state.current_workspace().unwrap();
        let context = state.layout_context(workspace, None);

        assert_eq!(context.workspace.window_count, 1);
        assert_eq!(context.windows.len(), 1);
        assert_eq!(context.windows[0].id, WindowId::from("tiled"));
    }
}
