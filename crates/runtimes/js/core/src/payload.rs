use hypreact_core::runtime::runtime_error::RuntimeError;

use crate::module_graph::JavaScriptModuleGraph;

pub fn encode_runtime_graph_payload(graph: &JavaScriptModuleGraph) -> serde_json::Value {
    serde_json::to_value(graph).unwrap_or(serde_json::Value::Null)
}

pub fn decode_runtime_graph_payload(
    payload: &serde_json::Value,
) -> Result<JavaScriptModuleGraph, RuntimeError> {
    serde_json::from_value(payload.clone()).map_err(|error| RuntimeError::Other {
        message: format!("invalid runtime payload: {error}"),
    })
}
