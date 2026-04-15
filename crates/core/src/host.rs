use serde::{Deserialize, Serialize};

use crate::command::{FocusDirection, LayoutCycleDirection, WmCommand};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum HostAction {
    SpawnCommand { command: String },
    ReloadConfig,
    SetLayout { name: String },
    CycleLayout { direction: Option<LayoutCycleDirection> },
    ActivateWorkspace { workspace_id: String },
    AssignFocusedWindowToWorkspace { workspace: u8 },
    ToggleAssignFocusedWindowToWorkspace { workspace: u8 },
    ToggleFloating,
    ToggleFullscreen,
    FocusWindow { window_id: String },
    FocusDirection { direction: FocusDirection },
    FocusNextWindow,
    FocusPreviousWindow,
    SwapDirection { direction: FocusDirection },
    MoveDirection { direction: FocusDirection },
    ResizeDirection { direction: FocusDirection },
    CloseFocusedWindow,
}

pub fn dispatch_wm_command(command: WmCommand) -> Vec<HostAction> {
    match command {
        WmCommand::Spawn { command } => vec![HostAction::SpawnCommand { command }],
        WmCommand::ReloadConfig => vec![HostAction::ReloadConfig],
        WmCommand::SetLayout { name } => vec![HostAction::SetLayout { name }],
        WmCommand::CycleLayout { direction } => vec![HostAction::CycleLayout { direction }],
        WmCommand::ActivateWorkspace { workspace_id }
        | WmCommand::SelectWorkspace { workspace_id } => {
            vec![HostAction::ActivateWorkspace { workspace_id: workspace_id.0 }]
        }
        WmCommand::SelectNextWorkspace => {
            vec![HostAction::ActivateWorkspace { workspace_id: "e+1".to_string() }]
        }
        WmCommand::SelectPreviousWorkspace => {
            vec![HostAction::ActivateWorkspace { workspace_id: "e-1".to_string() }]
        }
        WmCommand::ViewWorkspace { workspace } => {
            vec![HostAction::ActivateWorkspace { workspace_id: workspace.to_string() }]
        }
        WmCommand::AssignFocusedWindowToWorkspace { workspace } => {
            vec![HostAction::AssignFocusedWindowToWorkspace { workspace }]
        }
        WmCommand::ToggleAssignFocusedWindowToWorkspace { workspace } => {
            vec![HostAction::ToggleAssignFocusedWindowToWorkspace { workspace }]
        }
        WmCommand::ToggleFloating => vec![HostAction::ToggleFloating],
        WmCommand::ToggleFullscreen => vec![HostAction::ToggleFullscreen],
        WmCommand::FocusWindow { window_id } => {
            vec![HostAction::FocusWindow { window_id: window_id.0 }]
        }
        WmCommand::FocusDirection { direction } => vec![HostAction::FocusDirection { direction }],
        WmCommand::FocusNextWindow => vec![HostAction::FocusNextWindow],
        WmCommand::FocusPreviousWindow => vec![HostAction::FocusPreviousWindow],
        WmCommand::SwapDirection { direction } => vec![HostAction::SwapDirection { direction }],
        WmCommand::MoveDirection { direction } => vec![HostAction::MoveDirection { direction }],
        WmCommand::ResizeDirection { direction } => vec![HostAction::ResizeDirection { direction }],
        WmCommand::CloseFocusedWindow => vec![HostAction::CloseFocusedWindow],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{WindowId, WorkspaceId};

    #[test]
    fn dispatch_maps_commands_to_host_actions() {
        assert_eq!(
            dispatch_wm_command(WmCommand::FocusWindow { window_id: WindowId("win-1".into()) }),
            vec![HostAction::FocusWindow { window_id: "win-1".into() }]
        );

        assert_eq!(
            dispatch_wm_command(WmCommand::SelectWorkspace {
                workspace_id: WorkspaceId("2".into())
            }),
            vec![HostAction::ActivateWorkspace { workspace_id: "2".into() }]
        );
    }
}
