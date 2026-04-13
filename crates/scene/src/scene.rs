use crate::style::ComputedStyle;
use hypreact_core::resize::WorkspaceResizeState;
use hypreact_core::runtime::prepared_layout::PreparedStylesheets;
use hypreact_core::{
    LayoutNodeMeta, LayoutRect, LayoutSpace, OutputId, ResolvedLayoutNode, WindowId, WorkspaceId,
};

#[derive(Debug, Clone, PartialEq)]
pub enum LayoutSnapshotNode {
    Workspace {
        meta: LayoutNodeMeta,
        rect: LayoutRect,
        styles: Option<SceneNodeStyle>,
        children: Vec<LayoutSnapshotNode>,
    },
    Group {
        meta: LayoutNodeMeta,
        rect: LayoutRect,
        styles: Option<SceneNodeStyle>,
        children: Vec<LayoutSnapshotNode>,
    },
    Content {
        meta: LayoutNodeMeta,
        rect: LayoutRect,
        styles: Option<SceneNodeStyle>,
        text: Option<String>,
        children: Vec<LayoutSnapshotNode>,
    },
    Window {
        meta: LayoutNodeMeta,
        rect: LayoutRect,
        styles: Option<SceneNodeStyle>,
        window_id: Option<WindowId>,
        children: Vec<LayoutSnapshotNode>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneRequest {
    pub workspace_id: WorkspaceId,
    pub output_id: Option<OutputId>,
    pub layout_name: Option<String>,
    pub root: ResolvedLayoutNode,
    pub stylesheets: PreparedStylesheets,
    pub space: LayoutSpace,
    pub resize_state: WorkspaceResizeState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneResponse {
    pub root: LayoutSnapshotNode,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SceneNodeStyle {
    pub layout: ComputedStyle,
}

impl LayoutSnapshotNode {
    pub fn rect(&self) -> LayoutRect {
        match self {
            Self::Workspace { rect, .. }
            | Self::Group { rect, .. }
            | Self::Content { rect, .. }
            | Self::Window { rect, .. } => *rect,
        }
    }

    pub fn meta(&self) -> &LayoutNodeMeta {
        match self {
            Self::Workspace { meta, .. }
            | Self::Group { meta, .. }
            | Self::Content { meta, .. }
            | Self::Window { meta, .. } => meta,
        }
    }

    pub fn children(&self) -> &[LayoutSnapshotNode] {
        match self {
            Self::Workspace { children, .. }
            | Self::Group { children, .. }
            | Self::Content { children, .. }
            | Self::Window { children, .. } => children,
        }
    }

    pub fn styles(&self) -> Option<&SceneNodeStyle> {
        match self {
            Self::Workspace { styles, .. }
            | Self::Group { styles, .. }
            | Self::Content { styles, .. }
            | Self::Window { styles, .. } => styles.as_ref(),
        }
    }

    pub fn find_by_node_id(&self, node_id: &str) -> Option<&LayoutSnapshotNode> {
        if self.meta().id.as_deref() == Some(node_id) {
            return Some(self);
        }

        self.children()
            .iter()
            .find_map(|child| child.find_by_node_id(node_id))
    }

    pub fn find_by_window_id(&self, window_id: &WindowId) -> Option<&LayoutSnapshotNode> {
        if matches!(self, Self::Window { window_id: Some(id), .. } if id == window_id) {
            return Some(self);
        }

        self.children()
            .iter()
            .find_map(|child| child.find_by_window_id(window_id))
    }

    pub fn collect_windows<'a>(&'a self, windows: &mut Vec<&'a LayoutSnapshotNode>) {
        if matches!(self, Self::Window { .. }) {
            windows.push(self);
            return;
        }

        for child in self.children() {
            child.collect_windows(windows);
        }
    }

    pub fn window_nodes(&self) -> Vec<&LayoutSnapshotNode> {
        let mut windows = Vec::new();
        self.collect_windows(&mut windows);
        windows
    }
}
