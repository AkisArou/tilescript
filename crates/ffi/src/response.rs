#[derive(Debug, thiserror::Error)]
pub enum FfiError {
    #[error("null pointer")]
    NullPointer,
    #[error("invalid utf-8: {0}")]
    InvalidUtf8(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("invalid json: {0}")]
    InvalidJson(String),
    #[error("panic in ffi call")]
    Panic,
    #[error("failed to allocate ffi string: {0}")]
    NulByte(String),
}
