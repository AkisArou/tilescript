use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum RuntimeError {
    #[error("runtime operation is not implemented: {0}")]
    NotImplemented(String),
    #[error("javascript evaluation failed: {message}")]
    JavaScript { message: String },
    #[error("layout module `{name}` did not provide `{export}` export")]
    MissingExport { name: String, export: String },
    #[error("layout module `{name}` export `{export}` is not callable")]
    NonCallableExport { name: String, export: String },
    #[error("layout module `{name}` source is unavailable")]
    MissingRuntimeSource { name: String },
    #[error("js to layout conversion failed for layout `{name}`: {message}")]
    ValueConversion { name: String, message: String },
    #[error("validation failed: {message}")]
    Validation { message: String },
    #[error("config runtime failed: {message}")]
    Config { message: String },
    #[error("runtime failed: {message}")]
    Other { message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RuntimeRefreshSummary {
    pub refreshed_files: usize,
    pub pruned_files: usize,
}

impl RuntimeRefreshSummary {
    pub fn is_noop(self) -> bool {
        self.refreshed_files == 0 && self.pruned_files == 0
    }
}
