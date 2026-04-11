use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FfiResponse<T> {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

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

pub fn response_ok<T: Serialize>(data: T) -> Result<String, FfiError> {
    serde_json::to_string(&FfiResponse {
        ok: true,
        data: Some(data),
        error: None,
    })
    .map_err(|error| FfiError::InvalidJson(error.to_string()))
}

pub fn response_err(error: FfiError) -> Result<String, FfiError> {
    serde_json::to_string::<FfiResponse<serde_json::Value>>(&FfiResponse {
        ok: false,
        data: None,
        error: Some(error.to_string()),
    })
    .map_err(|json_error| FfiError::InvalidJson(json_error.to_string()))
}

pub fn fallback_json(error: FfiError) -> String {
    format!(
        r#"{{"ok":false,"error":{}}}"#,
        serde_json::to_string(&error.to_string())
            .unwrap_or_else(|_| "\"unknown ffi error\"".to_string())
    )
}
