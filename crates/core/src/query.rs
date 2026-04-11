use crate::snapshot::{OutputSnapshot, StateSnapshot, WindowSnapshot, WorkspaceSnapshot};
use crate::types::{WindowMode, WindowShell};
use crate::wm::{OutputModel, WindowModel, WmModel, WorkspaceModel};

pub fn state_snapshot_for_model(model: &WmModel) -> StateSnapshot {
    let outputs: Vec<OutputSnapshot> = model.outputs.values().map(output_snapshot).collect();
    let workspace_names = model.workspace_names();
    let workspaces: Vec<WorkspaceSnapshot> = model
        .workspaces
        .values()
        .map(|workspace| workspace_snapshot(model, workspace))
        .collect();
    let windows: Vec<WindowSnapshot> = model
        .windows
        .values()
        .map(|window| window_snapshot(model, window))
        .collect();
    let visible_window_ids = model.visible_window_ids();

    StateSnapshot {
        focused_window_id: model.focused_window_id().cloned(),
        current_output_id: model.current_output_id().cloned(),
        current_workspace_id: model.current_workspace_id().cloned(),
        outputs,
        workspaces,
        windows,
        visible_window_ids,
        workspace_names,
    }
}

pub fn output_snapshot(output: &OutputModel) -> OutputSnapshot {
    OutputSnapshot {
        id: output.id.clone(),
        name: output.name.clone(),
        logical_width: output.logical_width,
        logical_height: output.logical_height,
        scale: 1,
        enabled: output.enabled,
        current_workspace_id: output.focused_workspace_id.clone(),
    }
}

pub fn workspace_snapshot(model: &WmModel, workspace: &WorkspaceModel) -> WorkspaceSnapshot {
    WorkspaceSnapshot {
        id: workspace.id.clone(),
        name: workspace.name.clone(),
        output_id: workspace.output_id.clone(),
        active_workspaces: model.active_workspace_names(workspace),
        focused: workspace.focused,
        visible: workspace.visible,
        effective_layout: workspace.effective_layout.clone(),
    }
}

pub fn window_snapshot(model: &WmModel, window: &WindowModel) -> WindowSnapshot {
    WindowSnapshot {
        id: window.id.clone(),
        shell: if window.is_xwayland {
            WindowShell::Xwayland
        } else {
            WindowShell::Wayland
        },
        app_id: window.app_id.clone(),
        title: window.title.clone(),
        class: window.class.clone(),
        instance: window.instance.clone(),
        role: None,
        window_type: None,
        mapped: window.mapped,
        mode: window_mode(window),
        focused: window.focused,
        urgent: false,
        closing: window.closing,
        output_id: window.output_id.clone(),
        workspace_id: window.workspace_id.clone(),
        workspaces: model.workspace_names_for_window(&window.id),
    }
}

pub fn window_mode(window: &WindowModel) -> WindowMode {
    if window.fullscreen {
        WindowMode::Fullscreen
    } else if window.floating {
        WindowMode::Floating { rect: None }
    } else {
        WindowMode::Tiled
    }
}

#[cfg(test)]
mod tests {
    use crate::window_id;
    use crate::wm::WmModel;
    use crate::{OutputId, WorkspaceId};

    use super::*;

    fn sample_model() -> WmModel {
        let mut model = WmModel::default();

        model.upsert_workspace(WorkspaceId::from("1"), "1".into());
        model.upsert_workspace(WorkspaceId::from("2"), "2".into());
        model.set_current_workspace(WorkspaceId::from("1"));
        model.upsert_output(
            OutputId::from("output-1"),
            "HDMI-A-1",
            1920,
            1080,
            Some(WorkspaceId::from("1")),
        );
        model.attach_workspace_to_output(WorkspaceId::from("1"), OutputId::from("output-1"));
        model.set_current_output(OutputId::from("output-1"));
        model.insert_window(
            window_id(1),
            Some(WorkspaceId::from("1")),
            Some(OutputId::from("output-1")),
        );
        let window = model.windows.get_mut(&window_id(1)).unwrap();
        window.app_id = Some("foot".into());
        window.title = Some("terminal".into());
        window.mapped = true;
        window.focused = true;
        model.focused_window_id = Some(window_id(1));

        model
    }

    #[test]
    fn state_snapshot_tracks_query_state() {
        let snapshot = state_snapshot_for_model(&sample_model());

        assert_eq!(snapshot.current_workspace_id, Some(WorkspaceId::from("1")));
        assert_eq!(snapshot.current_output_id, Some(OutputId::from("output-1")));
        assert_eq!(snapshot.focused_window_id, Some(window_id(1)));
        assert_eq!(
            snapshot.workspace_names,
            vec!["1".to_string(), "2".to_string()]
        );
        assert_eq!(snapshot.visible_window_ids, vec![window_id(1)]);
        assert_eq!(snapshot.outputs.len(), 1);
        assert_eq!(snapshot.workspaces.len(), 2);
        assert_eq!(snapshot.windows.len(), 1);
        assert_eq!(snapshot.windows[0].mode, WindowMode::Tiled);
    }
}
