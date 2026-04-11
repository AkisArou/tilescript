use serde::{Deserialize, Serialize};

use crate::resize::LayoutAdjustmentState;
use crate::snapshot::OutputSnapshot;
use crate::{LayoutSpace, OutputId, WindowId, WorkspaceId};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LayoutEvaluationContext {
    pub monitor: LayoutMonitorContext,
    pub workspace: LayoutWorkspaceContext,
    pub windows: Vec<LayoutWindowContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<LayoutStateContext>,
    #[serde(skip)]
    pub workspace_id: WorkspaceId,
    #[serde(skip)]
    pub output: Option<OutputSnapshot>,
    #[serde(skip)]
    pub selected_layout_name: Option<String>,
    #[serde(skip)]
    pub space: LayoutSpace,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutEvaluationDependencies {
    pub uses_monitor_size: bool,
    pub uses_monitor_scale: bool,
    pub uses_window_count: bool,
    pub uses_window_order: bool,
    pub uses_window_focus: bool,
    pub uses_visible_window_ids: bool,
    pub uses_workspace_name: bool,
    pub uses_workspace_names: bool,
    pub uses_selected_layout_name: bool,
    pub uses_layout_adjustments: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutMonitorContext {
    pub name: String,
    pub width: u32,
    pub height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutWorkspaceContext {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workspaces: Vec<String>,
    #[serde(rename = "windowCount")]
    pub window_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutWindowContext {
    pub id: WindowId,
    pub app_id: Option<String>,
    pub title: Option<String>,
    pub class: Option<String>,
    pub instance: Option<String>,
    pub role: Option<String>,
    pub shell: Option<String>,
    pub window_type: Option<String>,
    pub floating: bool,
    pub fullscreen: bool,
    pub focused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutStateContext {
    pub focused_window_id: Option<WindowId>,
    pub current_output_id: Option<OutputId>,
    pub current_workspace_id: Option<WorkspaceId>,
    pub visible_window_ids: Vec<WindowId>,
    pub workspace_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_layout_name: Option<String>,
    #[serde(default, skip_serializing_if = "LayoutAdjustmentState::is_empty")]
    pub layout_adjustments: LayoutAdjustmentState,
}
