use crate::runtime_types::{
    HypreactRuntimeHandle, LayoutRuntimeState, LayoutRuntimeStatus, WindowGeometryEntry,
};
use hypreact_core::navigation::NavigationDirection;
use hypreact_layout_runtime::{
    LayoutRuntimePaths, LayoutRuntimeService, LayoutStatusSnapshot, close_focus_candidate,
    directional_focus_candidate, layout_status_for_model, placement_for_workspace,
};

use crate::response::FfiError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementGeometry {
    pub window_id: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub fn load_layout_config(
    handle: &mut HypreactRuntimeHandle,
    config_path: String,
) -> Result<LayoutRuntimeStatus, FfiError> {
    let mut service = LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(
        std::path::PathBuf::from(&config_path),
    ))
    .map_err(|error| FfiError::InvalidJson(error.to_string()))?;
    let _ = service.load_config().map_err(|error| FfiError::InvalidJson(error.to_string()))?;

    handle.layout_runtime =
        Some(LayoutRuntimeState { config_path: std::path::PathBuf::from(config_path), service });

    Ok(layout_runtime_status(handle))
}

pub fn reload_layout_config(
    handle: &mut HypreactRuntimeHandle,
) -> Result<LayoutRuntimeStatus, FfiError> {
    if let Some(layout_runtime) = handle.layout_runtime.as_mut() {
        let _ = layout_runtime
            .service
            .reload_config()
            .map_err(|error| FfiError::InvalidJson(error.to_string()))?;
    }

    Ok(layout_runtime_status(handle))
}

pub fn drain_layout_runtime_source_changes(
    handle: &mut HypreactRuntimeHandle,
) -> Result<bool, FfiError> {
    let Some(layout_runtime) = handle.layout_runtime.as_mut() else {
        return Ok(false);
    };

    layout_runtime
        .service
        .drain_source_changes()
        .map_err(|error| FfiError::InvalidJson(error.to_string()))
}

pub fn layout_runtime_source_change_fd(
    handle: &mut HypreactRuntimeHandle,
) -> Result<i32, FfiError> {
    let Some(layout_runtime) = handle.layout_runtime.as_mut() else {
        return Ok(-1);
    };

    layout_runtime
        .service
        .source_change_fd()
        .map_err(|error| FfiError::InvalidJson(error.to_string()))
}

pub fn layout_runtime_status(handle: &mut HypreactRuntimeHandle) -> LayoutRuntimeStatus {
    let Some(layout_runtime) = handle.layout_runtime.as_mut() else {
        return LayoutRuntimeStatus {
            config_path: None,
            workspace_names: None,
            loaded: false,
            selected_layout_name: None,
            layout: None,
            window_geometries: Vec::new(),
            ordered_window_ids: Vec::new(),
            error: None,
            diagnostics: Vec::new(),
        };
    };

    match layout_status_for_model(&mut layout_runtime.service, &mut handle.model) {
        Ok(status) => map_layout_status(status),
        Err(error) => LayoutRuntimeStatus {
            config_path: Some(layout_runtime.config_path.display().to_string()),
            workspace_names: None,
            loaded: false,
            selected_layout_name: None,
            layout: None,
            window_geometries: Vec::new(),
            ordered_window_ids: Vec::new(),
            error: Some(error.to_string()),
            diagnostics: Vec::new(),
        },
    }
}

pub fn layout_runtime_placement(handle: &mut HypreactRuntimeHandle) -> Vec<PlacementGeometry> {
    layout_runtime_status(handle)
        .window_geometries
        .into_iter()
        .map(|entry| PlacementGeometry {
            window_id: entry.window_id,
            x: entry.x,
            y: entry.y,
            width: entry.width,
            height: entry.height,
        })
        .collect()
}

pub fn layout_runtime_placement_for_workspace(
    handle: &mut HypreactRuntimeHandle,
    workspace_id: &str,
) -> Vec<PlacementGeometry> {
    let Some(layout_runtime) = handle.layout_runtime.as_mut() else {
        return Vec::new();
    };

    match placement_for_workspace(&mut layout_runtime.service, &handle.model, workspace_id) {
        Ok(entries) => entries
            .into_iter()
            .map(|(window_id, geometry)| PlacementGeometry {
                window_id: window_id.to_string(),
                x: geometry.x,
                y: geometry.y,
                width: geometry.width,
                height: geometry.height,
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

pub fn layout_focus_candidate(
    handle: &mut HypreactRuntimeHandle,
    direction: &str,
) -> Result<Option<String>, FfiError> {
    let Some(layout_runtime) = handle.layout_runtime.as_mut() else {
        return Ok(None);
    };

    let direction = match direction {
        "left" => NavigationDirection::Left,
        "right" => NavigationDirection::Right,
        "up" => NavigationDirection::Up,
        "down" => NavigationDirection::Down,
        other => return Err(FfiError::InvalidJson(format!("unknown focus direction: {other}"))),
    };

    directional_focus_candidate(&mut layout_runtime.service, &mut handle.model, direction)
        .map(|candidate| candidate.map(|window_id| window_id.to_string()))
        .map_err(|error| FfiError::InvalidJson(error.to_string()))
}

pub fn layout_close_focus_candidate(
    handle: &mut HypreactRuntimeHandle,
    window_id: &str,
) -> Result<Option<String>, FfiError> {
    let window_id = hypreact_core::WindowId::from(window_id.to_string());
    Ok(close_focus_candidate(&handle.model, &window_id).map(|window_id| window_id.to_string()))
}

fn map_layout_status(status: LayoutStatusSnapshot) -> LayoutRuntimeStatus {
    LayoutRuntimeStatus {
        config_path: status.config_path,
        workspace_names: status.workspace_names,
        loaded: status.loaded,
        selected_layout_name: status.selected_layout_name,
        layout: status.layout,
        window_geometries: status
            .window_geometries
            .into_iter()
            .map(|(window_id, geometry)| WindowGeometryEntry {
                window_id: window_id.to_string(),
                x: geometry.x,
                y: geometry.y,
                width: geometry.width,
                height: geometry.height,
            })
            .collect(),
        ordered_window_ids: status
            .ordered_window_ids
            .into_iter()
            .map(|window_id| window_id.to_string())
            .collect(),
        error: status.error,
        diagnostics: status.diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::layout_focus_candidate;
    use crate::runtime_types::{HypreactRuntimeHandle, LayoutRuntimeState};
    use hypreact_config::model::{Config, LayoutRule};
    use hypreact_core::wm::WmModel;
    use hypreact_core::{OutputId, WindowId, WorkspaceId};
    use hypreact_layout_runtime::{LayoutRuntimePaths, LayoutRuntimeService};

    #[test]
    fn later_layout_rule_overrides_earlier_matching_rule() {
        let config = Config {
            default_layout: Some("default-layout".to_string()),
            layout_rules: vec![
                LayoutRule {
                    layout: "master-stack".to_string(),
                    monitor: Some("eDP-1".to_string()),
                    ..LayoutRule::default()
                },
                LayoutRule {
                    layout: "primary-stack".to_string(),
                    index: Some(1),
                    monitor: Some("eDP-1".to_string()),
                    ..LayoutRule::default()
                },
            ],
            ..Config::default()
        };

        let layout = config.selected_layout_ref_for_workspace("2", Some(&OutputId::from("eDP-1")));

        assert_eq!(layout.map(|layout| layout.name), Some("primary-stack".to_string()));
    }

    #[test]
    fn layout_focus_candidate_persists_scene_focus_tree_on_model() {
        let config_path = "/home/akisarou/projects/hypreact/dev/test-config/config.ts";
        let service =
            LayoutRuntimeService::new(LayoutRuntimePaths::from_authored_config(config_path))
                .expect("layout runtime service");

        let mut model = WmModel::default();
        model.upsert_output(OutputId::from("eDP-1"), "eDP-1".to_string(), 1600, 1000, None);
        model.upsert_workspace(WorkspaceId::from("1"), "1".to_string());
        model.attach_workspace_to_output(WorkspaceId::from("1"), OutputId::from("eDP-1"));
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

        model.set_window_focused(Some(WindowId::from("master")));

        let mut handle = HypreactRuntimeHandle {
            model,
            layout_runtime: Some(LayoutRuntimeState { config_path: config_path.into(), service }),
        };

        let _ = layout_focus_candidate(&mut handle, "right").expect("focus candidate query");

        let focus_tree = handle.model.focus_tree.as_ref().expect("scene-derived focus tree");
        assert!(focus_tree.scope_path(&WindowId::from("master")).is_some());
        assert!(focus_tree.scope_path(&WindowId::from("stack-a")).is_some());
        assert!(focus_tree.scope_path(&WindowId::from("stack-b")).is_some());
    }
}
