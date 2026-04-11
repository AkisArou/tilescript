mod action;
mod ffi_string;
mod layout;
mod response;
mod types;

use std::panic::{AssertUnwindSafe, catch_unwind};

use hypreact_core::OutputId;
use hypreact_core::WorkspaceId;
use hypreact_core::command::WmCommand;
use hypreact_core::query::state_snapshot_for_model;

use action::{CommandResult, dispatch_wm_command};
use ffi_string::{cstr_to_str, into_ffi_string, optional_cstr_to_string, string_free};
use layout::{layout_focus_candidate, layout_runtime_resize_master, layout_runtime_status, load_layout_config, reload_layout_config};
use response::{FfiError, response_ok};
pub use types::{HypreactOutputSync, HypreactRuntimeHandle, HypreactWindowSync};
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
pub extern "C" fn hypreact_runtime_handle_command(
    handle: *mut HypreactRuntimeHandle,
    command_json: *const std::ffi::c_char,
) -> *mut std::ffi::c_char {
    into_ffi_string(catch_unwind(AssertUnwindSafe(|| {
        let _ = ffi_handle_mut(handle)?;
        let command = parse_json::<WmCommand>(command_json)?;
        let actions = dispatch_wm_command(command);
        response_ok(CommandResult { actions })
    })))
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
pub extern "C" fn hypreact_runtime_load_layout_config(
    handle: *mut HypreactRuntimeHandle,
    config_path: *const std::ffi::c_char,
) -> *mut std::ffi::c_char {
    into_ffi_string(catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        let config_path = cstr_to_str(config_path)?.to_string();
        response_ok(load_layout_config(handle, config_path)?)
    })))
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_reload_layout_config(
    handle: *mut HypreactRuntimeHandle,
) -> *mut std::ffi::c_char {
    into_ffi_string(catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        response_ok(reload_layout_config(handle)?)
    })))
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_layout_status(
    handle: *mut HypreactRuntimeHandle,
) -> *mut std::ffi::c_char {
    into_ffi_string(catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        response_ok(layout_runtime_status(handle))
    })))
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_layout_placement(
    handle: *mut HypreactRuntimeHandle,
) -> *mut std::ffi::c_char {
    into_ffi_string(catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        response_ok(layout_runtime_status(handle))
    })))
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_layout_focus_candidate(
    handle: *mut HypreactRuntimeHandle,
    direction: *const std::ffi::c_char,
) -> *mut std::ffi::c_char {
    into_ffi_string(catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        response_ok(layout_focus_candidate(handle, cstr_to_str(direction)?)?)
    })))
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_layout_swap_candidate(
    handle: *mut HypreactRuntimeHandle,
    direction: *const std::ffi::c_char,
) -> *mut std::ffi::c_char {
    into_ffi_string(catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        response_ok(layout_focus_candidate(handle, cstr_to_str(direction)?)?)
    })))
}

#[unsafe(no_mangle)]
pub extern "C" fn hypreact_runtime_layout_resize_master(
    handle: *mut HypreactRuntimeHandle,
    delta: f64,
) -> *mut std::ffi::c_char {
    into_ffi_string(catch_unwind(AssertUnwindSafe(|| {
        let handle = ffi_handle_mut(handle)?;
        response_ok(layout_runtime_resize_master(handle, delta)?)
    })))
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
pub unsafe extern "C" fn hypreact_string_free(value: *mut std::ffi::c_char) {
    unsafe {
        string_free(value);
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

fn parse_json<T: serde::de::DeserializeOwned>(
    value: *const std::ffi::c_char,
) -> Result<T, FfiError> {
    let json = cstr_to_str(value)?;
    serde_json::from_str(json).map_err(|error| FfiError::InvalidJson(error.to_string()))
}

fn optional_cstr_to_window_id(
    value: *const std::ffi::c_char,
) -> Result<Option<hypreact_core::WindowId>, FfiError> {
    optional_cstr_to_string(value).map(|value| value.map(hypreact_core::WindowId::from))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::HostAction;

    #[test]
    fn dispatch_command_returns_hypreact_actions() {
        let actions = dispatch_wm_command(WmCommand::ToggleFullscreen);
        assert!(matches!(actions.as_slice(), [HostAction::ToggleFullscreen]));
    }
}
