use tilescript_core::runtime::runtime_error::RuntimeError;
use serde::{Deserialize, Serialize};

use crate::module_graph::JavaScriptModuleGraph;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuntimeGraphPayload {
    graph: JavaScriptModuleGraph,
    authored_dependencies: Vec<String>,
}

pub fn encode_runtime_graph_payload(
    graph: &JavaScriptModuleGraph,
    authored_dependencies: &[String],
) -> serde_json::Value {
    serde_json::to_value(RuntimeGraphPayload {
        graph: graph.clone(),
        authored_dependencies: authored_dependencies.to_vec(),
    })
    .unwrap_or(serde_json::Value::Null)
}

pub fn decode_runtime_graph_payload(
    payload: &serde_json::Value,
) -> Result<JavaScriptModuleGraph, RuntimeError> {
    serde_json::from_value::<RuntimeGraphPayload>(payload.clone())
        .map(|payload| payload.graph)
        .map_err(|error| RuntimeError::Other {
            message: format!("invalid runtime payload: {error}"),
        })
}

pub fn decode_runtime_graph_authored_dependencies(
    payload: &serde_json::Value,
) -> Result<Vec<String>, RuntimeError> {
    serde_json::from_value::<RuntimeGraphPayload>(payload.clone())
        .map(|payload| payload.authored_dependencies)
        .map_err(|error| RuntimeError::Other {
            message: format!("invalid runtime payload: {error}"),
        })
}
