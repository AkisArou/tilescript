use hypreact_core::runtime::native_artifact::{
    NativeDependencySnapshot, load_cached_stylesheet, load_text_dependency,
};
use hypreact_core::runtime::prepared_layout::{
    PreparedLayout, PreparedStylesheet, PreparedStylesheets, SelectedLayout,
};
use hypreact_core::runtime::runtime_error::RuntimeError;
use hypreact_core::runtime::runtime_kind::RuntimeKind;
use tracing::{debug, warn};

use hypreact_config::model::{Config, LayoutConfigError, LayoutDefinition};

use crate::module_graph::{JavaScriptModule, JavaScriptModuleGraph};
use crate::payload::{decode_runtime_graph_payload, encode_runtime_graph_payload};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePathResolver {
    pub project_root: std::path::PathBuf,
    pub runtime_root: std::path::PathBuf,
}

impl RuntimePathResolver {
    pub fn new(
        project_root: impl Into<std::path::PathBuf>,
        runtime_root: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self { project_root: project_root.into(), runtime_root: runtime_root.into() }
    }

    pub fn resolve_module_path(&self, module: &str) -> std::path::PathBuf {
        let module_path = std::path::Path::new(module);
        if module_path.is_absolute() {
            return module_path.to_path_buf();
        }

        let runtime_candidate = self.runtime_root.join(module_path);
        if runtime_candidate.exists() {
            return runtime_candidate;
        }

        self.project_root.join(module_path)
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LayoutLoadError {
    #[error(transparent)]
    Config(#[from] LayoutConfigError),
    #[error("layout module `{module}` graph is unavailable")]
    MissingRuntimeSource { module: String },
    #[error("layout module `{module}` runtime payload is invalid: {message}")]
    InvalidRuntimePayload { module: String, message: String },
}

#[derive(Debug, Default, Clone, Copy)]
pub struct InlineLayoutSourceLoader;

#[derive(Debug, Default, Clone, Copy)]
pub struct FsLayoutSourceLoader;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProjectLayoutSourceLoader {
    resolver: RuntimePathResolver,
}

pub trait JsLayoutSourceLoader: std::fmt::Debug {
    fn load_runtime_source(
        &self,
        config: &Config,
        workspace: &hypreact_core::snapshot::WorkspaceSnapshot,
    ) -> Result<Option<PreparedLayout>, RuntimeError>;
}

impl RuntimeProjectLayoutSourceLoader {
    pub fn new(resolver: RuntimePathResolver) -> Self {
        Self { resolver }
    }

    pub fn load_definition(
        &self,
        global_stylesheet_path: Option<&str>,
        layout: &LayoutDefinition,
    ) -> Result<PreparedLayout, LayoutLoadError> {
        let module_path = self.resolver.resolve_module_path(&layout.module);
        if let Some(runtime_payload) = layout.runtime_cache_payload.clone() {
            let runtime_graph =
                decode_runtime_graph_payload(&runtime_payload).map_err(|error| {
                    LayoutLoadError::InvalidRuntimePayload {
                        module: layout.module.clone(),
                        message: error.to_string(),
                    }
                })?;
            return Ok(loaded_layout_definition(
                layout,
                global_stylesheet_path,
                module_path.to_string_lossy().into_owned(),
                runtime_graph,
            ));
        }
        let runtime_source = std::fs::read_to_string(&module_path).map_err(|_| {
            LayoutLoadError::MissingRuntimeSource {
                module: module_path.to_string_lossy().into_owned(),
            }
        })?;

        Ok(loaded_layout_definition(
            layout,
            global_stylesheet_path,
            module_path.to_string_lossy().into_owned(),
            single_module_graph(module_path.to_string_lossy().into_owned(), runtime_source),
        ))
    }
}

impl JsLayoutSourceLoader for InlineLayoutSourceLoader {
    fn load_runtime_source(
        &self,
        config: &Config,
        workspace: &hypreact_core::snapshot::WorkspaceSnapshot,
    ) -> Result<Option<PreparedLayout>, RuntimeError> {
        let Some(selected_layout) = config
            .resolve_selected_layout(workspace)
            .map_err(|error| RuntimeError::Config { message: error.to_string() })?
        else {
            return Ok(None);
        };

        Err(RuntimeError::MissingRuntimeSource { name: selected_layout.module })
    }
}

impl FsLayoutSourceLoader {
    pub fn load_definition(
        &self,
        global_stylesheet_path: Option<&str>,
        layout: &LayoutDefinition,
    ) -> Result<PreparedLayout, LayoutLoadError> {
        if let Some(runtime_payload) = layout.runtime_cache_payload.clone() {
            let runtime_graph =
                decode_runtime_graph_payload(&runtime_payload).map_err(|error| {
                    LayoutLoadError::InvalidRuntimePayload {
                        module: layout.module.clone(),
                        message: error.to_string(),
                    }
                })?;
            return Ok(loaded_layout_definition(
                layout,
                global_stylesheet_path,
                layout.module.clone(),
                runtime_graph,
            ));
        }
        let runtime_source = std::fs::read_to_string(&layout.module)
            .map_err(|_| LayoutLoadError::MissingRuntimeSource { module: layout.module.clone() })?;

        Ok(loaded_layout_definition(
            layout,
            global_stylesheet_path,
            layout.module.clone(),
            single_module_graph(layout.module.clone(), runtime_source),
        ))
    }
}

impl JsLayoutSourceLoader for FsLayoutSourceLoader {
    fn load_runtime_source(
        &self,
        config: &Config,
        workspace: &hypreact_core::snapshot::WorkspaceSnapshot,
    ) -> Result<Option<PreparedLayout>, RuntimeError> {
        let Some(layout) = config.selected_layout(workspace) else {
            return Ok(None);
        };

        self.load_definition(config.global_stylesheet_path.as_deref(), layout)
            .map(Some)
            .map_err(|error| RuntimeError::Other { message: error.to_string() })
    }
}

impl JsLayoutSourceLoader for RuntimeProjectLayoutSourceLoader {
    fn load_runtime_source(
        &self,
        config: &Config,
        workspace: &hypreact_core::snapshot::WorkspaceSnapshot,
    ) -> Result<Option<PreparedLayout>, RuntimeError> {
        let Some(layout) = config.selected_layout(workspace) else {
            return Ok(None);
        };

        self.load_definition(config.global_stylesheet_path.as_deref(), layout)
            .map(Some)
            .map_err(|error| RuntimeError::Other { message: error.to_string() })
    }
}

pub fn loaded_layout_definition(
    layout: &LayoutDefinition,
    global_stylesheet_path: Option<&str>,
    module: String,
    runtime_graph: JavaScriptModuleGraph,
) -> PreparedLayout {
    let mut dependencies = Vec::new();
    if let Some((_, dependency)) = load_text_dependency(&module) {
        dependencies.push(dependency);
    }

    let global_stylesheet =
        global_stylesheet_path.and_then(|path| load_global_stylesheet_asset(layout, path));
    if let Some(stylesheet) = global_stylesheet.as_ref() {
        dependencies.push(NativeDependencySnapshot {
            path: stylesheet.path.clone(),
            content_hash: hash_source(&stylesheet.source),
        });
    }

    let layout_stylesheet = layout
        .stylesheet_path
        .as_ref()
        .and_then(|path| load_stylesheet_asset(layout, &module, path));
    if let Some(stylesheet) = layout_stylesheet.as_ref() {
        dependencies.push(NativeDependencySnapshot {
            path: stylesheet.path.clone(),
            content_hash: hash_source(&stylesheet.source),
        });
    }

    PreparedLayout {
        selected: SelectedLayout {
            name: layout.name.clone(),
            runtime: RuntimeKind::Js,
            directory: layout.directory.clone(),
            module: module.clone(),
        },
        runtime_payload: layout
            .runtime_cache_payload
            .clone()
            .unwrap_or_else(|| encode_runtime_graph_payload(&runtime_graph, &[])),
        stylesheets: PreparedStylesheets { global: global_stylesheet, layout: layout_stylesheet },
        dependencies,
    }
}

fn load_stylesheet_asset(
    layout: &LayoutDefinition,
    module_path: &str,
    path: &str,
) -> Option<PreparedStylesheet> {
    let stylesheet = load_stylesheet_asset_source(layout, module_path, path).or_else(|| {
        warn!(
            layout = %layout.name,
            stylesheet_path = %path,
            layout_directory = %layout.directory,
            module = %module_path,
            "failed to load layout stylesheet from any candidate path"
        );
        None
    });

    let Some(stylesheet) = stylesheet else {
        return None;
    };

    if stylesheet.source.trim().is_empty() {
        warn!(
            layout = %layout.name,
            stylesheet_path = %path,
            module = %module_path,
            "loaded layout stylesheet is empty"
        );
    } else {
        debug!(
            layout = %layout.name,
            stylesheet_path = %path,
            module = %module_path,
            bytes = stylesheet.source.len(),
            "loaded layout stylesheet source"
        );
    }

    Some(stylesheet)
}

fn load_global_stylesheet_asset(
    layout: &LayoutDefinition,
    path: &str,
) -> Option<PreparedStylesheet> {
    let Some((stylesheet, _)) = load_cached_stylesheet(path) else {
        warn!(
            layout = %layout.name,
            stylesheet_path = %path,
            "failed to load global stylesheet"
        );
        return None;
    };

    if stylesheet.source.trim().is_empty() {
        warn!(
            layout = %layout.name,
            stylesheet_path = %path,
            "loaded global stylesheet is empty"
        );
    } else {
        debug!(
            layout = %layout.name,
            stylesheet_path = %path,
            bytes = stylesheet.source.len(),
            "loaded global stylesheet source"
        );
    }

    Some(stylesheet)
}

fn load_stylesheet_asset_source(
    layout: &LayoutDefinition,
    module_path: &str,
    path: &str,
) -> Option<PreparedStylesheet> {
    let path_obj = std::path::Path::new(path);
    let mut candidates = Vec::new();

    candidates.push(path_obj.to_path_buf());

    if path_obj.is_relative() {
        let module_path_obj = std::path::Path::new(module_path);
        if let Some(module_dir) = module_path_obj.parent() {
            candidates.push(module_dir.join(path_obj));
            if let Some(file_name) = path_obj.file_name() {
                candidates.push(module_dir.join(file_name));
            }
        }

        let layout_dir = std::path::Path::new(&layout.directory);
        candidates.push(layout_dir.join(path_obj));
        if let Some(file_name) = path_obj.file_name() {
            candidates.push(layout_dir.join(file_name));
        }
    }

    candidates.sort();
    candidates.dedup();

    for candidate in candidates {
        if let Some((stylesheet, _)) = load_cached_stylesheet(&candidate) {
            debug!(candidate = %candidate.display(), "resolved stylesheet candidate path");
            return Some(stylesheet);
        }
    }

    None
}

fn hash_source(source: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn single_module_graph(module: String, source: String) -> JavaScriptModuleGraph {
    JavaScriptModuleGraph {
        entry: module.clone(),
        modules: vec![JavaScriptModule {
            specifier: module,
            source: normalize_runtime_module_source(&source),
            resolved_imports: Default::default(),
        }],
    }
}

fn normalize_runtime_module_source(source: &str) -> String {
    let trimmed = source.trim();
    if trimmed.contains("export default")
        || trimmed.contains("export {")
        || trimmed.contains("export function")
    {
        source.to_owned()
    } else {
        format!("export default ({trimmed});")
    }
}
