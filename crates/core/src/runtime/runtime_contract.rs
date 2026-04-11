use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::SourceLayoutNode;
use crate::snapshot::{StateSnapshot, WorkspaceSnapshot};

use super::layout_context::LayoutEvaluationContext;
use super::prepared_layout::PreparedLayout;
use super::runtime_error::{RuntimeError, RuntimeRefreshSummary};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutModuleContract {
    pub export_name: String,
}

impl Default for LayoutModuleContract {
    fn default() -> Self {
        Self { export_name: "default".into() }
    }
}

pub trait PreparedLayoutRuntime: std::fmt::Debug {
    type Config;

    fn prepare_layout(
        &self,
        config: &Self::Config,
        workspace: &WorkspaceSnapshot,
    ) -> Result<Option<PreparedLayout>, RuntimeError>;

    fn build_context(
        &self,
        state: &StateSnapshot,
        workspace: &WorkspaceSnapshot,
        artifact: Option<&PreparedLayout>,
    ) -> LayoutEvaluationContext;

    fn evaluate_layout(
        &self,
        artifact: &PreparedLayout,
        context: &LayoutEvaluationContext,
    ) -> Result<SourceLayoutNode, RuntimeError>;

    fn contract(&self) -> LayoutModuleContract;
}

pub trait AuthoringLayoutRuntime: PreparedLayoutRuntime {
    fn load_authored_config(&self, path: &Path) -> Result<Self::Config, RuntimeError>;
    fn load_prepared_config(&self, path: &Path) -> Result<Self::Config, RuntimeError>;
    fn refresh_prepared_config(
        &self,
        authored: &Path,
        runtime: &Path,
    ) -> Result<RuntimeRefreshSummary, RuntimeError>;
    fn rebuild_prepared_config(
        &self,
        authored: &Path,
        runtime: &Path,
    ) -> Result<RuntimeRefreshSummary, RuntimeError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeInfo {
    pub name: String,
}
