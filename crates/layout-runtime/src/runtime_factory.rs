use hypreact_config::model::{ConfigPaths, LayoutConfigError};
use hypreact_config::runtime::RuntimeBundle;
use hypreact_core::runtime::runtime_kind::RuntimeKind;

pub fn build_runtime_bundle(paths: &ConfigPaths) -> Result<RuntimeBundle, LayoutConfigError> {
    match runtime_kind_for_authored_config(paths) {
        RuntimeKind::Js => hypreact_runtime_js_native::build_runtime_bundle(paths),
        RuntimeKind::Lua => hypreact_runtime_lua_native::build_runtime_bundle(paths),
    }
}

fn runtime_kind_for_authored_config(paths: &ConfigPaths) -> RuntimeKind {
    match paths.authored_config.extension().and_then(|ext| ext.to_str()) {
        Some("lua") => RuntimeKind::Lua,
        _ => RuntimeKind::Js,
    }
}
