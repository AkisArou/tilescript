use std::path::PathBuf;

use serde::Serialize;

use hypreact_core::SourceLayoutNode;
use hypreact_core::wm::WmModel;
use hypreact_layout_runtime::LayoutDiagnostic;
use hypreact_layout_runtime::LayoutRuntimeService;

pub struct HypreactRuntimeHandle {
    pub model: WmModel,
    pub layout_runtime: Option<LayoutRuntimeState>,
}

pub struct LayoutRuntimeState {
    pub config_path: PathBuf,
    pub service: LayoutRuntimeService,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusResult {
    pub changed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_window_id: Option<String>,
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<LayoutDiagnostic>,
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
