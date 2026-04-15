use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use hypreact_config::model::{Config, LayoutConfigError, LayoutDefinition};
use hypreact_config::runtime::{
    EvaluatedSourceLayout, SourceBundle, SourceBundleConfigRuntime,
    SourceBundlePreparedLayoutRuntime, SourceBundleRuntimeBundle,
};
use hypreact_config::{config_decode::decode_config_value, layout_decode::decode_layout_value};
use hypreact_config::selection::validate_layout_selection;
use hypreact_core::runtime::layout_context::{
    LayoutEvaluationContext, LayoutEvaluationDependencies,
};
use hypreact_core::runtime::prepared_layout::{
    PreparedLayout, PreparedStylesheet, PreparedStylesheets,
};
use hypreact_core::runtime::runtime_kind::RuntimeKind;
use hypreact_core::snapshot::{StateSnapshot, WorkspaceSnapshot};
use hypreact_runtime_lua_core::LUA_SDK_SOURCE;
use serde_json::Value;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen(module = "/src/lua_runtime_bundle.js")]
extern "C" {
    #[wasm_bindgen(catch, js_name = evaluateLuaConfig)]
    fn evaluate_lua_config_js(
        source: &str,
        chunk_name: &str,
        sdk_source: &str,
    ) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_name = evaluateLuaLayout)]
    fn evaluate_lua_layout_js(
        source: &str,
        chunk_name: &str,
        sdk_source: &str,
        context: JsValue,
    ) -> Result<js_sys::Promise, JsValue>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LuaBrowserRuntimeProvider;

impl LuaBrowserRuntimeProvider {
    pub const fn new() -> Self {
        Self
    }

    pub fn build_source_bundle_runtime_bundle(
        &self,
    ) -> Result<SourceBundleRuntimeBundle, LayoutConfigError> {
        Ok(SourceBundleRuntimeBundle {
            config_runtime: Box::new(LuaBrowserConfigRuntime),
            layout_runtime: Box::new(LuaBrowserPreparedLayoutRuntime),
        })
    }
}

#[derive(Debug)]
pub struct LuaBrowserConfigRuntime;

#[derive(Debug)]
pub struct LuaBrowserPreparedLayoutRuntime;

impl SourceBundleConfigRuntime for LuaBrowserConfigRuntime {
    fn load_config<'a>(
        &'a self,
        root_dir: &'a Path,
        entry_path: &'a Path,
        sources: &'a SourceBundle,
    ) -> Pin<Box<dyn Future<Output = Result<Config, LayoutConfigError>> + 'a>> {
        Box::pin(async move {
            load_config_from_source_bundle(root_dir, entry_path, sources)
                .await
                .map_err(|message| LayoutConfigError::EvaluateAuthoredConfig {
                    path: entry_path.to_path_buf(),
                    message,
                })
        })
    }
}

impl SourceBundlePreparedLayoutRuntime for LuaBrowserPreparedLayoutRuntime {
    fn prepare_layout<'a>(
        &'a self,
        root_dir: &'a Path,
        sources: &'a SourceBundle,
        config: &'a Config,
        workspace: &'a WorkspaceSnapshot,
    ) -> Pin<Box<dyn Future<Output = Result<Option<PreparedLayout>, LayoutConfigError>> + 'a>> {
        Box::pin(async move {
            let Some(layout) = config.selected_layout(workspace) else {
                return Ok(None);
            };

            let source_path = root_dir.join(&layout.module);
            let source = sources.get(&source_path).cloned().ok_or_else(|| {
                LayoutConfigError::EvaluateAuthoredConfig {
                    path: source_path.clone(),
                    message: format!("missing Lua source for `{}`", layout.name),
                }
            })?;

            Ok(Some(PreparedLayout {
                selected: config
                    .resolve_selected_layout(workspace)?
                    .expect("selected layout exists"),
                runtime_payload: serde_json::json!({ "source": source }),
                stylesheets: PreparedStylesheets {
                    global: load_stylesheet_asset(
                        config.global_stylesheet_path.as_deref(),
                        root_dir,
                        sources,
                    ),
                    layout: load_stylesheet_asset(
                        layout.stylesheet_path.as_deref(),
                        root_dir,
                        sources,
                    ),
                },
            }))
        })
    }

    fn build_context(
        &self,
        state: &StateSnapshot,
        workspace: &WorkspaceSnapshot,
        artifact: Option<&PreparedLayout>,
    ) -> LayoutEvaluationContext {
        state.layout_context(workspace, artifact.map(|artifact| artifact.selected.clone()))
    }

    fn evaluate_layout<'a>(
        &'a self,
        _root_dir: &'a Path,
        _sources: &'a SourceBundle,
        artifact: &'a PreparedLayout,
        context: &'a LayoutEvaluationContext,
    ) -> Pin<Box<dyn Future<Output = Result<EvaluatedSourceLayout, LayoutConfigError>> + 'a>> {
        Box::pin(async move {
            let source = artifact
                .runtime_payload
                .get("source")
                .and_then(Value::as_str)
                .ok_or_else(|| LayoutConfigError::DecodeAuthoredConfig {
                    path: PathBuf::from(&artifact.selected.module),
                    message: format!(
                        "lua runtime payload for `{}` is missing source",
                        artifact.selected.name
                    ),
                })?;

            let result = evaluate_layout(source, &artifact.selected.module, context)
                .await
                .map_err(|message| LayoutConfigError::EvaluateAuthoredConfig {
                    path: PathBuf::from(&artifact.selected.module),
                    message,
                })?;
            let layout = decode_layout_value(&result.layout).map_err(|message| {
                LayoutConfigError::DecodeAuthoredConfig {
                    path: PathBuf::from(&artifact.selected.module),
                    message,
                }
            })?;

            Ok(EvaluatedSourceLayout {
                layout,
                dependencies: result.dependencies,
            })
        })
    }
}

pub async fn load_config_from_source_bundle(
    root_dir: &Path,
    entry_path: &Path,
    sources: &BTreeMap<PathBuf, String>,
) -> Result<Config, String> {
    let source = sources
        .get(entry_path)
        .ok_or_else(|| format!("missing Lua config source `{}`", entry_path.display()))?;
    let value = evaluate_config(source, &entry_path.to_string_lossy()).await?;
    let mut config = decode_config_value(entry_path, &value).map_err(|error| error.to_string())?;
    config.global_stylesheet_path = sources
        .contains_key(&root_dir.join("index.css"))
        .then(|| root_dir.join("index.css").to_string_lossy().into_owned());
    config.layouts = discover_layout_definitions(root_dir, sources);
    validate_layout_selection(
        entry_path,
        config.default_layout.as_deref(),
        &config.layout_rules,
        &config.layouts,
    )
    .map_err(|error| error.to_string())?;
    Ok(config)
}

async fn evaluate_config(source: &str, chunk_name: &str) -> Result<Value, String> {
    let promise = evaluate_lua_config_js(source, chunk_name, LUA_SDK_SOURCE).map_err(js_error_to_string)?;
    let value = JsFuture::from(promise).await.map_err(js_error_to_string)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

async fn evaluate_layout(
    source: &str,
    chunk_name: &str,
    context: &LayoutEvaluationContext,
) -> Result<LayoutEvaluationResult, String> {
    let context_value = serde_wasm_bindgen::to_value(context).map_err(|error| error.to_string())?;
    let promise =
        evaluate_lua_layout_js(source, chunk_name, LUA_SDK_SOURCE, context_value).map_err(js_error_to_string)?;
    let value = JsFuture::from(promise).await.map_err(js_error_to_string)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

fn discover_layout_definitions(
    root_dir: &Path,
    sources: &BTreeMap<PathBuf, String>,
) -> Vec<LayoutDefinition> {
    let layouts_root = root_dir.join("layouts");
    let mut layouts = sources
        .keys()
        .filter_map(|path| {
            let relative = path.strip_prefix(root_dir).ok()?;
            let components = relative
                .iter()
                .map(|segment| segment.to_str())
                .collect::<Option<Vec<_>>>()?;
            if components.len() != 3
                || components[0] != "layouts"
                || components[2] != "index.lua"
            {
                return None;
            }

            let stylesheet_path = layouts_root.join(components[1]).join("index.css");
            Some(LayoutDefinition {
                name: components[1].to_string(),
                runtime: RuntimeKind::Lua,
                directory: root_dir
                    .join("layouts")
                    .join(components[1])
                    .to_string_lossy()
                    .into_owned(),
                module: path.strip_prefix(root_dir).ok()?.to_string_lossy().into_owned(),
                stylesheet_path: sources
                    .contains_key(&stylesheet_path)
                    .then(|| stylesheet_path.strip_prefix(root_dir).unwrap().to_string_lossy().into_owned()),
                runtime_cache_payload: None,
            })
        })
        .collect::<Vec<_>>();
    layouts.sort_by(|left, right| left.name.cmp(&right.name));
    layouts.dedup_by(|left, right| left.name == right.name);
    layouts
}

fn load_stylesheet_asset(
    path: Option<&str>,
    root_dir: &Path,
    sources: &SourceBundle,
) -> Option<PreparedStylesheet> {
    let path = path?;
    let source_path = root_dir.join(path);
    let source = sources.get(&source_path).cloned().unwrap_or_default();
    Some(PreparedStylesheet {
        path: path.to_string(),
        source,
    })
}

fn js_error_to_string(error: JsValue) -> String {
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LayoutEvaluationResult {
    layout: Value,
    #[serde(default)]
    dependencies: LayoutEvaluationDependencies,
}
