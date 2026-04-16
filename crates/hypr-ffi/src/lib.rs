mod abi;
mod action;
mod bootstrap;
mod ffi_string;
mod layout;
mod response;
mod runtime_types;
mod sdk;

use std::panic::{AssertUnwindSafe, catch_unwind};

use hypreact_core::OutputId;
use hypreact_core::WorkspaceId;
use hypreact_core::host::{HostAction, dispatch_wm_command};
use hypreact_core::query::state_snapshot_for_model;
use hypreact_core::resize::ResizeDirection;
use hypreact_core::wm::DrawableSpace;
use hypreact_layout_runtime as runtime_facade;

pub use abi::{
    HypreactAction, HypreactActionResult, HypreactCommandInput, HypreactDiagnostic,
    HypreactDiagnosticRange, HypreactLayoutStatusResult, HypreactOutputSync,
    HypreactPlacementGeometry, HypreactPlacementResult, HypreactStateResult, HypreactStatusResult,
    HypreactStringResult, HypreactWindowSync, HypreactWorkspaceLayoutSpaceSync,
};
use action::{action_to_ffi, wm_command_from_ffi};
use bootstrap::bootstrap_config_root;
use ffi_string::{cstr_to_str, optional_cstr_to_string, string_free};
use layout::{
    drain_layout_runtime_source_changes, layout_close_focus_candidate, layout_focus_candidate,
    layout_runtime_placement, layout_runtime_placement_for_workspace,
    layout_runtime_source_change_fd, layout_runtime_status, load_layout_config,
    reload_layout_config,
};
use response::FfiError;
use runtime_types::{HypreactRuntimeHandle, LayoutRuntimeStatus, StatusResult};
use sdk::sync_sdk_support;

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
pub extern "C" fn hypreact_runtime_reset_state_result(
    handle: *mut HypreactRuntimeHandle,
) -> HypreactStatusResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        runtime_facade::reset_model(&mut handle.model);
        status_result(StatusResult { changed: true, focused_window_id: None })
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => error_status_result(error),
        Err(_) => error_status_result(FfiError::Panic),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hypreact_runtime_upsert_output_result(
    handle: *mut HypreactRuntimeHandle,
    output: *const HypreactOutputSync,
) -> HypreactStatusResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        if output.is_null() {
            return Err(FfiError::NullPointer);
        }

        let output = unsafe { &*output };
        let output_id = OutputId::from(cstr_to_str(output.output_id)?.to_string());
        let name = cstr_to_str(output.name)?.to_string();
        runtime_facade::upsert_output(
            &mut handle.model,
            output_id,
            name,
            output.logical_width,
            output.logical_height,
        );

        status_result(StatusResult { changed: true, focused_window_id: None })
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => error_status_result(error),
        Err(_) => error_status_result(FfiError::Panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_remove_output_result(
    handle: *mut HypreactRuntimeHandle,
    output_id: *const std::ffi::c_char,
) -> HypreactStatusResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        let output_id = OutputId::from(cstr_to_str(output_id)?.to_string());
        let changed = runtime_facade::remove_output(&mut handle.model, &output_id);
        status_result(StatusResult { changed, focused_window_id: None })
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => error_status_result(error),
        Err(_) => error_status_result(FfiError::Panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_activate_workspace_result(
    handle: *mut HypreactRuntimeHandle,
    workspace_id: *const std::ffi::c_char,
    output_id: *const std::ffi::c_char,
) -> HypreactStatusResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        let workspace_id = WorkspaceId::from(cstr_to_str(workspace_id)?.to_string());
        let output_id = if output_id.is_null() {
            None
        } else {
            Some(OutputId::from(cstr_to_str(output_id)?.to_string()))
        };
        runtime_facade::activate_workspace(&mut handle.model, workspace_id, output_id);

        status_result(StatusResult { changed: true, focused_window_id: None })
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => error_status_result(error),
        Err(_) => error_status_result(FfiError::Panic),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hypreact_runtime_set_workspace_layout_space_result(
    handle: *mut HypreactRuntimeHandle,
    layout_space: *const HypreactWorkspaceLayoutSpaceSync,
) -> HypreactStatusResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        if layout_space.is_null() {
            return Err(FfiError::NullPointer);
        }

        let layout_space = unsafe { &*layout_space };
        let workspace_id = WorkspaceId::from(cstr_to_str(layout_space.workspace_id)?.to_string());
        let output_id = optional_cstr_to_string(layout_space.output_id)?.map(OutputId::from);

        runtime_facade::set_workspace_layout_space(
            &mut handle.model,
            workspace_id,
            output_id,
            DrawableSpace { width: layout_space.width as i32, height: layout_space.height as i32 },
        );

        status_result(StatusResult { changed: true, focused_window_id: None })
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => error_status_result(error),
        Err(_) => error_status_result(FfiError::Panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_focus_window_result(
    handle: *mut HypreactRuntimeHandle,
    window_id: *const std::ffi::c_char,
) -> HypreactStatusResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        let window_id = optional_cstr_to_window_id(window_id)?;
        runtime_facade::focus_window(&mut handle.model, window_id);
        status_result(StatusResult { changed: true, focused_window_id: None })
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => error_status_result(error),
        Err(_) => error_status_result(FfiError::Panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_set_window_closing_result(
    handle: *mut HypreactRuntimeHandle,
    window_id: *const std::ffi::c_char,
    closing: bool,
) -> HypreactStatusResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        let window_id = hypreact_core::WindowId::from(cstr_to_str(window_id)?.to_string());
        let changed = runtime_facade::set_window_closing(&mut handle.model, &window_id, closing);
        status_result(StatusResult { changed, focused_window_id: None })
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => error_status_result(error),
        Err(_) => error_status_result(FfiError::Panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_remove_window_result(
    handle: *mut HypreactRuntimeHandle,
    window_id: *const std::ffi::c_char,
) -> HypreactStatusResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        let window_id = hypreact_core::WindowId::from(cstr_to_str(window_id)?.to_string());
        let (changed, focused_window_id) =
            runtime_facade::remove_window(&mut handle.model, window_id);
        status_result(StatusResult {
            changed,
            focused_window_id: focused_window_id.map(|window_id| window_id.to_string()),
        })
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => error_status_result(error),
        Err(_) => error_status_result(FfiError::Panic),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hypreact_runtime_upsert_window_result(
    handle: *mut HypreactRuntimeHandle,
    window: *const HypreactWindowSync,
) -> HypreactStatusResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        if window.is_null() {
            return Err(FfiError::NullPointer);
        }

        let window = unsafe { &*window };
        let window_id = hypreact_core::WindowId::from(cstr_to_str(window.window_id)?.to_string());
        let workspace_id = optional_cstr_to_string(window.workspace_id)?.map(WorkspaceId::from);
        let output_id = optional_cstr_to_string(window.output_id)?.map(OutputId::from);

        let changed = runtime_facade::upsert_window(
            &mut handle.model,
            window_id,
            workspace_id,
            output_id,
            window.is_xwayland,
            window.mapped,
            optional_cstr_to_string(window.title)?,
            optional_cstr_to_string(window.app_id)?,
            optional_cstr_to_string(window.class_name)?,
            optional_cstr_to_string(window.instance)?,
            optional_cstr_to_string(window.role)?,
            optional_cstr_to_string(window.window_type)?,
            window.urgent,
            window.floating,
            window.fullscreen,
        );

        status_result(StatusResult { changed, focused_window_id: None })
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => error_status_result(error),
        Err(_) => error_status_result(FfiError::Panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_bootstrap_config_result(
    config_root: *const std::ffi::c_char,
) -> HypreactStatusResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let config_root = std::path::PathBuf::from(cstr_to_str(config_root)?);
        let changed = bootstrap_config_root(&config_root)?;
        status_result(StatusResult { changed, focused_window_id: None })
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => error_status_result(error),
        Err(_) => error_status_result(FfiError::Panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_sync_sdk_support_result(
    config_root: *const std::ffi::c_char,
) -> HypreactStatusResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let config_root = std::path::PathBuf::from(cstr_to_str(config_root)?);
        let changed = sync_sdk_support(&config_root)?;
        status_result(StatusResult { changed, focused_window_id: None })
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => error_status_result(error),
        Err(_) => error_status_result(FfiError::Panic),
    }
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
        status_result(StatusResult { changed: true, focused_window_id: None })
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
        status_result(StatusResult { changed: true, focused_window_id: None })
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => error_status_result(error),
        Err(_) => error_status_result(FfiError::Panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_poll_layout_sources_result(
    handle: *mut HypreactRuntimeHandle,
) -> HypreactStatusResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        let changed = drain_layout_runtime_source_changes(handle)?;
        status_result(StatusResult { changed, focused_window_id: None })
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => error_status_result(error),
        Err(_) => error_status_result(FfiError::Panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_layout_source_change_fd(
    handle: *mut HypreactRuntimeHandle,
) -> i32 {
    match catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        layout_runtime_source_change_fd(handle)
    })) {
        Ok(Ok(fd)) => fd,
        Ok(Err(_)) | Err(_) => -1,
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
        Ok(Err(_)) | Err(_) => {
            HypreactPlacementResult { geometries: std::ptr::null_mut(), geometry_count: 0 }
        }
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
        Ok(Err(_)) | Err(_) => {
            HypreactPlacementResult { geometries: std::ptr::null_mut(), geometry_count: 0 }
        }
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
        Ok(Err(_)) | Err(_) => HypreactStringResult { value: std::ptr::null_mut() },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_layout_close_focus_candidate(
    handle: *mut HypreactRuntimeHandle,
    window_id: *const std::ffi::c_char,
) -> HypreactStringResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        string_result(layout_close_focus_candidate(handle, cstr_to_str(window_id)?)?)
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(_)) | Err(_) => HypreactStringResult { value: std::ptr::null_mut() },
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
        Ok(Err(_)) | Err(_) => HypreactStringResult { value: std::ptr::null_mut() },
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
        let first_window_id =
            hypreact_core::WindowId::from(cstr_to_str(first_window_id)?.to_string());
        let second_window_id =
            hypreact_core::WindowId::from(cstr_to_str(second_window_id)?.to_string());
        let changed = runtime_facade::move_tiled_window(
            &mut handle.model,
            &first_window_id,
            &second_window_id,
        );
        status_result(StatusResult { changed, focused_window_id: None })
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => error_status_result(error),
        Err(_) => error_status_result(FfiError::Panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_resize_direction(
    handle: *mut HypreactRuntimeHandle,
    direction: *const std::ffi::c_char,
) -> HypreactStatusResult {
    match catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        let Some(layout_runtime) = handle.layout_runtime.as_mut() else {
            return Err(FfiError::InvalidInput("layout runtime not initialized".to_string()));
        };

        let direction = match cstr_to_str(direction)? {
            "left" => ResizeDirection::Left,
            "right" => ResizeDirection::Right,
            "up" => ResizeDirection::Up,
            "down" => ResizeDirection::Down,
            value => {
                return Err(FfiError::InvalidInput(format!("invalid resize direction: {value}")));
            }
        };

        let changed = runtime_facade::resize_direction(
            &mut layout_runtime.service,
            &mut handle.model,
            direction,
        )
        .map_err(|error| FfiError::InvalidJson(error.to_string()))?;
        status_result(StatusResult { changed, focused_window_id: None })
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => error_status_result(error),
        Err(_) => error_status_result(FfiError::Panic),
    }
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

    let actions =
        unsafe { Vec::from_raw_parts(result.actions, result.action_count, result.action_count) };
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
        unsafe {
            string_free(result.current_workspace_id);
        }
    }
    if !result.current_output_id.is_null() {
        unsafe {
            string_free(result.current_output_id);
        }
    }
    if !result.focused_window_id.is_null() {
        unsafe {
            string_free(result.focused_window_id);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hypreact_runtime_free_layout_status_result(
    result: HypreactLayoutStatusResult,
) {
    if !result.config_path.is_null() {
        unsafe {
            string_free(result.config_path);
        }
    }
    if !result.selected_layout_name.is_null() {
        unsafe {
            string_free(result.selected_layout_name);
        }
    }
    if !result.error.is_null() {
        unsafe {
            string_free(result.error);
        }
    }
    free_diagnostic_array(result.diagnostics, result.diagnostic_count);
    free_string_array(result.workspace_names, result.workspace_name_count);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hypreact_runtime_free_status_result(result: HypreactStatusResult) {
    if !result.focused_window_id.is_null() {
        unsafe {
            string_free(result.focused_window_id);
        }
    }
    if !result.error.is_null() {
        unsafe {
            string_free(result.error);
        }
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

    Ok(HypreactPlacementResult { geometries: geometries_ptr, geometry_count })
}

fn string_result(value: Option<String>) -> Result<HypreactStringResult, FfiError> {
    let Some(value) = value else {
        return Ok(HypreactStringResult { value: std::ptr::null_mut() });
    };

    Ok(HypreactStringResult {
        value: std::ffi::CString::new(value)
            .map_err(|error| FfiError::NulByte(error.to_string()))?
            .into_raw(),
    })
}

fn action_result(actions: Vec<HostAction>) -> Result<HypreactActionResult, FfiError> {
    let actions = actions
        .into_iter()
        .map(action_to_ffi)
        .collect::<Result<Vec<HypreactAction>, FfiError>>()?;

    let action_count = actions.len();
    let mut actions = actions;
    let actions_ptr = actions.as_mut_ptr();
    std::mem::forget(actions);

    Ok(HypreactActionResult { actions: actions_ptr, action_count, error: std::ptr::null_mut() })
}

fn error_action_result(error: FfiError) -> HypreactActionResult {
    let error = std::ffi::CString::new(error.to_string())
        .expect("ffi error strings must not contain nul bytes")
        .into_raw();

    HypreactActionResult { actions: std::ptr::null_mut(), action_count: 0, error }
}

fn state_result(
    snapshot: hypreact_core::snapshot::StateSnapshot,
) -> Result<HypreactStateResult, FfiError> {
    let workspace_name_count = snapshot.workspace_names.len();
    Ok(HypreactStateResult {
        workspace_names: string_array(snapshot.workspace_names)?,
        workspace_name_count,
        current_workspace_id: optional_owned_string(snapshot.current_workspace_id.map(|id| id.0))?,
        current_output_id: optional_owned_string(snapshot.current_output_id.map(|id| id.0))?,
        focused_window_id: optional_owned_string(snapshot.focused_window_id.map(|id| id.0))?,
    })
}

fn layout_status_result(
    status: LayoutRuntimeStatus,
) -> Result<HypreactLayoutStatusResult, FfiError> {
    let workspace_names = status.workspace_names.unwrap_or_default();
    let diagnostic_count = status.diagnostics.len();
    Ok(HypreactLayoutStatusResult {
        loaded: status.loaded,
        config_path: optional_owned_string(status.config_path)?,
        selected_layout_name: optional_owned_string(status.selected_layout_name)?,
        error: optional_owned_string(status.error)?,
        diagnostics: diagnostic_array(status.diagnostics)?,
        diagnostic_count,
        workspace_names: string_array(workspace_names.clone())?,
        workspace_name_count: workspace_names.len(),
    })
}

fn diagnostic_array(
    diagnostics: Vec<hypreact_layout_runtime::LayoutDiagnostic>,
) -> Result<*mut HypreactDiagnostic, FfiError> {
    let mut raw_diagnostics = diagnostics
        .into_iter()
        .map(|diagnostic| {
            Ok(HypreactDiagnostic {
                source: optional_owned_string(Some(diagnostic.source))?,
                severity: optional_owned_string(Some(diagnostic.severity))?,
                code: optional_owned_string(Some(diagnostic.code))?,
                message: optional_owned_string(Some(diagnostic.message))?,
                path: optional_owned_string(diagnostic.path)?,
                range: HypreactDiagnosticRange {
                    start_line: diagnostic.range.start_line,
                    start_column: diagnostic.range.start_column,
                    end_line: diagnostic.range.end_line,
                    end_column: diagnostic.range.end_column,
                },
            })
        })
        .collect::<Result<Vec<_>, FfiError>>()?;

    let ptr = raw_diagnostics.as_mut_ptr();
    std::mem::forget(raw_diagnostics);
    Ok(ptr)
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
            unsafe {
                string_free(value);
            }
        }
    }
}

fn free_diagnostic_array(values: *mut HypreactDiagnostic, count: usize) {
    if values.is_null() {
        return;
    }

    let values = unsafe { Vec::from_raw_parts(values, count, count) };
    for value in values {
        for string in [value.source, value.severity, value.code, value.message, value.path] {
            if !string.is_null() {
                unsafe {
                    string_free(string);
                }
            }
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
        diagnostics: std::ptr::null_mut(),
        diagnostic_count: 0,
        workspace_names: std::ptr::null_mut(),
        workspace_name_count: 0,
    }
}

fn status_result(status: StatusResult) -> Result<HypreactStatusResult, FfiError> {
    Ok(HypreactStatusResult {
        changed: status.changed,
        focused_window_id: optional_owned_string(status.focused_window_id)?,
        error: std::ptr::null_mut(),
    })
}

fn error_status_result(error: FfiError) -> HypreactStatusResult {
    HypreactStatusResult {
        changed: false,
        focused_window_id: std::ptr::null_mut(),
        error: std::ffi::CString::new(error.to_string())
            .expect("ffi error strings must not contain nul bytes")
            .into_raw(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypreact_core::command::WmCommand;

    #[test]
    fn dispatch_command_returns_hypreact_actions() {
        let actions = dispatch_wm_command(WmCommand::ToggleFullscreen);
        assert!(matches!(actions.as_slice(), [HostAction::ToggleFullscreen]));
    }
}
