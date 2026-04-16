use crate::abi::{
    TilescriptAction, TilescriptActionKind, TilescriptCommandInput, TilescriptCommandKind,
    TilescriptDirection, TilescriptLayoutCycleDirection,
};
use crate::response::FfiError;
use tilescript_core::command::{FocusDirection, LayoutCycleDirection, WmCommand};
use tilescript_core::host::HostAction;
use tilescript_core::{WindowId, WorkspaceId};

pub fn wm_command_from_ffi(input: &TilescriptCommandInput) -> Result<WmCommand, FfiError> {
    Ok(match input.kind {
        TilescriptCommandKind::Spawn => {
            WmCommand::Spawn { command: required_string(input.string_value, "command")? }
        }
        TilescriptCommandKind::ReloadConfig => WmCommand::ReloadConfig,
        TilescriptCommandKind::SetLayout => {
            WmCommand::SetLayout { name: required_string(input.string_value, "name")? }
        }
        TilescriptCommandKind::CycleLayout => WmCommand::CycleLayout {
            direction: input
                .has_cycle_direction
                .then(|| cycle_direction_from_ffi(input.cycle_direction)),
        },
        TilescriptCommandKind::ViewWorkspace => {
            WmCommand::ViewWorkspace { workspace: input.workspace }
        }
        TilescriptCommandKind::ActivateWorkspace => WmCommand::ActivateWorkspace {
            workspace_id: WorkspaceId(required_string(input.string_value, "workspace_id")?),
        },
        TilescriptCommandKind::ToggleFloating => WmCommand::ToggleFloating,
        TilescriptCommandKind::ToggleFullscreen => WmCommand::ToggleFullscreen,
        TilescriptCommandKind::AssignFocusedWindowToWorkspace => {
            WmCommand::AssignFocusedWindowToWorkspace { workspace: input.workspace }
        }
        TilescriptCommandKind::ToggleAssignFocusedWindowToWorkspace => {
            WmCommand::ToggleAssignFocusedWindowToWorkspace { workspace: input.workspace }
        }
        TilescriptCommandKind::FocusWindow => WmCommand::FocusWindow {
            window_id: WindowId(required_string(input.string_value, "window_id")?),
        },
        TilescriptCommandKind::FocusDirection => {
            WmCommand::FocusDirection { direction: direction_from_ffi(input.direction) }
        }
        TilescriptCommandKind::SwapDirection => {
            WmCommand::SwapDirection { direction: direction_from_ffi(input.direction) }
        }
        TilescriptCommandKind::ResizeDirection => {
            WmCommand::ResizeDirection { direction: direction_from_ffi(input.direction) }
        }
        TilescriptCommandKind::MoveDirection => {
            WmCommand::MoveDirection { direction: direction_from_ffi(input.direction) }
        }
        TilescriptCommandKind::FocusNextWindow => WmCommand::FocusNextWindow,
        TilescriptCommandKind::FocusPreviousWindow => WmCommand::FocusPreviousWindow,
        TilescriptCommandKind::SelectNextWorkspace => WmCommand::SelectNextWorkspace,
        TilescriptCommandKind::SelectPreviousWorkspace => WmCommand::SelectPreviousWorkspace,
        TilescriptCommandKind::SelectWorkspace => WmCommand::SelectWorkspace {
            workspace_id: WorkspaceId(required_string(input.string_value, "workspace_id")?),
        },
        TilescriptCommandKind::CloseFocusedWindow => WmCommand::CloseFocusedWindow,
    })
}

pub fn action_to_ffi(action: HostAction) -> Result<TilescriptAction, FfiError> {
    let default_direction = TilescriptDirection::Left;
    let default_cycle_direction = TilescriptLayoutCycleDirection::Next;

    Ok(match action {
        HostAction::SpawnCommand { command } => TilescriptAction {
            kind: TilescriptActionKind::SpawnCommand,
            string_value: c_string(command)?,
            workspace: 0,
            direction: default_direction,
            cycle_direction: default_cycle_direction,
            has_cycle_direction: false,
        },
        HostAction::ReloadConfig => simple_action(TilescriptActionKind::ReloadConfig),
        HostAction::SetLayout { name } => TilescriptAction {
            kind: TilescriptActionKind::SetLayout,
            string_value: c_string(name)?,
            workspace: 0,
            direction: default_direction,
            cycle_direction: default_cycle_direction,
            has_cycle_direction: false,
        },
        HostAction::CycleLayout { direction } => TilescriptAction {
            kind: TilescriptActionKind::CycleLayout,
            string_value: std::ptr::null_mut(),
            workspace: 0,
            direction: default_direction,
            cycle_direction: direction
                .map(cycle_direction_to_ffi)
                .unwrap_or(default_cycle_direction),
            has_cycle_direction: direction.is_some(),
        },
        HostAction::ActivateWorkspace { workspace_id } => TilescriptAction {
            kind: TilescriptActionKind::ActivateWorkspace,
            string_value: c_string(workspace_id)?,
            workspace: 0,
            direction: default_direction,
            cycle_direction: default_cycle_direction,
            has_cycle_direction: false,
        },
        HostAction::AssignFocusedWindowToWorkspace { workspace } => TilescriptAction {
            kind: TilescriptActionKind::AssignFocusedWindowToWorkspace,
            string_value: std::ptr::null_mut(),
            workspace,
            direction: default_direction,
            cycle_direction: default_cycle_direction,
            has_cycle_direction: false,
        },
        HostAction::ToggleAssignFocusedWindowToWorkspace { workspace } => TilescriptAction {
            kind: TilescriptActionKind::ToggleAssignFocusedWindowToWorkspace,
            string_value: std::ptr::null_mut(),
            workspace,
            direction: default_direction,
            cycle_direction: default_cycle_direction,
            has_cycle_direction: false,
        },
        HostAction::ToggleFloating => simple_action(TilescriptActionKind::ToggleFloating),
        HostAction::ToggleFullscreen => simple_action(TilescriptActionKind::ToggleFullscreen),
        HostAction::FocusWindow { window_id } => TilescriptAction {
            kind: TilescriptActionKind::FocusWindow,
            string_value: c_string(window_id)?,
            workspace: 0,
            direction: default_direction,
            cycle_direction: default_cycle_direction,
            has_cycle_direction: false,
        },
        HostAction::FocusDirection { direction } => {
            direction_action(TilescriptActionKind::FocusDirection, direction)
        }
        HostAction::FocusNextWindow => simple_action(TilescriptActionKind::FocusNextWindow),
        HostAction::FocusPreviousWindow => simple_action(TilescriptActionKind::FocusPreviousWindow),
        HostAction::SwapDirection { direction } => {
            direction_action(TilescriptActionKind::SwapDirection, direction)
        }
        HostAction::MoveDirection { direction } => {
            direction_action(TilescriptActionKind::MoveDirection, direction)
        }
        HostAction::ResizeDirection { direction } => {
            direction_action(TilescriptActionKind::ResizeDirection, direction)
        }
        HostAction::CloseFocusedWindow => simple_action(TilescriptActionKind::CloseFocusedWindow),
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

fn direction_from_ffi(direction: TilescriptDirection) -> FocusDirection {
    match direction {
        TilescriptDirection::Left => FocusDirection::Left,
        TilescriptDirection::Right => FocusDirection::Right,
        TilescriptDirection::Up => FocusDirection::Up,
        TilescriptDirection::Down => FocusDirection::Down,
    }
}

fn direction_to_ffi(direction: FocusDirection) -> TilescriptDirection {
    match direction {
        FocusDirection::Left => TilescriptDirection::Left,
        FocusDirection::Right => TilescriptDirection::Right,
        FocusDirection::Up => TilescriptDirection::Up,
        FocusDirection::Down => TilescriptDirection::Down,
    }
}

fn cycle_direction_from_ffi(direction: TilescriptLayoutCycleDirection) -> LayoutCycleDirection {
    match direction {
        TilescriptLayoutCycleDirection::Next => LayoutCycleDirection::Next,
        TilescriptLayoutCycleDirection::Previous => LayoutCycleDirection::Previous,
    }
}

fn cycle_direction_to_ffi(direction: LayoutCycleDirection) -> TilescriptLayoutCycleDirection {
    match direction {
        LayoutCycleDirection::Next => TilescriptLayoutCycleDirection::Next,
        LayoutCycleDirection::Previous => TilescriptLayoutCycleDirection::Previous,
    }
}

fn simple_action(kind: TilescriptActionKind) -> TilescriptAction {
    TilescriptAction {
        kind,
        string_value: std::ptr::null_mut(),
        workspace: 0,
        direction: TilescriptDirection::Left,
        cycle_direction: TilescriptLayoutCycleDirection::Next,
        has_cycle_direction: false,
    }
}

fn direction_action(kind: TilescriptActionKind, direction: FocusDirection) -> TilescriptAction {
    TilescriptAction {
        kind,
        string_value: std::ptr::null_mut(),
        workspace: 0,
        direction: direction_to_ffi(direction),
        cycle_direction: TilescriptLayoutCycleDirection::Next,
        has_cycle_direction: false,
    }
}
