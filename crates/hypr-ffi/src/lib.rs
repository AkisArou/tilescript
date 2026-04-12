mod action;
mod ffi_string;
mod layout;
mod response;
mod types;

use std::panic::{AssertUnwindSafe, catch_unwind};

use hypreact_core::wm::LayoutSpaceBox;
use hypreact_core::OutputId;
use hypreact_core::WorkspaceId;
use hypreact_core::query::state_snapshot_for_model;

use action::{action_to_ffi, dispatch_wm_command, wm_command_from_ffi};
use ffi_string::{cstr_to_str, into_ffi_string, optional_cstr_to_string, string_free};
use layout::{layout_focus_candidate, layout_runtime_placement, layout_runtime_placement_for_workspace, layout_runtime_status, load_layout_config, reload_layout_config};
use response::{FfiError, response_ok};
pub use types::{HypreactAction, HypreactActionResult, HypreactCommandInput, HypreactLayoutStatusResult, HypreactOutputSync, HypreactPlacementGeometry, HypreactPlacementResult, HypreactRuntimeHandle, HypreactStateResult, HypreactStatusResult, HypreactStringResult, HypreactWindowSync, HypreactWorkspaceLayoutSpaceSync};
use types::StatusResult;

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_new() -> *mut HypreactRuntimeHandle {
    Box::into_raw(Box::new(HypreactRuntimeHandle {
        model: hypreact_core::wm::WmModel::default(),
        layout_runtime: None,
    }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hypreact_runtime_free(handle: *mut HypreactRuntimeHandle) {
    if handle.is_null() {
        return;
    }

    unsafe {
        drop(Box::from_raw(handle));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hypreact_runtime_dispatch_command(
    handle: *mut HypreactRuntimeHandle,
    command: *const HypreactCommandInput,
) -> HypreactActionResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let _ = ffi_handle_mut(handle)?;
        if command.is_null() {
            return Err(FfiError::NullPointer);
        }

        let command = unsafe { &*command };
        let command = wm_command_from_ffi(command)?;
        action_result(dispatch_wm_command(command))
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => error_action_result(error),
        Err(_) => error_action_result(FfiError::Panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_reset_state(
    handle: *mut HypreactRuntimeHandle,
) -> *mut std::ffi::c_char {
    into_ffi_string(catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        handle.model = hypreact_core::wm::WmModel::default();
        response_ok(StatusResult { changed: true })
    })))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hypreact_runtime_upsert_output(
    handle: *mut HypreactRuntimeHandle,
    output: *const HypreactOutputSync,
) -> *mut std::ffi::c_char {
    into_ffi_string(catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        if output.is_null() {
            return Err(FfiError::NullPointer);
        }

        let output = unsafe { &*output };
        let output_id = OutputId::from(cstr_to_str(output.output_id)?.to_string());
        let name = cstr_to_str(output.name)?.to_string();
        let current_workspace_id = handle
            .model
            .outputs
            .get(&output_id)
            .and_then(|existing| existing.focused_workspace_id.clone());
        handle.model.upsert_output(
            output_id,
            name,
            output.logical_width,
            output.logical_height,
            current_workspace_id,
        );

        response_ok(StatusResult { changed: true })
    })))
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_remove_output(
    handle: *mut HypreactRuntimeHandle,
    output_id: *const std::ffi::c_char,
) -> *mut std::ffi::c_char {
    into_ffi_string(catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        let output_id = OutputId::from(cstr_to_str(output_id)?.to_string());
        let changed = handle.model.outputs.contains_key(&output_id);
        handle.model.remove_output(&output_id);
        response_ok(StatusResult { changed })
    })))
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_activate_workspace(
    handle: *mut HypreactRuntimeHandle,
    workspace_id: *const std::ffi::c_char,
    output_id: *const std::ffi::c_char,
) -> *mut std::ffi::c_char {
    into_ffi_string(catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        let workspace_id = WorkspaceId::from(cstr_to_str(workspace_id)?.to_string());
        let workspace_name = workspace_id.as_str().to_string();
        handle.model.upsert_workspace(workspace_id.clone(), workspace_name);
        handle.model.set_current_workspace(workspace_id.clone());

        if !output_id.is_null() {
            let output_id = OutputId::from(cstr_to_str(output_id)?.to_string());
            handle.model.set_current_output(output_id.clone());
            handle
                .model
                .attach_workspace_to_output(workspace_id.clone(), output_id.clone());
            if let Some(output) = handle.model.outputs.get_mut(&output_id) {
                output.focused_workspace_id = Some(workspace_id.clone());
            }
        }

        response_ok(StatusResult { changed: true })
    })))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hypreact_runtime_set_workspace_layout_space(
    handle: *mut HypreactRuntimeHandle,
    layout_space: *const HypreactWorkspaceLayoutSpaceSync,
) -> *mut std::ffi::c_char {
    into_ffi_string(catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        if layout_space.is_null() {
            return Err(FfiError::NullPointer);
        }

        let layout_space = unsafe { &*layout_space };
        let workspace_id = WorkspaceId::from(cstr_to_str(layout_space.workspace_id)?.to_string());
        let output_id = optional_cstr_to_string(layout_space.output_id)?.map(OutputId::from);

        handle
            .model
            .upsert_workspace(workspace_id.clone(), workspace_id.as_str().to_string());
        if let Some(output_id) = output_id.clone() {
            handle.model.attach_workspace_to_output(workspace_id.clone(), output_id);
        }
        handle.model.set_workspace_layout_space(
            workspace_id,
            Some(LayoutSpaceBox {
                x: layout_space.x,
                y: layout_space.y,
                width: layout_space.width as i32,
                height: layout_space.height as i32,
            }),
        );

        response_ok(StatusResult { changed: true })
    })))
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_focus_window(
    handle: *mut HypreactRuntimeHandle,
    window_id: *const std::ffi::c_char,
) -> *mut std::ffi::c_char {
    into_ffi_string(catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        let window_id = optional_cstr_to_window_id(window_id)?;
        handle.model.set_window_focused(window_id);
        response_ok(StatusResult { changed: true })
    })))
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_remove_window(
    handle: *mut HypreactRuntimeHandle,
    window_id: *const std::ffi::c_char,
) -> *mut std::ffi::c_char {
    into_ffi_string(catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        let window_id = hypreact_core::WindowId::from(cstr_to_str(window_id)?.to_string());
        let changed = handle.model.windows.contains_key(&window_id);
        handle.model.remove_window(window_id);
        response_ok(StatusResult { changed })
    })))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hypreact_runtime_upsert_window(
    handle: *mut HypreactRuntimeHandle,
    window: *const HypreactWindowSync,
) -> *mut std::ffi::c_char {
    into_ffi_string(catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        if window.is_null() {
            return Err(FfiError::NullPointer);
        }

        let window = unsafe { &*window };
        let window_id = hypreact_core::WindowId::from(cstr_to_str(window.window_id)?.to_string());
        let workspace_id = optional_cstr_to_string(window.workspace_id)?.map(WorkspaceId::from);
        let output_id = optional_cstr_to_string(window.output_id)?.map(OutputId::from);

        if !window.mapped {
            let changed = handle.model.windows.contains_key(&window_id);
            if changed {
                handle.model.remove_window(window_id);
            }
            return response_ok(StatusResult { changed });
        }

        if let Some(workspace_id) = workspace_id.as_ref() {
            handle
                .model
                .upsert_workspace(workspace_id.clone(), workspace_id.as_str().to_string());
        }

        let existed = handle.model.windows.contains_key(&window_id);
        if !existed {
            handle
                .model
                .insert_window(window_id.clone(), workspace_id.clone(), output_id.clone());
        }

        if let Some(window_model) = handle.model.windows.get_mut(&window_id) {
            window_model.is_xwayland = window.is_xwayland;
            window_model.workspace_id = workspace_id;
            window_model.output_id = output_id;
            window_model.mapped = window.mapped;
            window_model.title = optional_cstr_to_string(window.title)?;
            window_model.app_id = optional_cstr_to_string(window.app_id)?;
            window_model.class = optional_cstr_to_string(window.class_name)?;
            window_model.instance = optional_cstr_to_string(window.instance)?;
            window_model.role = optional_cstr_to_string(window.role)?;
            window_model.window_type = optional_cstr_to_string(window.window_type)?;
            window_model.urgent = window.urgent;
            window_model.floating = window.floating;
            window_model.fullscreen = window.fullscreen;
        }

        response_ok(StatusResult { changed: true })
    })))
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_load_layout_config_result(
    handle: *mut HypreactRuntimeHandle,
    config_path: *const std::ffi::c_char,
) -> HypreactStatusResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        let config_path = cstr_to_str(config_path)?.to_string();
        let _ = load_layout_config(handle, config_path)?;
        status_result(StatusResult { changed: true })
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => error_status_result(error),
        Err(_) => error_status_result(FfiError::Panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_reload_layout_config_result(
    handle: *mut HypreactRuntimeHandle,
) -> HypreactStatusResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        let _ = reload_layout_config(handle)?;
        status_result(StatusResult { changed: true })
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => error_status_result(error),
        Err(_) => error_status_result(FfiError::Panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_layout_placement(
    handle: *mut HypreactRuntimeHandle,
) -> HypreactPlacementResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        placement_result(layout_runtime_placement(handle))
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(_)) | Err(_) => HypreactPlacementResult {
            geometries: std::ptr::null_mut(),
            geometry_count: 0,
        },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_layout_placement_for_workspace(
    handle: *mut HypreactRuntimeHandle,
    workspace_id: *const std::ffi::c_char,
) -> HypreactPlacementResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        let workspace_id = cstr_to_str(workspace_id)?;
        placement_result(layout_runtime_placement_for_workspace(handle, workspace_id))
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(_)) | Err(_) => HypreactPlacementResult {
            geometries: std::ptr::null_mut(),
            geometry_count: 0,
        },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_layout_focus_candidate(
    handle: *mut HypreactRuntimeHandle,
    direction: *const std::ffi::c_char,
) -> HypreactStringResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        string_result(layout_focus_candidate(handle, cstr_to_str(direction)?)?)
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(_)) | Err(_) => HypreactStringResult {
            value: std::ptr::null_mut(),
        },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_layout_swap_candidate(
    handle: *mut HypreactRuntimeHandle,
    direction: *const std::ffi::c_char,
) -> HypreactStringResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        string_result(layout_focus_candidate(handle, cstr_to_str(direction)?)?)
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(_)) | Err(_) => HypreactStringResult {
            value: std::ptr::null_mut(),
        },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_move_tiled_window(
    handle: *mut HypreactRuntimeHandle,
    first_window_id: *const std::ffi::c_char,
    second_window_id: *const std::ffi::c_char,
) -> HypreactStatusResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        let first_window_id = hypreact_core::WindowId::from(cstr_to_str(first_window_id)?.to_string());
        let second_window_id = hypreact_core::WindowId::from(cstr_to_str(second_window_id)?.to_string());
        let changed = handle.model.move_tiled_window(&first_window_id, &second_window_id);
        status_result(StatusResult { changed })
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => error_status_result(error),
        Err(_) => error_status_result(FfiError::Panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_state(
    handle: *mut HypreactRuntimeHandle,
) -> *mut std::ffi::c_char {
    into_ffi_string(catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        response_ok(state_snapshot_for_model(&handle.model))
    })))
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_state_result(
    handle: *mut HypreactRuntimeHandle,
) -> HypreactStateResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        state_result(state_snapshot_for_model(&handle.model))
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(_)) | Err(_) => empty_state_result(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_layout_status_result(
    handle: *mut HypreactRuntimeHandle,
) -> HypreactLayoutStatusResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        layout_status_result(layout_runtime_status(handle))
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(_)) | Err(_) => empty_layout_status_result(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hypreact_string_free(value: *mut std::ffi::c_char) {
    unsafe {
        string_free(value);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hypreact_runtime_free_placement_result(result: HypreactPlacementResult) {
    if result.geometries.is_null() {
        return;
    }

    let geometries = unsafe {
        Vec::from_raw_parts(result.geometries, result.geometry_count, result.geometry_count)
    };

    for geometry in geometries {
        unsafe {
            string_free(geometry.window_id.cast_mut());
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hypreact_runtime_free_action_result(result: HypreactActionResult) {
    if !result.error.is_null() {
        unsafe {
            string_free(result.error);
        }
    }

    if result.actions.is_null() {
        return;
    }

    let actions = unsafe { Vec::from_raw_parts(result.actions, result.action_count, result.action_count) };
    for action in actions {
        if !action.string_value.is_null() {
            unsafe {
                string_free(action.string_value);
            }
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hypreact_runtime_free_state_result(result: HypreactStateResult) {
    free_string_array(result.workspace_names, result.workspace_name_count);
    if !result.current_workspace_id.is_null() {
        unsafe { string_free(result.current_workspace_id); }
    }
    if !result.current_output_id.is_null() {
        unsafe { string_free(result.current_output_id); }
    }
    if !result.focused_window_id.is_null() {
        unsafe { string_free(result.focused_window_id); }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hypreact_runtime_free_layout_status_result(result: HypreactLayoutStatusResult) {
    if !result.config_path.is_null() {
        unsafe { string_free(result.config_path); }
    }
    if !result.selected_layout_name.is_null() {
        unsafe { string_free(result.selected_layout_name); }
    }
    if !result.error.is_null() {
        unsafe { string_free(result.error); }
    }
    free_string_array(result.workspace_names, result.workspace_name_count);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hypreact_runtime_free_status_result(result: HypreactStatusResult) {
    if !result.error.is_null() {
        unsafe { string_free(result.error); }
    }
}

fn ffi_handle_mut<'a>(
    handle: *mut HypreactRuntimeHandle,
) -> Result<&'a mut HypreactRuntimeHandle, FfiError> {
    if handle.is_null() {
        return Err(FfiError::NullPointer);
    }

    Ok(unsafe { &mut *handle })
}

fn optional_cstr_to_window_id(
    value: *const std::ffi::c_char,
) -> Result<Option<hypreact_core::WindowId>, FfiError> {
    optional_cstr_to_string(value).map(|value| value.map(hypreact_core::WindowId::from))
}

fn placement_result(
    entries: Vec<layout::PlacementGeometry>,
) -> Result<HypreactPlacementResult, FfiError> {
    let geometries = entries
        .into_iter()
        .map(|entry| {
            Ok(HypreactPlacementGeometry {
                window_id: std::ffi::CString::new(entry.window_id)
                    .map_err(|error| FfiError::NulByte(error.to_string()))?
                    .into_raw(),
                x: entry.x,
                y: entry.y,
                width: entry.width,
                height: entry.height,
            })
        })
        .collect::<Result<Vec<_>, FfiError>>()?;

    let geometry_count = geometries.len();
    let mut geometries = geometries;
    let geometries_ptr = geometries.as_mut_ptr();
    std::mem::forget(geometries);

    Ok(HypreactPlacementResult {
        geometries: geometries_ptr,
        geometry_count,
    })
}

fn string_result(value: Option<String>) -> Result<HypreactStringResult, FfiError> {
    let Some(value) = value else {
        return Ok(HypreactStringResult {
            value: std::ptr::null_mut(),
        });
    };

    Ok(HypreactStringResult {
        value: std::ffi::CString::new(value)
            .map_err(|error| FfiError::NulByte(error.to_string()))?
            .into_raw(),
    })
}

fn action_result(actions: Vec<action::HostAction>) -> Result<HypreactActionResult, FfiError> {
    let actions = actions
        .into_iter()
        .map(action_to_ffi)
        .collect::<Result<Vec<HypreactAction>, FfiError>>()?;

    let action_count = actions.len();
    let mut actions = actions;
    let actions_ptr = actions.as_mut_ptr();
    std::mem::forget(actions);

    Ok(HypreactActionResult {
        actions: actions_ptr,
        action_count,
        error: std::ptr::null_mut(),
    })
}

fn error_action_result(error: FfiError) -> HypreactActionResult {
    let error = std::ffi::CString::new(error.to_string())
        .expect("ffi error strings must not contain nul bytes")
        .into_raw();

    HypreactActionResult {
        actions: std::ptr::null_mut(),
        action_count: 0,
        error,
    }
}

fn state_result(snapshot: hypreact_core::snapshot::StateSnapshot) -> Result<HypreactStateResult, FfiError> {
    let workspace_name_count = snapshot.workspace_names.len();
    Ok(HypreactStateResult {
        workspace_names: string_array(snapshot.workspace_names)?,
        workspace_name_count,
        current_workspace_id: optional_owned_string(snapshot.current_workspace_id.map(|id| id.0))?,
        current_output_id: optional_owned_string(snapshot.current_output_id.map(|id| id.0))?,
        focused_window_id: optional_owned_string(snapshot.focused_window_id.map(|id| id.0))?,
    })
}

fn layout_status_result(status: types::LayoutRuntimeStatus) -> Result<HypreactLayoutStatusResult, FfiError> {
    let workspace_names = status.workspace_names.unwrap_or_default();
    Ok(HypreactLayoutStatusResult {
        loaded: status.loaded,
        config_path: optional_owned_string(status.config_path)?,
        selected_layout_name: optional_owned_string(status.selected_layout_name)?,
        error: optional_owned_string(status.error)?,
        workspace_names: string_array(workspace_names.clone())?,
        workspace_name_count: workspace_names.len(),
    })
}

fn string_array(values: Vec<String>) -> Result<*mut *mut std::ffi::c_char, FfiError> {
    let mut raw_values = values
        .into_iter()
        .map(|value| {
            std::ffi::CString::new(value)
                .map(|value| value.into_raw())
                .map_err(|error| FfiError::NulByte(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let ptr = raw_values.as_mut_ptr();
    std::mem::forget(raw_values);
    Ok(ptr)
}

fn optional_owned_string(value: Option<String>) -> Result<*mut std::ffi::c_char, FfiError> {
    match value {
        Some(value) => std::ffi::CString::new(value)
            .map(|value| value.into_raw())
            .map_err(|error| FfiError::NulByte(error.to_string())),
        None => Ok(std::ptr::null_mut()),
    }
}

fn free_string_array(values: *mut *mut std::ffi::c_char, count: usize) {
    if values.is_null() {
        return;
    }

    let values = unsafe { Vec::from_raw_parts(values, count, count) };
    for value in values {
        if !value.is_null() {
            unsafe { string_free(value); }
        }
    }
}

fn empty_state_result() -> HypreactStateResult {
    HypreactStateResult {
        workspace_names: std::ptr::null_mut(),
        workspace_name_count: 0,
        current_workspace_id: std::ptr::null_mut(),
        current_output_id: std::ptr::null_mut(),
        focused_window_id: std::ptr::null_mut(),
    }
}

fn empty_layout_status_result() -> HypreactLayoutStatusResult {
    HypreactLayoutStatusResult {
        loaded: false,
        config_path: std::ptr::null_mut(),
        selected_layout_name: std::ptr::null_mut(),
        error: std::ptr::null_mut(),
        workspace_names: std::ptr::null_mut(),
        workspace_name_count: 0,
    }
}

fn status_result(status: types::StatusResult) -> Result<HypreactStatusResult, FfiError> {
    Ok(HypreactStatusResult {
        changed: status.changed,
        error: std::ptr::null_mut(),
    })
}

fn error_status_result(error: FfiError) -> HypreactStatusResult {
    HypreactStatusResult {
        changed: false,
        error: std::ffi::CString::new(error.to_string())
            .expect("ffi error strings must not contain nul bytes")
            .into_raw(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::HostAction;
    use hypreact_core::command::WmCommand;

    #[test]
    fn dispatch_command_returns_hypreact_actions() {
        let actions = dispatch_wm_command(WmCommand::ToggleFullscreen);
        assert!(matches!(actions.as_slice(), [HostAction::ToggleFullscreen]));
    }
}
