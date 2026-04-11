use std::collections::BTreeMap;
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
    pub workspace_overrides: BTreeMap<String, WorkspaceLayoutOverride>,
}

#[derive(Debug, Clone, Default)]
pub struct WorkspaceLayoutOverride {
    pub master_ratio: Option<f64>,
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
    pub master_ratio: Option<f64>,
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
