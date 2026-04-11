use crate::response::FfiError;
use crate::types::{
    HypreactAction, HypreactActionKind, HypreactCommandInput, HypreactCommandKind,
    HypreactDirection, HypreactLayoutCycleDirection,
};
use hypreact_core::command::{FocusDirection, LayoutCycleDirection, WmCommand};
use hypreact_core::{WindowId, WorkspaceId};

#[derive(Debug)]
pub enum HostAction {
    SpawnCommand {
        command: String,
    },
    ReloadConfig,
    SetLayout {
        name: String,
    },
    CycleLayout {
        direction: Option<LayoutCycleDirection>,
    },
    ActivateWorkspace {
        workspace_id: String,
    },
    AssignFocusedWindowToWorkspace {
        workspace: u8,
    },
    ToggleAssignFocusedWindowToWorkspace {
        workspace: u8,
    },
    ToggleFloating,
    ToggleFullscreen,
    FocusWindow {
        window_id: String,
    },
    FocusDirection {
        direction: FocusDirection,
    },
    FocusNextWindow,
    FocusPreviousWindow,
    SwapDirection {
        direction: FocusDirection,
    },
    MoveDirection {
        direction: FocusDirection,
    },
    ResizeDirection {
        direction: FocusDirection,
    },
    CloseFocusedWindow,
}

pub fn dispatch_wm_command(command: WmCommand) -> Vec<HostAction> {
    match command {
        WmCommand::Spawn { command } => vec![HostAction::SpawnCommand { command }],
        WmCommand::ReloadConfig => vec![HostAction::ReloadConfig],
        WmCommand::SetLayout { name } => vec![HostAction::SetLayout { name }],
        WmCommand::CycleLayout { direction } => vec![HostAction::CycleLayout { direction }],
        WmCommand::ActivateWorkspace { workspace_id }
        | WmCommand::SelectWorkspace { workspace_id } => vec![HostAction::ActivateWorkspace {
            workspace_id: workspace_id.0,
        }],
        WmCommand::SelectNextWorkspace => vec![HostAction::ActivateWorkspace {
            workspace_id: "e+1".to_string(),
        }],
        WmCommand::SelectPreviousWorkspace => vec![HostAction::ActivateWorkspace {
            workspace_id: "e-1".to_string(),
        }],
        WmCommand::ViewWorkspace { workspace } => vec![HostAction::ActivateWorkspace {
            workspace_id: workspace.to_string(),
        }],
        WmCommand::AssignFocusedWindowToWorkspace { workspace } => {
            vec![HostAction::AssignFocusedWindowToWorkspace { workspace }]
        }
        WmCommand::ToggleAssignFocusedWindowToWorkspace { workspace } => {
            vec![HostAction::ToggleAssignFocusedWindowToWorkspace { workspace }]
        }
        WmCommand::ToggleFloating => vec![HostAction::ToggleFloating],
        WmCommand::ToggleFullscreen => vec![HostAction::ToggleFullscreen],
        WmCommand::FocusWindow { window_id } => vec![HostAction::FocusWindow {
            window_id: window_id.0,
        }],
        WmCommand::FocusDirection { direction } => vec![HostAction::FocusDirection { direction }],
        WmCommand::FocusNextWindow => vec![HostAction::FocusNextWindow],
        WmCommand::FocusPreviousWindow => vec![HostAction::FocusPreviousWindow],
        WmCommand::SwapDirection { direction } => vec![HostAction::SwapDirection { direction }],
        WmCommand::MoveDirection { direction } => vec![HostAction::MoveDirection { direction }],
        WmCommand::ResizeDirection { direction } => vec![HostAction::ResizeDirection { direction }],
        WmCommand::CloseFocusedWindow => vec![HostAction::CloseFocusedWindow],
    }
}

pub fn wm_command_from_ffi(input: &HypreactCommandInput) -> Result<WmCommand, FfiError> {
    Ok(match input.kind {
        HypreactCommandKind::Spawn => WmCommand::Spawn {
            command: required_string(input.string_value, "command")?,
        },
        HypreactCommandKind::ReloadConfig => WmCommand::ReloadConfig,
        HypreactCommandKind::SetLayout => WmCommand::SetLayout {
            name: required_string(input.string_value, "name")?,
        },
        HypreactCommandKind::CycleLayout => WmCommand::CycleLayout {
            direction: input
                .has_cycle_direction
                .then(|| cycle_direction_from_ffi(input.cycle_direction)),
        },
        HypreactCommandKind::ViewWorkspace => WmCommand::ViewWorkspace {
            workspace: input.workspace,
        },
        HypreactCommandKind::ActivateWorkspace => WmCommand::ActivateWorkspace {
            workspace_id: WorkspaceId(required_string(input.string_value, "workspace_id")?),
        },
        HypreactCommandKind::ToggleFloating => WmCommand::ToggleFloating,
        HypreactCommandKind::ToggleFullscreen => WmCommand::ToggleFullscreen,
        HypreactCommandKind::AssignFocusedWindowToWorkspace => {
            WmCommand::AssignFocusedWindowToWorkspace {
                workspace: input.workspace,
            }
        }
        HypreactCommandKind::ToggleAssignFocusedWindowToWorkspace => {
            WmCommand::ToggleAssignFocusedWindowToWorkspace {
                workspace: input.workspace,
            }
        }
        HypreactCommandKind::FocusWindow => WmCommand::FocusWindow {
            window_id: WindowId(required_string(input.string_value, "window_id")?),
        },
        HypreactCommandKind::FocusDirection => WmCommand::FocusDirection {
            direction: direction_from_ffi(input.direction),
        },
        HypreactCommandKind::SwapDirection => WmCommand::SwapDirection {
            direction: direction_from_ffi(input.direction),
        },
        HypreactCommandKind::ResizeDirection => WmCommand::ResizeDirection {
            direction: direction_from_ffi(input.direction),
        },
        HypreactCommandKind::MoveDirection => WmCommand::MoveDirection {
            direction: direction_from_ffi(input.direction),
        },
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

pub fn wm_command_from_text(input: &str) -> Result<WmCommand, FfiError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(FfiError::InvalidInput("empty command".into()));
    }

    match input {
        "reload-config" => return Ok(WmCommand::ReloadConfig),
        "toggle-floating" => return Ok(WmCommand::ToggleFloating),
        "toggle-fullscreen" => return Ok(WmCommand::ToggleFullscreen),
        "close-focused-window" => return Ok(WmCommand::CloseFocusedWindow),
        "focus-next-window" => return Ok(WmCommand::FocusNextWindow),
        "focus-previous-window" => return Ok(WmCommand::FocusPreviousWindow),
        "select-next-workspace" => return Ok(WmCommand::SelectNextWorkspace),
        "select-previous-workspace" => return Ok(WmCommand::SelectPreviousWorkspace),
        "cycle-layout" => return Ok(WmCommand::CycleLayout { direction: None }),
        "cycle-layout previous" => {
            return Ok(WmCommand::CycleLayout {
                direction: Some(LayoutCycleDirection::Previous),
            });
        }
        _ => {}
    }

    if let Some(value) = input.strip_prefix("spawn ") {
        return Ok(WmCommand::Spawn {
            command: non_empty_suffix(value, "command")?,
        });
    }
    if let Some(value) = input.strip_prefix("set-layout ") {
        return Ok(WmCommand::SetLayout {
            name: non_empty_suffix(value, "layout name")?,
        });
    }
    if let Some(value) = input.strip_prefix("activate-workspace ") {
        return Ok(WmCommand::ActivateWorkspace {
            workspace_id: WorkspaceId(non_empty_suffix(value, "workspace id")?),
        });
    }
    if let Some(value) = input.strip_prefix("select-workspace ") {
        return Ok(WmCommand::SelectWorkspace {
            workspace_id: WorkspaceId(non_empty_suffix(value, "workspace id")?),
        });
    }
    if let Some(value) = input.strip_prefix("focus-window ") {
        return Ok(WmCommand::FocusWindow {
            window_id: WindowId(non_empty_suffix(value, "window id")?),
        });
    }
    if let Some(value) = input.strip_prefix("focus-direction ") {
        return Ok(WmCommand::FocusDirection {
            direction: parse_direction(value)?,
        });
    }
    if let Some(value) = input.strip_prefix("swap-direction ") {
        return Ok(WmCommand::SwapDirection {
            direction: parse_direction(value)?,
        });
    }
    if let Some(value) = input.strip_prefix("move-direction ") {
        return Ok(WmCommand::MoveDirection {
            direction: parse_direction(value)?,
        });
    }
    if let Some(value) = input.strip_prefix("resize-direction ") {
        return Ok(WmCommand::ResizeDirection {
            direction: parse_direction(value)?,
        });
    }
    if let Some(value) = input.strip_prefix("view-workspace ") {
        return Ok(WmCommand::ViewWorkspace {
            workspace: parse_workspace(value)?,
        });
    }
    if let Some(value) = input.strip_prefix("assign-focused-window-to-workspace ") {
        return Ok(WmCommand::AssignFocusedWindowToWorkspace {
            workspace: parse_workspace(value)?,
        });
    }
    if let Some(value) = input.strip_prefix("toggle-assign-focused-window-to-workspace ") {
        return Ok(WmCommand::ToggleAssignFocusedWindowToWorkspace {
            workspace: parse_workspace(value)?,
        });
    }

    Err(FfiError::InvalidInput(
        "unsupported hypreact command".into(),
    ))
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

fn parse_direction(value: &str) -> Result<FocusDirection, FfiError> {
    match value.trim() {
        "l" | "left" => Ok(FocusDirection::Left),
        "r" | "right" => Ok(FocusDirection::Right),
        "u" | "up" => Ok(FocusDirection::Up),
        "d" | "down" => Ok(FocusDirection::Down),
        other => Err(FfiError::InvalidInput(format!(
            "unknown direction: {other}"
        ))),
    }
}

fn parse_workspace(value: &str) -> Result<u8, FfiError> {
    value
        .trim()
        .parse::<u8>()
        .map_err(|_| FfiError::InvalidInput(format!("invalid workspace: {value}")))
}

fn non_empty_suffix(value: &str, field: &str) -> Result<String, FfiError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(FfiError::InvalidInput(format!("missing {field}")));
    }
    Ok(value.to_string())
}
