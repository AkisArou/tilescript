use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::WindowId;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LayoutSpace {
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LayoutRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LayoutNodeType {
    Workspace,
    Group,
    Window,
    Slot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeLayoutNodeType {
    Workspace,
    Group,
    Window,
    Content,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeContentKind {
    Container,
    Text,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutNodeMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub class: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub data: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchKey {
    AppId,
    Title,
    Class,
    Instance,
    Role,
    Shell,
    WindowType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchClause {
    pub key: MatchKey,
    pub value: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowMatch {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clauses: Vec<MatchClause>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemainingTake {
    #[serde(rename = "remaining")]
    Remaining,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SlotTake {
    Count(u32),
    Remaining(RemainingTake),
}

impl Default for SlotTake {
    fn default() -> Self {
        Self::Remaining(RemainingTake::Remaining)
    }
}

impl SlotTake {
    pub fn is_remaining(&self) -> bool {
        matches!(self, Self::Remaining(RemainingTake::Remaining))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SourceLayoutNode {
    Workspace {
        #[serde(flatten)]
        meta: LayoutNodeMeta,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        children: Vec<SourceLayoutNode>,
    },
    Group {
        #[serde(flatten)]
        meta: LayoutNodeMeta,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        children: Vec<SourceLayoutNode>,
    },
    Window {
        #[serde(flatten)]
        meta: LayoutNodeMeta,
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "match")]
        window_match: Option<WindowMatch>,
    },
    Slot {
        #[serde(flatten)]
        meta: LayoutNodeMeta,
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "match")]
        window_match: Option<WindowMatch>,
        #[serde(default, skip_serializing_if = "SlotTake::is_remaining")]
        take: SlotTake,
    },
}

impl SourceLayoutNode {
    pub fn node_type(&self) -> LayoutNodeType {
        match self {
            Self::Workspace { .. } => LayoutNodeType::Workspace,
            Self::Group { .. } => LayoutNodeType::Group,
            Self::Window { .. } => LayoutNodeType::Window,
            Self::Slot { .. } => LayoutNodeType::Slot,
        }
    }

    pub fn meta(&self) -> &LayoutNodeMeta {
        match self {
            Self::Workspace { meta, .. }
            | Self::Group { meta, .. }
            | Self::Window { meta, .. }
            | Self::Slot { meta, .. } => meta,
        }
    }

    pub fn children(&self) -> &[SourceLayoutNode] {
        match self {
            Self::Workspace { children, .. } | Self::Group { children, .. } => children,
            Self::Window { .. } | Self::Slot { .. } => &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ResolvedLayoutNode {
    Workspace {
        #[serde(flatten)]
        meta: LayoutNodeMeta,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        children: Vec<ResolvedLayoutNode>,
    },
    Group {
        #[serde(flatten)]
        meta: LayoutNodeMeta,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        children: Vec<ResolvedLayoutNode>,
    },
    Window {
        #[serde(flatten)]
        meta: LayoutNodeMeta,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        window_id: Option<WindowId>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        children: Vec<ResolvedLayoutNode>,
    },
    Content {
        #[serde(flatten)]
        meta: LayoutNodeMeta,
        kind: RuntimeContentKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        children: Vec<ResolvedLayoutNode>,
    },
}

impl ResolvedLayoutNode {
    pub fn node_type(&self) -> RuntimeLayoutNodeType {
        match self {
            Self::Workspace { .. } => RuntimeLayoutNodeType::Workspace,
            Self::Group { .. } => RuntimeLayoutNodeType::Group,
            Self::Window { .. } => RuntimeLayoutNodeType::Window,
            Self::Content { .. } => RuntimeLayoutNodeType::Content,
        }
    }

    pub fn meta(&self) -> &LayoutNodeMeta {
        match self {
            Self::Workspace { meta, .. }
            | Self::Group { meta, .. }
            | Self::Window { meta, .. }
            | Self::Content { meta, .. } => meta,
        }
    }

    pub fn children(&self) -> &[ResolvedLayoutNode] {
        match self {
            Self::Workspace { children, .. }
            | Self::Group { children, .. }
            | Self::Window { children, .. }
            | Self::Content { children, .. } => children,
        }
    }
}
