pub mod authored;
pub mod compile;
pub mod graph;
mod layout_value;
pub mod loader;
mod module_graph_runtime;
mod module_graph;
mod payload;
pub mod runtime;

use hypreact_config::model::{ConfigPaths, LayoutConfigError};
use hypreact_config::runtime::RuntimeBundle;

pub use module_graph_runtime::{
    ModuleGraphRuntimeError, call_entry_export_with_json_arg, evaluate_entry_export_to_json,
    format_js_error,
};
pub use layout_value::decode_js_layout_value;
pub use module_graph::{JavaScriptModule, JavaScriptModuleGraph};
pub use payload::{decode_runtime_graph_payload, encode_runtime_graph_payload};
pub use runtime::QuickJsPreparedLayoutRuntime;

pub type DefaultLayoutRuntime =
    runtime::QuickJsPreparedLayoutRuntime<loader::RuntimeProjectLayoutSourceLoader>;

pub fn build_default_runtime(paths: &ConfigPaths) -> DefaultLayoutRuntime {
    let resolver = loader::RuntimePathResolver::new(
        paths
            .authored_config
            .parent()
            .and_then(|dir| dir.parent())
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| std::path::PathBuf::from(".")),
        paths
            .prepared_config
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| std::path::PathBuf::from(".")),
    );
    let loader = loader::RuntimeProjectLayoutSourceLoader::new(resolver);
    runtime::QuickJsPreparedLayoutRuntime::with_loader(loader)
}

pub fn build_runtime_bundle(paths: &ConfigPaths) -> Result<RuntimeBundle, LayoutConfigError> {
    let runtime = build_default_runtime(paths);
    Ok(RuntimeBundle {
        config_runtime: Box::new(runtime.clone()),
        layout_runtime: Box::new(runtime),
    })
}
