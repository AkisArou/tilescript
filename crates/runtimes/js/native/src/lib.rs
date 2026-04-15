pub mod authored;
mod module_graph_runtime;
pub mod runtime;

use hypreact_config::model::{ConfigPaths, LayoutConfigError};
use hypreact_config::runtime::RuntimeBundle;

pub use hypreact_runtime_js_core::{
    JavaScriptModule, JavaScriptModuleGraph, decode_js_layout_value,
    decode_runtime_graph_payload, encode_runtime_graph_payload,
};
pub use module_graph_runtime::{
    ModuleGraphRuntimeError, call_entry_export_with_json_arg, evaluate_entry_export_to_json,
    format_js_error,
};
pub use runtime::QuickJsPreparedLayoutRuntime;

pub type DefaultLayoutRuntime =
    runtime::QuickJsPreparedLayoutRuntime<hypreact_runtime_js_core::loader::RuntimeProjectLayoutSourceLoader>;

pub fn build_default_runtime(paths: &ConfigPaths) -> DefaultLayoutRuntime {
    let resolver = hypreact_runtime_js_core::loader::RuntimePathResolver::new(
        paths
            .authored_config
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| std::path::PathBuf::from(".")),
        paths
            .prepared_config
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| std::path::PathBuf::from(".")),
    );
    let loader = hypreact_runtime_js_core::loader::RuntimeProjectLayoutSourceLoader::new(resolver);
    runtime::QuickJsPreparedLayoutRuntime::with_loader(loader)
}

pub fn build_runtime_bundle(paths: &ConfigPaths) -> Result<RuntimeBundle, LayoutConfigError> {
    let runtime = build_default_runtime(paths);
    Ok(RuntimeBundle {
        config_runtime: Box::new(runtime.clone()),
        layout_runtime: Box::new(runtime),
    })
}
