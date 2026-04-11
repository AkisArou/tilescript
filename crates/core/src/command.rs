use serde::{Deserialize, Serialize};

use crate::{WindowId, WorkspaceId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FocusDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LayoutCycleDirection {
    Next,
    Previous,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum WmCommand {
    Spawn {
        command: String,
    },
    ReloadConfig,
    SetLayout {
        name: String,
    },
    CycleLayout {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        direction: Option<LayoutCycleDirection>,
    },
    ViewWorkspace {
        workspace: u8,
    },
    ActivateWorkspace {
        workspace_id: WorkspaceId,
    },
    ToggleFloating,
    ToggleFullscreen,
    AssignFocusedWindowToWorkspace {
        workspace: u8,
    },
    ToggleAssignFocusedWindowToWorkspace {
        workspace: u8,
    },
    FocusWindow {
        window_id: WindowId,
    },
    FocusDirection {
        direction: FocusDirection,
    },
    SwapDirection {
        direction: FocusDirection,
    },
    ResizeDirection {
        direction: FocusDirection,
    },
    MoveDirection {
        direction: FocusDirection,
    },
    FocusNextWindow,
    FocusPreviousWindow,
    SelectNextWorkspace,
    SelectPreviousWorkspace,
    SelectWorkspace {
        workspace_id: WorkspaceId,
    },
    CloseFocusedWindow,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_serializes_with_stable_tag() {
        let json = serde_json::to_value(WmCommand::ToggleFullscreen).unwrap();

        assert_eq!(json["type"], "toggle-fullscreen");
    }
}
