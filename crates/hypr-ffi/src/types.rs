use std::ffi::c_char;
use std::path::PathBuf;

use serde::Serialize;

use hypreact_core::wm::WmModel;
use hypreact_core::SourceLayoutNode;
use hypreact_layout_runtime::LayoutRuntimeService;

pub struct HypreactRuntimeHandle {
    pub model: WmModel,
    pub layout_runtime: Option<LayoutRuntimeState>,
}

pub struct LayoutRuntimeState {
    pub config_path: PathBuf,
    pub service: LayoutRuntimeService,
}

#[repr(C)]
pub struct HypreactWindowSync {
    pub window_id: *const c_char,
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
pub struct HypreactOutputSync {
    pub output_id: *const c_char,
    pub name: *const c_char,
    pub logical_width: u32,
    pub logical_height: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusResult {
    pub changed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutRuntimeStatus {
    pub config_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_names: Option<Vec<String>>,
    pub loaded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_layout_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<SourceLayoutNode>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub window_geometries: Vec<WindowGeometryEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ordered_window_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowGeometryEntry {
    pub window_id: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[repr(C)]
pub struct HypreactPlacementGeometry {
    pub window_id: *const c_char,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[repr(C)]
pub struct HypreactPlacementResult {
    pub geometries: *mut HypreactPlacementGeometry,
    pub geometry_count: usize,
}

#[repr(C)]
pub struct HypreactStringResult {
    pub value: *mut c_char,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HypreactDirection {
    Left = 0,
    Right = 1,
    Up = 2,
    Down = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HypreactLayoutCycleDirection {
    Next = 0,
    Previous = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HypreactCommandKind {
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
pub struct HypreactCommandInput {
    pub kind: HypreactCommandKind,
    pub string_value: *const c_char,
    pub workspace: u8,
    pub direction: HypreactDirection,
    pub cycle_direction: HypreactLayoutCycleDirection,
    pub has_cycle_direction: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HypreactActionKind {
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
pub struct HypreactAction {
    pub kind: HypreactActionKind,
    pub string_value: *mut c_char,
    pub workspace: u8,
    pub direction: HypreactDirection,
    pub cycle_direction: HypreactLayoutCycleDirection,
    pub has_cycle_direction: bool,
}

#[repr(C)]
pub struct HypreactActionResult {
    pub actions: *mut HypreactAction,
    pub action_count: usize,
    pub error: *mut c_char,
}

#[repr(C)]
pub struct HypreactStateResult {
    pub workspace_names: *mut *mut c_char,
    pub workspace_name_count: usize,
    pub current_workspace_id: *mut c_char,
    pub current_output_id: *mut c_char,
    pub focused_window_id: *mut c_char,
}

#[repr(C)]
pub struct HypreactLayoutStatusResult {
    pub loaded: bool,
    pub config_path: *mut c_char,
    pub selected_layout_name: *mut c_char,
    pub error: *mut c_char,
    pub workspace_names: *mut *mut c_char,
    pub workspace_name_count: usize,
}

#[repr(C)]
pub struct HypreactStatusResult {
    pub changed: bool,
    pub error: *mut c_char,
}
