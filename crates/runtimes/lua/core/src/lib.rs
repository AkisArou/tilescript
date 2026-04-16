use serde::{Deserialize, Serialize};

pub const LUA_SDK_SOURCE: &str = include_str!("../../../../../packages/sdk/lua/tilescript.lua");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LuaModuleContract {
    pub export_name: String,
}

impl Default for LuaModuleContract {
    fn default() -> Self {
        Self { export_name: "default".into() }
    }
}
