use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaScriptModule {
    pub specifier: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub resolved_imports: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaScriptModuleGraph {
    pub entry: String,
    pub modules: Vec<JavaScriptModule>,
}
