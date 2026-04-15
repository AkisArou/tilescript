use crate::abi::{
    HypreactAction, HypreactActionKind, HypreactCommandInput, HypreactCommandKind,
    HypreactDirection, HypreactLayoutCycleDirection,
};
use crate::response::FfiError;
use hypreact_core::command::{FocusDirection, LayoutCycleDirection, WmCommand};
use hypreact_core::host::HostAction;
use hypreact_core::{WindowId, WorkspaceId};

pub fn wm_command_from_ffi(input: &HypreactCommandInput) -> Result<WmCommand, FfiError> {
    Ok(match input.kind {
        HypreactCommandKind::Spawn => {
            WmCommand::Spawn { command: required_string(input.string_value, "command")? }
        }
        HypreactCommandKind::ReloadConfig => WmCommand::ReloadConfig,
        HypreactCommandKind::SetLayout => {
            WmCommand::SetLayout { name: required_string(input.string_value, "name")? }
        }
        HypreactCommandKind::CycleLayout => WmCommand::CycleLayout {
            direction: input
                .has_cycle_direction
                .then(|| cycle_direction_from_ffi(input.cycle_direction)),
        },
        HypreactCommandKind::ViewWorkspace => {
            WmCommand::ViewWorkspace { workspace: input.workspace }
        }
        HypreactCommandKind::ActivateWorkspace => WmCommand::ActivateWorkspace {
            workspace_id: WorkspaceId(required_string(input.string_value, "workspace_id")?),
        },
        HypreactCommandKind::ToggleFloating => WmCommand::ToggleFloating,
        HypreactCommandKind::ToggleFullscreen => WmCommand::ToggleFullscreen,
        HypreactCommandKind::AssignFocusedWindowToWorkspace => {
            WmCommand::AssignFocusedWindowToWorkspace { workspace: input.workspace }
        }
        HypreactCommandKind::ToggleAssignFocusedWindowToWorkspace => {
            WmCommand::ToggleAssignFocusedWindowToWorkspace { workspace: input.workspace }
        }
        HypreactCommandKind::FocusWindow => WmCommand::FocusWindow {
            window_id: WindowId(required_string(input.string_value, "window_id")?),
        },
        HypreactCommandKind::FocusDirection => {
            WmCommand::FocusDirection { direction: direction_from_ffi(input.direction) }
        }
        HypreactCommandKind::SwapDirection => {
            WmCommand::SwapDirection { direction: direction_from_ffi(input.direction) }
        }
        HypreactCommandKind::ResizeDirection => {
            WmCommand::ResizeDirection { direction: direction_from_ffi(input.direction) }
        }
        HypreactCommandKind::MoveDirection => {
            WmCommand::MoveDirection { direction: direction_from_ffi(input.direction) }
        }
        HypreactCommandKind::FocusNextWindow => WmCommand::FocusNextWindow,
        HypreactCommandKind::FocusPreviousWindow => WmCommand::FocusPreviousWindow,
        HypreactCommandKind::SelectNextWorkspace => WmCommand::SelectNextWorkspace,
        HypreactCommandKind::SelectPreviousWorkspace => WmCommand::SelectPreviousWorkspace,
        HypreactCommandKind::SelectWorkspace => WmCommand::SelectWorkspace {
            workspace_id: WorkspaceId(required_string(input.string_value, "workspace_id")?),
        },
        HypreactCommandKind::CloseFocusedWindow => WmCommand::CloseFocusedWindow,
    })
}

pub fn action_to_ffi(action: HostAction) -> Result<HypreactAction, FfiError> {
    let default_direction = HypreactDirection::Left;
    let default_cycle_direction = HypreactLayoutCycleDirection::Next;

    Ok(match action {
        HostAction::SpawnCommand { command } => HypreactAction {
            kind: HypreactActionKind::SpawnCommand,
            string_value: c_string(command)?,
            workspace: 0,
            direction: default_direction,
            cycle_direction: default_cycle_direction,
            has_cycle_direction: false,
        },
        HostAction::ReloadConfig => simple_action(HypreactActionKind::ReloadConfig),
        HostAction::SetLayout { name } => HypreactAction {
            kind: HypreactActionKind::SetLayout,
            string_value: c_string(name)?,
            workspace: 0,
            direction: default_direction,
            cycle_direction: default_cycle_direction,
            has_cycle_direction: false,
        },
        HostAction::CycleLayout { direction } => HypreactAction {
            kind: HypreactActionKind::CycleLayout,
            string_value: std::ptr::null_mut(),
            workspace: 0,
            direction: default_direction,
            cycle_direction: direction
                .map(cycle_direction_to_ffi)
                .unwrap_or(default_cycle_direction),
            has_cycle_direction: direction.is_some(),
        },
        HostAction::ActivateWorkspace { workspace_id } => HypreactAction {
            kind: HypreactActionKind::ActivateWorkspace,
            string_value: c_string(workspace_id)?,
            workspace: 0,
            direction: default_direction,
            cycle_direction: default_cycle_direction,
            has_cycle_direction: false,
        },
        HostAction::AssignFocusedWindowToWorkspace { workspace } => HypreactAction {
            kind: HypreactActionKind::AssignFocusedWindowToWorkspace,
            string_value: std::ptr::null_mut(),
            workspace,
            direction: default_direction,
            cycle_direction: default_cycle_direction,
            has_cycle_direction: false,
        },
        HostAction::ToggleAssignFocusedWindowToWorkspace { workspace } => HypreactAction {
            kind: HypreactActionKind::ToggleAssignFocusedWindowToWorkspace,
            string_value: std::ptr::null_mut(),
            workspace,
            direction: default_direction,
            cycle_direction: default_cycle_direction,
            has_cycle_direction: false,
        },
        HostAction::ToggleFloating => simple_action(HypreactActionKind::ToggleFloating),
        HostAction::ToggleFullscreen => simple_action(HypreactActionKind::ToggleFullscreen),
        HostAction::FocusWindow { window_id } => HypreactAction {
            kind: HypreactActionKind::FocusWindow,
            string_value: c_string(window_id)?,
            workspace: 0,
            direction: default_direction,
            cycle_direction: default_cycle_direction,
            has_cycle_direction: false,
        },
        HostAction::FocusDirection { direction } => {
            direction_action(HypreactActionKind::FocusDirection, direction)
        }
        HostAction::FocusNextWindow => simple_action(HypreactActionKind::FocusNextWindow),
        HostAction::FocusPreviousWindow => simple_action(HypreactActionKind::FocusPreviousWindow),
        HostAction::SwapDirection { direction } => {
            direction_action(HypreactActionKind::SwapDirection, direction)
        }
        HostAction::MoveDirection { direction } => {
            direction_action(HypreactActionKind::MoveDirection, direction)
        }
        HostAction::ResizeDirection { direction } => {
            direction_action(HypreactActionKind::ResizeDirection, direction)
        }
        HostAction::CloseFocusedWindow => simple_action(HypreactActionKind::CloseFocusedWindow),
    })
}

fn required_string(value: *const std::ffi::c_char, field: &str) -> Result<String, FfiError> {
    if value.is_null() {
        return Err(FfiError::InvalidInput(format!("missing {field}")));
    }

    let value = unsafe { std::ffi::CStr::from_ptr(value) }
        .to_str()
        .map_err(|error| FfiError::InvalidUtf8(error.to_string()))?;
    Ok(value.to_string())
}

fn c_string(value: String) -> Result<*mut std::ffi::c_char, FfiError> {
    std::ffi::CString::new(value)
        .map(|value| value.into_raw())
        .map_err(|error| FfiError::NulByte(error.to_string()))
}

fn direction_from_ffi(direction: HypreactDirection) -> FocusDirection {
    match direction {
        HypreactDirection::Left => FocusDirection::Left,
        HypreactDirection::Right => FocusDirection::Right,
        HypreactDirection::Up => FocusDirection::Up,
        HypreactDirection::Down => FocusDirection::Down,
    }
}

fn direction_to_ffi(direction: FocusDirection) -> HypreactDirection {
    match direction {
        FocusDirection::Left => HypreactDirection::Left,
        FocusDirection::Right => HypreactDirection::Right,
        FocusDirection::Up => HypreactDirection::Up,
        FocusDirection::Down => HypreactDirection::Down,
    }
}

fn cycle_direction_from_ffi(direction: HypreactLayoutCycleDirection) -> LayoutCycleDirection {
    match direction {
        HypreactLayoutCycleDirection::Next => LayoutCycleDirection::Next,
        HypreactLayoutCycleDirection::Previous => LayoutCycleDirection::Previous,
    }
}

fn cycle_direction_to_ffi(direction: LayoutCycleDirection) -> HypreactLayoutCycleDirection {
    match direction {
        LayoutCycleDirection::Next => HypreactLayoutCycleDirection::Next,
        LayoutCycleDirection::Previous => HypreactLayoutCycleDirection::Previous,
    }
}

fn simple_action(kind: HypreactActionKind) -> HypreactAction {
    HypreactAction {
        kind,
        string_value: std::ptr::null_mut(),
        workspace: 0,
        direction: HypreactDirection::Left,
        cycle_direction: HypreactLayoutCycleDirection::Next,
        has_cycle_direction: false,
    }
}

fn direction_action(kind: HypreactActionKind, direction: FocusDirection) -> HypreactAction {
    HypreactAction {
        kind,
        string_value: std::ptr::null_mut(),
        workspace: 0,
        direction: direction_to_ffi(direction),
        cycle_direction: HypreactLayoutCycleDirection::Next,
        has_cycle_direction: false,
    }
}
