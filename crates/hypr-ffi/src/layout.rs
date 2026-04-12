use hypreact_config::model::Config;
use hypreact_core::OutputId;
use hypreact_core::navigation::{NavigationDirection, select_directional_focus_candidate};
use hypreact_core::query::state_snapshot_for_model;
use hypreact_core::types::LayoutRef;
use hypreact_core::wm::WmModel;

use crate::response::FfiError;
use crate::types::{HypreactRuntimeHandle, LayoutRuntimeState, LayoutRuntimeStatus, WindowGeometryEntry};

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
    let service = hypreact_layout_runtime::LayoutRuntimeService::new(
        hypreact_layout_runtime::LayoutRuntimePaths::from_authored_config(std::path::PathBuf::from(&config_path)),
    )
    .map_err(|error| FfiError::InvalidJson(error.to_string()))?;

    handle.layout_runtime = Some(LayoutRuntimeState {
        config_path: std::path::PathBuf::from(config_path),
        service,
    });

    Ok(layout_runtime_status(handle))
}

pub fn reload_layout_config(handle: &mut HypreactRuntimeHandle) -> Result<LayoutRuntimeStatus, FfiError> {
    if let Some(layout_runtime) = handle.layout_runtime.as_mut() {
        let _ = layout_runtime
            .service
            .reload_config()
            .map_err(|error| FfiError::InvalidJson(error.to_string()))?;
    }

    Ok(layout_runtime_status(handle))
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
        };
    };

    layout_runtime_status_for_model(layout_runtime, &mut handle.model)
}

fn layout_runtime_status_for_model(
    layout_runtime: &mut LayoutRuntimeState,
    model: &mut WmModel,
) -> LayoutRuntimeStatus {
    let config_path = Some(layout_runtime.config_path.display().to_string());
    let loaded = match layout_runtime.service.load_config() {
        Ok(loaded) => loaded,
        Err(error) => {
            return LayoutRuntimeStatus {
                config_path,
                workspace_names: None,
                loaded: false,
                selected_layout_name: None,
                layout: None,
                window_geometries: Vec::new(),
                ordered_window_ids: Vec::new(),
                error: Some(error.to_string()),
            };
        }
    };

    apply_layout_selection_to_model(model, &loaded.config);
    let snapshot = state_snapshot_for_model(model);
    let workspace = snapshot.current_workspace().cloned();

    let Some(workspace) = workspace else {
        return LayoutRuntimeStatus {
            config_path,
            workspace_names: Some(snapshot.workspace_names.clone()),
            loaded: true,
            selected_layout_name: None,
            layout: None,
            window_geometries: Vec::new(),
            ordered_window_ids: Vec::new(),
            error: None,
        };
    };

    match layout_runtime
        .service
        .evaluate_workspace_scene(&loaded.config, &snapshot, &workspace)
    {
        Ok(evaluation) => {
            if let Some(evaluation) = evaluation.as_ref() {
                model.set_focus_tree_value(Some(evaluation.focus_tree.clone()));
            }

            LayoutRuntimeStatus {
                config_path,
                workspace_names: Some(snapshot.workspace_names.clone()),
                loaded: true,
                selected_layout_name: evaluation
                    .as_ref()
                    .map(|evaluation| evaluation.evaluation.artifact.selected.name.clone())
                    .or_else(|| workspace.effective_layout.as_ref().map(|layout| layout.name.clone())),
                layout: evaluation
                    .as_ref()
                    .map(|evaluation| evaluation.evaluation.layout.clone()),
                window_geometries: evaluation
                    .as_ref()
                    .map(|evaluation| {
                        evaluation
                            .window_geometries
                            .iter()
                            .map(|(window_id, geometry)| WindowGeometryEntry {
                                window_id: window_id.to_string(),
                                x: geometry.x,
                                y: geometry.y,
                                width: geometry.width,
                                height: geometry.height,
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                ordered_window_ids: evaluation
                    .as_ref()
                    .map(|evaluation| {
                        evaluation
                            .ordered_window_ids
                            .iter()
                            .map(|window_id| window_id.to_string())
                            .collect()
                    })
                    .unwrap_or_default(),
                error: None,
            }
        }
        Err(error) => LayoutRuntimeStatus {
            config_path,
            workspace_names: Some(snapshot.workspace_names.clone()),
            loaded: false,
            selected_layout_name: workspace.effective_layout.as_ref().map(|layout| layout.name.clone()),
            layout: None,
            window_geometries: Vec::new(),
            ordered_window_ids: Vec::new(),
            error: Some(error.to_string()),
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
    let Some(target_workspace) = handle
        .model
        .workspaces
        .keys()
        .find(|id| id.as_str() == workspace_id)
        .cloned()
    else {
        return Vec::new();
    };

    let mut model = handle.model.clone();
    let target_output = model
        .workspaces
        .get(&target_workspace)
        .and_then(|workspace| workspace.output_id.clone());

    model.set_current_workspace(target_workspace);
    if let Some(target_output) = target_output {
        model.set_current_output(target_output);
    }

    layout_runtime_status_for_model(layout_runtime, &mut model)
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

pub fn layout_focus_candidate(
    handle: &mut HypreactRuntimeHandle,
    direction: &str,
) -> Result<Option<String>, FfiError> {
    let Some(layout_runtime) = handle.layout_runtime.as_mut() else {
        return Ok(None);
    };
    let loaded = layout_runtime
        .service
        .load_config()
        .map_err(|error| FfiError::InvalidJson(error.to_string()))?;

    apply_layout_selection_to_model(&mut handle.model, &loaded.config);
    let snapshot = state_snapshot_for_model(&handle.model);
    let Some(workspace) = snapshot.current_workspace().cloned() else {
        return Ok(None);
    };
    let Some(scene) = layout_runtime
        .service
        .evaluate_workspace_scene(&loaded.config, &snapshot, &workspace)
        .map_err(|error| FfiError::InvalidJson(error.to_string()))?
    else {
        return Ok(None);
    };

    handle.model.set_focus_tree_value(Some(scene.focus_tree.clone()));

    let direction = match direction {
        "left" => NavigationDirection::Left,
        "right" => NavigationDirection::Right,
        "up" => NavigationDirection::Up,
        "down" => NavigationDirection::Down,
        other => return Err(FfiError::InvalidJson(format!("unknown focus direction: {other}"))),
    };

    Ok(select_directional_focus_candidate(
        &scene.geometry_candidates,
        snapshot.focused_window_id,
        direction,
        &handle.model.last_focused_window_id_by_scope,
        handle.model.focus_tree.as_ref(),
    )
    .map(|window_id| window_id.to_string()))
}

fn apply_layout_selection_to_model(model: &mut WmModel, config: &Config) {
    let current_output_id = model.current_output_id().cloned();
    let workspace_names = model.workspace_names();

    for workspace in model.workspaces.values_mut() {
        workspace.effective_layout = selected_layout_ref_for_workspace(
            config,
            &workspace.name,
            workspace.output_id.as_ref().or(current_output_id.as_ref()),
            &workspace_names,
        );
    }
}

fn selected_layout_ref_for_workspace(
    config: &Config,
    workspace_name: &str,
    output_id: Option<&OutputId>,
    workspace_names: &[String],
) -> Option<LayoutRef> {
    let workspace_index = workspace_names.iter().position(|name| name == workspace_name);
    if let Some(index) = workspace_index
        && let Some(layout_name) = config.layout_selection.per_workspace.get(index)
    {
        return Some(LayoutRef {
            name: layout_name.clone(),
        });
    }

    if let Some(output_id) = output_id
        && let Some(layout_name) = config.layout_selection.per_monitor.get(output_id.as_str())
    {
        return Some(LayoutRef {
            name: layout_name.clone(),
        });
    }

    config
        .layout_selection
        .default
        .clone()
        .map(|name| LayoutRef { name })
}

#[cfg(test)]
mod tests {
    use super::{layout_focus_candidate, selected_layout_ref_for_workspace};
    use hypreact_config::model::{Config, LayoutSelectionConfig};
    use hypreact_core::{OutputId, WorkspaceId, WindowId};
    use hypreact_core::wm::WmModel;
    use crate::types::{HypreactRuntimeHandle, LayoutRuntimeState};
    use std::collections::BTreeMap;

    #[test]
    fn per_workspace_takes_precedence_over_per_monitor() {
        let mut per_monitor = BTreeMap::new();
        per_monitor.insert("eDP-1".to_string(), "master-stack".to_string());

        let config = Config {
            layout_selection: LayoutSelectionConfig {
                default: Some("default-layout".to_string()),
                per_workspace: vec!["master-stack".to_string(), "primary-stack".to_string()],
                per_monitor,
            },
            ..Config::default()
        };

        let layout = selected_layout_ref_for_workspace(
            &config,
            "2",
            Some(&OutputId::from("eDP-1")),
            &["1".to_string(), "2".to_string()],
        );

        assert_eq!(layout.map(|layout| layout.name), Some("primary-stack".to_string()));
    }

    #[test]
    fn layout_focus_candidate_persists_scene_focus_tree_on_model() {
        let config_path = "/home/akisarou/projects/hypreact/test_config/test_config/config.ts";
        let service = hypreact_layout_runtime::LayoutRuntimeService::new(
            hypreact_layout_runtime::LayoutRuntimePaths::from_authored_config(config_path),
        )
        .expect("layout runtime service");

        let mut model = WmModel::default();
        model.upsert_output(OutputId::from("eDP-1"), "eDP-1".to_string(), 1600, 1000, None);
        model.upsert_workspace(WorkspaceId::from("1"), "1".to_string());
        model.attach_workspace_to_output(WorkspaceId::from("1"), OutputId::from("eDP-1"));
        model.set_current_output(OutputId::from("eDP-1"));
        model.set_current_workspace(WorkspaceId::from("1"));

        for id in ["master", "stack-a", "stack-b"] {
            let window_id = WindowId::from(id.to_string());
            model.insert_window(window_id.clone(), Some(WorkspaceId::from("1")), Some(OutputId::from("eDP-1")));
            model.set_window_mapped(window_id, true);
        }

        model.set_window_focused(Some(WindowId::from("master")));

        let mut handle = HypreactRuntimeHandle {
            model,
            layout_runtime: Some(LayoutRuntimeState {
                config_path: config_path.into(),
                service,
            }),
        };

        let _ = layout_focus_candidate(&mut handle, "right").expect("focus candidate query");

        let focus_tree = handle.model.focus_tree.as_ref().expect("scene-derived focus tree");
        assert!(focus_tree.scope_path(&WindowId::from("master")).is_some());
        assert!(focus_tree.scope_path(&WindowId::from("stack-a")).is_some());
        assert!(focus_tree.scope_path(&WindowId::from("stack-b")).is_some());
    }
}
