use std::ffi::c_char;

#[repr(C)]
pub struct TilescriptWindowSync {
    pub window_id: *const c_char,
    pub previous_focused_window_id: *const c_char,
    pub workspace_id: *const c_char,
    pub output_id: *const c_char,
    pub is_xwayland: bool,
    pub mapped: bool,
    pub title: *const c_char,
    pub app_id: *const c_char,
    pub class_name: *const c_char,
    pub instance: *const c_char,
    pub role: *const c_char,
    pub window_type: *const c_char,
    pub urgent: bool,
    pub floating: bool,
    pub fullscreen: bool,
}

#[repr(C)]
pub struct TilescriptOutputSync {
    pub output_id: *const c_char,
    pub name: *const c_char,
    pub logical_width: u32,
    pub logical_height: u32,
}

#[repr(C)]
pub struct TilescriptWorkspaceLayoutSpaceSync {
    pub workspace_id: *const c_char,
    pub output_id: *const c_char,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[repr(C)]
pub struct TilescriptDiagnosticRange {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

#[repr(C)]
pub struct TilescriptDiagnostic {
    pub source: *mut c_char,
    pub severity: *mut c_char,
    pub code: *mut c_char,
    pub message: *mut c_char,
    pub path: *mut c_char,
    pub range: TilescriptDiagnosticRange,
}

#[repr(C)]
pub struct TilescriptPlacementGeometry {
    pub window_id: *const c_char,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[repr(C)]
pub struct TilescriptPlacementResult {
    pub geometries: *mut TilescriptPlacementGeometry,
    pub geometry_count: usize,
}

#[repr(C)]
pub struct TilescriptStringResult {
    pub value: *mut c_char,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TilescriptDirection {
    Left = 0,
    Right = 1,
    Up = 2,
    Down = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TilescriptLayoutCycleDirection {
    Next = 0,
    Previous = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TilescriptCommandKind {
    Spawn = 0,
    ReloadConfig = 1,
    SetLayout = 2,
    CycleLayout = 3,
    ViewWorkspace = 4,
    ActivateWorkspace = 5,
    ToggleFloating = 6,
    ToggleFullscreen = 7,
    AssignFocusedWindowToWorkspace = 8,
    ToggleAssignFocusedWindowToWorkspace = 9,
    FocusWindow = 10,
    FocusDirection = 11,
    SwapDirection = 12,
    ResizeDirection = 13,
    MoveDirection = 14,
    FocusNextWindow = 15,
    FocusPreviousWindow = 16,
    SelectNextWorkspace = 17,
    SelectPreviousWorkspace = 18,
    SelectWorkspace = 19,
    CloseFocusedWindow = 20,
}

#[repr(C)]
pub struct TilescriptCommandInput {
    pub kind: TilescriptCommandKind,
    pub string_value: *const c_char,
    pub workspace: u8,
    pub direction: TilescriptDirection,
    pub cycle_direction: TilescriptLayoutCycleDirection,
    pub has_cycle_direction: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TilescriptActionKind {
    SpawnCommand = 0,
    ReloadConfig = 1,
    SetLayout = 2,
    CycleLayout = 3,
    ActivateWorkspace = 4,
    AssignFocusedWindowToWorkspace = 5,
    ToggleAssignFocusedWindowToWorkspace = 6,
    ToggleFloating = 7,
    ToggleFullscreen = 8,
    FocusWindow = 9,
    FocusDirection = 10,
    FocusNextWindow = 11,
    FocusPreviousWindow = 12,
    SwapDirection = 13,
    MoveDirection = 14,
    ResizeDirection = 15,
    CloseFocusedWindow = 16,
}

#[repr(C)]
pub struct TilescriptAction {
    pub kind: TilescriptActionKind,
    pub string_value: *mut c_char,
    pub workspace: u8,
    pub direction: TilescriptDirection,
    pub cycle_direction: TilescriptLayoutCycleDirection,
    pub has_cycle_direction: bool,
}

#[repr(C)]
pub struct TilescriptActionResult {
    pub actions: *mut TilescriptAction,
    pub action_count: usize,
    pub error: *mut c_char,
}

#[repr(C)]
pub struct TilescriptStateResult {
    pub workspace_names: *mut *mut c_char,
    pub workspace_name_count: usize,
    pub current_workspace_id: *mut c_char,
    pub current_output_id: *mut c_char,
    pub focused_window_id: *mut c_char,
}

#[repr(C)]
pub struct TilescriptLayoutStatusResult {
    pub loaded: bool,
    pub config_path: *mut c_char,
    pub selected_layout_name: *mut c_char,
    pub error: *mut c_char,
    pub diagnostics: *mut TilescriptDiagnostic,
    pub diagnostic_count: usize,
    pub workspace_names: *mut *mut c_char,
    pub workspace_name_count: usize,
}

#[repr(C)]
pub struct TilescriptStatusResult {
    pub changed: bool,
    pub focused_window_id: *mut c_char,
    pub error: *mut c_char,
}
