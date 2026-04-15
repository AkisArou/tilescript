use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use js_sys::{Array, Function, Promise, Reflect};
use serde_json::Value;
use hypreact_config::model::{Config, LayoutConfigError};
use hypreact_config::runtime::{
    EvaluatedSourceLayout, SourceBundle, SourceBundleConfigRuntime,
    SourceBundlePreparedLayoutRuntime, SourceBundleRuntimeBundle,
};
use hypreact_core::runtime::runtime_kind::RuntimeKind;
use hypreact_core::runtime::layout_context::{
    LayoutEvaluationContext, LayoutEvaluationDependencies,
};
use hypreact_core::runtime::prepared_layout::{
    PreparedLayout, PreparedStylesheet, PreparedStylesheets,
};
use hypreact_core::snapshot::{StateSnapshot, WorkspaceSnapshot};
use hypreact_runtime_js_core::{
    JavaScriptModule, JavaScriptModuleGraph, compile_source_bundle_to_module_graph,
    decode_config_value, decode_js_layout_value, decode_runtime_graph_payload,
    encode_runtime_graph_payload, validate_layout_selection,
};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Blob, BlobPropertyBag, Url};

#[wasm_bindgen(inline_js = r#"
export async function importModule(url) {
  return await import(url);
}

export function createTrackedLayoutContext(context) {
  const dependencies = {
    usesMonitorSize: false,
    usesMonitorScale: false,
    usesWindowCount: false,
    usesWindowOrder: false,
    usesWindowFocus: false,
    usesVisibleWindowIds: false,
    usesWorkspaceName: false,
    usesWorkspaceNames: false,
    usesSelectedLayoutName: false,
    usesLayoutAdjustments: false,
  };

  const trackedWindows = context.windows.map((window) => new Proxy(window, {
    get(target, prop, receiver) {
      if (prop === "focused") {
        dependencies.usesWindowFocus = true;
      } else if (typeof prop === "string" && prop !== "id") {
        dependencies.usesWindowOrder = true;
      }
      return Reflect.get(target, prop, receiver);
    },
  }));

  const windowsProxy = new Proxy(trackedWindows, {
    get(target, prop, receiver) {
      if (prop === "length") {
        dependencies.usesWindowCount = true;
        return Reflect.get(target, prop, receiver);
      }

      if (typeof prop === "string") {
        const index = Number(prop);
        if (!Number.isNaN(index)) {
          dependencies.usesWindowOrder = true;
        }
      }

      return Reflect.get(target, prop, receiver);
    },
  });

  const trackedWorkspace = new Proxy(context.workspace, {
    get(target, prop, receiver) {
      if (prop === "windowCount") {
        dependencies.usesWindowCount = true;
      } else if (prop === "name") {
        dependencies.usesWorkspaceName = true;
      } else if (prop === "workspaces") {
        dependencies.usesWorkspaceNames = true;
      }
      return Reflect.get(target, prop, receiver);
    },
  });

  const trackedMonitor = new Proxy(context.monitor, {
    get(target, prop, receiver) {
      if (prop === "width" || prop === "height") {
        dependencies.usesMonitorSize = true;
      } else if (prop === "scale") {
        dependencies.usesMonitorScale = true;
      }
      return Reflect.get(target, prop, receiver);
    },
  });

  const trackedState = context.state
    ? new Proxy(context.state, {
        get(target, prop, receiver) {
          if (prop === "focusedWindowId") {
            dependencies.usesWindowFocus = true;
          } else if (prop === "visibleWindowIds") {
            dependencies.usesVisibleWindowIds = true;
          } else if (prop === "selectedLayoutName") {
            dependencies.usesSelectedLayoutName = true;
          } else if (prop === "resizeState") {
            dependencies.usesLayoutAdjustments = true;
          } else if (prop === "workspaceNames") {
            dependencies.usesWorkspaceNames = true;
          }
          return Reflect.get(target, prop, receiver);
        },
      })
    : undefined;

  return {
    context: new Proxy(context, {
      get(target, prop, receiver) {
        if (prop === "windows") {
          return windowsProxy;
        }
        if (prop === "workspace") {
          return trackedWorkspace;
        }
        if (prop === "monitor") {
          return trackedMonitor;
        }
        if (prop === "state") {
          return trackedState;
        }
        return Reflect.get(target, prop, receiver);
      },
    }),
    dependencies,
  };
}

export function buildTrackedLayoutResult(layout, dependencies) {
  return { layout, dependencies };
}
"#)]
extern "C" {
    #[wasm_bindgen(catch, js_name = importModule)]
    fn import_module(url: &str) -> Result<Promise, JsValue>;

    #[wasm_bindgen(js_name = createTrackedLayoutContext)]
    fn create_tracked_layout_context(context: JsValue) -> JsValue;

    #[wasm_bindgen(js_name = buildTrackedLayoutResult)]
    fn build_tracked_layout_result(layout: JsValue, dependencies: JsValue) -> JsValue;
}

async fn evaluate_layout_module_graph(
    module_graph: &JavaScriptModuleGraph,
    context: &LayoutEvaluationContext,
) -> Result<LayoutEvaluationResult, String> {
    let raw_context = serde_wasm_bindgen::to_value(context).map_err(|error| error.to_string())?;
    let tracked = create_tracked_layout_context(raw_context);
    let tracked_context =
        Reflect::get(&tracked, &JsValue::from_str("context")).map_err(js_error_to_string)?;
    let dependencies =
        Reflect::get(&tracked, &JsValue::from_str("dependencies")).map_err(js_error_to_string)?;
    let layout = evaluate_module_export_function(module_graph, "default", tracked_context).await?;
    let result = build_tracked_layout_result(layout, dependencies);
    serde_wasm_bindgen::from_value(result).map_err(|error| error.to_string())
}

pub async fn load_config_from_source_bundle(
    root_dir: &Path,
    entry_path: &Path,
    sources: &BTreeMap<PathBuf, String>,
) -> Result<Config, String> {
    let graph = compile_source_bundle_to_module_graph(root_dir, entry_path, sources)
        .map_err(|error| error.to_string())?;
    let value = evaluate_module_export_value(&graph, "default").await?;
    let config_value: Value =
        serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())?;
    let mut config =
        decode_config_value(entry_path, &config_value).map_err(|error| error.to_string())?;
    config.global_stylesheet_path = sources
        .contains_key(&root_dir.join("index.css"))
        .then(|| root_dir.join("index.css").to_string_lossy().into_owned());
    config.layouts = discover_layout_definitions(root_dir, sources)?;
    validate_layout_selection(
        entry_path,
        config.default_layout.as_deref(),
        &config.layout_rules,
        &config.layouts,
    )
    .map_err(|error| error.to_string())?;
    Ok(config)
}

pub fn compile_module_graph_from_source_bundle(
    root_dir: &Path,
    entry_path: &Path,
    sources: &BTreeMap<PathBuf, String>,
) -> Result<JavaScriptModuleGraph, String> {
    compile_source_bundle_to_module_graph(root_dir, entry_path, sources)
        .map_err(|error| error.to_string())
}

fn js_error_to_string(error: JsValue) -> String {
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}

async fn evaluate_module_export_value(
    module_graph: &JavaScriptModuleGraph,
    export_name: &str,
) -> Result<JsValue, String> {
    let module_urls = build_module_urls(module_graph)?;
    let entry_url = module_urls
        .get(&module_graph.entry)
        .ok_or_else(|| format!("Missing module {}", module_graph.entry))?
        .clone();
    let promise = import_module(&entry_url).map_err(js_error_to_string)?;
    let namespace = JsFuture::from(promise).await.map_err(js_error_to_string)?;
    Reflect::get(&namespace, &JsValue::from_str(export_name)).map_err(js_error_to_string)
}

async fn evaluate_module_export_function(
    module_graph: &JavaScriptModuleGraph,
    export_name: &str,
    arg: JsValue,
) -> Result<JsValue, String> {
    let export = evaluate_module_export_value(module_graph, export_name).await?;
    let function = export.dyn_into::<Function>().map_err(|_| {
        format!("Module {} does not export callable {}", module_graph.entry, export_name)
    })?;
    function.call1(&JsValue::UNDEFINED, &arg).map_err(js_error_to_string)
}

fn build_module_urls(
    module_graph: &JavaScriptModuleGraph,
) -> Result<BTreeMap<String, String>, String> {
    let module_map = module_graph
        .modules
        .iter()
        .cloned()
        .map(|module| (module.specifier.clone(), module))
        .collect::<BTreeMap<_, _>>();
    let mut cache = BTreeMap::new();
    let mut object_urls = Vec::new();
    let mut visiting = Vec::new();

    let _ = module_url_for(
        &module_graph.entry,
        &module_map,
        &mut cache,
        &mut object_urls,
        &mut visiting,
    )?;
    Ok(cache)
}

fn module_url_for(
    specifier: &str,
    module_map: &BTreeMap<String, JavaScriptModule>,
    cache: &mut BTreeMap<String, String>,
    object_urls: &mut Vec<String>,
    visiting: &mut Vec<String>,
) -> Result<String, String> {
    if let Some(url) = cache.get(specifier) {
        return Ok(url.clone());
    }

    if visiting.iter().any(|entry| entry == specifier) {
        return Err(format!("Circular module graph dependency detected at {specifier}"));
    }

    let module = module_map.get(specifier).ok_or_else(|| format!("Missing module {specifier}"))?;
    visiting.push(specifier.to_string());
    let rewritten = rewrite_source(
        &module.source,
        &module.resolved_imports,
        module_map,
        cache,
        object_urls,
        visiting,
    )?;
    let url = create_module_url(&rewritten)?;
    object_urls.push(url.clone());
    cache.insert(specifier.to_string(), url.clone());
    visiting.pop();
    Ok(url)
}

fn rewrite_source(
    source: &str,
    resolved_imports: &BTreeMap<String, String>,
    module_map: &BTreeMap<String, JavaScriptModule>,
    cache: &mut BTreeMap<String, String>,
    object_urls: &mut Vec<String>,
    visiting: &mut Vec<String>,
) -> Result<String, String> {
    let mut output = String::with_capacity(source.len());
    let bytes = source.as_bytes();
    let mut cursor = 0usize;

    while cursor < bytes.len() {
        if starts_with_keyword(bytes, cursor, b"from") {
            let Some((next_cursor, replaced)) = rewrite_static_import(
                source,
                cursor + 4,
                resolved_imports,
                module_map,
                cache,
                object_urls,
                visiting,
            )?
            else {
                output.push(bytes[cursor] as char);
                cursor += 1;
                continue;
            };
            output.push_str("from ");
            output.push_str(&replaced);
            cursor = next_cursor;
            continue;
        }

        if starts_with_keyword(bytes, cursor, b"import") {
            if let Some((next_cursor, replaced)) = rewrite_dynamic_or_bare_import(
                source,
                cursor + 6,
                resolved_imports,
                module_map,
                cache,
                object_urls,
                visiting,
            )? {
                output.push_str("import");
                output.push_str(&replaced);
                cursor = next_cursor;
                continue;
            }
        }

        output.push(bytes[cursor] as char);
        cursor += 1;
    }

    Ok(output)
}

fn rewrite_static_import(
    source: &str,
    mut cursor: usize,
    resolved_imports: &BTreeMap<String, String>,
    module_map: &BTreeMap<String, JavaScriptModule>,
    cache: &mut BTreeMap<String, String>,
    object_urls: &mut Vec<String>,
    visiting: &mut Vec<String>,
) -> Result<Option<(usize, String)>, String> {
    let whitespace = consume_whitespace(source, cursor);
    cursor = whitespace;
    let Some((next_cursor, specifier, quote)) = consume_string_literal(source, cursor) else {
        return Ok(None);
    };
    let replacement = resolve_rewritten_specifier(
        specifier,
        resolved_imports,
        module_map,
        cache,
        object_urls,
        visiting,
    )?;
    Ok(Some((
        next_cursor,
        format!("{}{}{}", quote, replacement.unwrap_or_else(|| specifier.to_string()), quote),
    )))
}

fn rewrite_dynamic_or_bare_import(
    source: &str,
    mut cursor: usize,
    resolved_imports: &BTreeMap<String, String>,
    module_map: &BTreeMap<String, JavaScriptModule>,
    cache: &mut BTreeMap<String, String>,
    object_urls: &mut Vec<String>,
    visiting: &mut Vec<String>,
) -> Result<Option<(usize, String)>, String> {
    let whitespace = consume_whitespace(source, cursor);
    cursor = whitespace;
    let bytes = source.as_bytes();
    if bytes.get(cursor) == Some(&b'(') {
        let after_open = consume_whitespace(source, cursor + 1);
        let Some((next_cursor, specifier, quote)) = consume_string_literal(source, after_open)
        else {
            return Ok(None);
        };
        let close_cursor = consume_whitespace(source, next_cursor);
        if source.as_bytes().get(close_cursor) != Some(&b')') {
            return Ok(None);
        }
        let replacement = resolve_rewritten_specifier(
            specifier,
            resolved_imports,
            module_map,
            cache,
            object_urls,
            visiting,
        )?;
        return Ok(Some((
            close_cursor + 1,
            format!("({quote}{}{quote})", replacement.unwrap_or_else(|| specifier.to_string())),
        )));
    }

    let Some((next_cursor, specifier, quote)) = consume_string_literal(source, cursor) else {
        return Ok(None);
    };
    let replacement = resolve_rewritten_specifier(
        specifier,
        resolved_imports,
        module_map,
        cache,
        object_urls,
        visiting,
    )?;
    Ok(Some((
        next_cursor,
        format!(" {quote}{}{quote}", replacement.unwrap_or_else(|| specifier.to_string())),
    )))
}

fn resolve_rewritten_specifier(
    specifier: &str,
    resolved_imports: &BTreeMap<String, String>,
    module_map: &BTreeMap<String, JavaScriptModule>,
    cache: &mut BTreeMap<String, String>,
    object_urls: &mut Vec<String>,
    visiting: &mut Vec<String>,
) -> Result<Option<String>, String> {
    let resolved = resolved_imports
        .get(specifier)
        .map(String::as_str)
        .or_else(|| module_map.contains_key(specifier).then_some(specifier));

    match resolved {
        Some(target) => module_url_for(target, module_map, cache, object_urls, visiting).map(Some),
        None => Ok(None),
    }
}

fn starts_with_keyword(bytes: &[u8], cursor: usize, keyword: &[u8]) -> bool {
    bytes.get(cursor..cursor + keyword.len()) == Some(keyword)
}

fn consume_whitespace(source: &str, mut cursor: usize) -> usize {
    let bytes = source.as_bytes();
    while let Some(byte) = bytes.get(cursor) {
        if !byte.is_ascii_whitespace() {
            break;
        }
        cursor += 1;
    }
    cursor
}

fn consume_string_literal(source: &str, cursor: usize) -> Option<(usize, &str, char)> {
    let bytes = source.as_bytes();
    let quote = match bytes.get(cursor) {
        Some(b'\'') => '\'',
        Some(b'"') => '"',
        _ => return None,
    };
    let mut end = cursor + 1;
    while let Some(byte) = bytes.get(end) {
        if *byte == b'\\' {
            end += 2;
            continue;
        }
        if *byte == quote as u8 {
            return Some((end + 1, &source[cursor + 1..end], quote));
        }
        end += 1;
    }
    None
}

fn create_module_url(source: &str) -> Result<String, String> {
    let parts = Array::new();
    parts.push(&JsValue::from_str(source));
    let bag = BlobPropertyBag::new();
    bag.set_type("text/javascript;charset=utf-8");
    let blob = Blob::new_with_str_sequence_and_options(&parts, &bag).map_err(js_error_to_string)?;
    Url::create_object_url_with_blob(&blob).map_err(js_error_to_string)
}

fn discover_layout_definitions(
    root_dir: &Path,
    sources: &BTreeMap<PathBuf, String>,
) -> Result<Vec<hypreact_config::model::LayoutDefinition>, String> {
    let mut layout_entries = sources
        .keys()
        .filter_map(|path| discover_layout_entry(root_dir, sources, path))
        .collect::<Vec<_>>();
    layout_entries.sort_by(|left, right| left.name.cmp(&right.name));
    layout_entries.dedup_by(|left, right| left.name == right.name);

    layout_entries
        .into_iter()
        .map(|layout| {
            let runtime_graph =
                compile_source_bundle_to_module_graph(root_dir, &layout.entry_path, sources)?;
            Ok(hypreact_config::model::LayoutDefinition {
                name: layout.name,
                runtime: RuntimeKind::Js,
                directory: layout
                    .entry_path
                    .parent()
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                module: runtime_graph.entry.clone(),
                stylesheet_path: layout
                    .stylesheet_path
                    .map(|path| path.to_string_lossy().into_owned()),
                runtime_cache_payload: Some(encode_runtime_graph_payload(&runtime_graph)),
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiscoveredLayoutApp {
    name: String,
    entry_path: PathBuf,
    stylesheet_path: Option<PathBuf>,
}

fn discover_layout_entry(
    root_dir: &Path,
    sources: &BTreeMap<PathBuf, String>,
    path: &Path,
) -> Option<DiscoveredLayoutApp> {
    let relative = path.strip_prefix(root_dir).ok()?;
    let components = relative.iter().map(|segment| segment.to_str()).collect::<Option<Vec<_>>>()?;
    if components.len() != 3 || components[0] != "layouts" || components[2] == "index.css" {
        return None;
    }

    if !matches!(components[2], "index.ts" | "index.tsx" | "index.js" | "index.jsx") {
        return None;
    }

    let stylesheet_path = root_dir.join("layouts").join(components[1]).join("index.css");

    Some(DiscoveredLayoutApp {
        name: components[1].to_string(),
        entry_path: path.to_path_buf(),
        stylesheet_path: sources.contains_key(&stylesheet_path).then_some(stylesheet_path),
    })
}

#[derive(Debug, Clone, Copy, Default)]
pub struct JavaScriptBrowserRuntimeProvider;

impl JavaScriptBrowserRuntimeProvider {
    pub const fn new() -> Self {
        Self
    }

    pub fn build_source_bundle_runtime_bundle(
        &self,
    ) -> Result<SourceBundleRuntimeBundle, LayoutConfigError> {
        Ok(SourceBundleRuntimeBundle {
            config_runtime: Box::new(JavaScriptBrowserConfigRuntime),
            layout_runtime: Box::new(JavaScriptBrowserPreparedLayoutRuntime),
        })
    }
}

#[derive(Debug)]
pub struct JavaScriptBrowserConfigRuntime;

#[derive(Debug)]
pub struct JavaScriptBrowserPreparedLayoutRuntime;

impl SourceBundleConfigRuntime for JavaScriptBrowserConfigRuntime {
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

impl SourceBundlePreparedLayoutRuntime for JavaScriptBrowserPreparedLayoutRuntime {
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

            let runtime_graph =
                decode_runtime_graph_payload(layout.runtime_cache_payload.as_ref().ok_or_else(
                    || LayoutConfigError::DecodeAuthoredConfig {
                        path: root_dir.join(&layout.module),
                        message: format!(
                            "layout `{}` is missing runtime cache payload",
                            layout.name
                        ),
                    },
                )?)
                .map_err(|error| LayoutConfigError::DecodeAuthoredConfig {
                    path: root_dir.join(&layout.module),
                    message: error.to_string(),
                })?;

            Ok(Some(PreparedLayout {
                selected: config
                    .resolve_selected_layout(workspace)?
                    .expect("selected layout exists"),
                runtime_payload: encode_runtime_graph_payload(&runtime_graph),
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
            let runtime_graph =
                decode_runtime_graph_payload(&artifact.runtime_payload).map_err(|error| {
                    LayoutConfigError::DecodeAuthoredConfig {
                        path: PathBuf::from(&artifact.selected.module),
                        message: error.to_string(),
                    }
                })?;
            let value = evaluate_layout_module_graph(&runtime_graph, context)
                .await
                .map_err(|message| LayoutConfigError::EvaluateAuthoredConfig {
                    path: PathBuf::from(&artifact.selected.module),
                    message,
                })?;
            let layout = decode_js_layout_value(&value.layout).map_err(|message| {
                LayoutConfigError::DecodeAuthoredConfig {
                    path: PathBuf::from(&artifact.selected.module),
                    message,
                }
            })?;

            Ok(EvaluatedSourceLayout { layout, dependencies: value.dependencies })
        })
    }
}

fn load_stylesheet_asset(
    path: Option<&str>,
    root_dir: &Path,
    sources: &SourceBundle,
) -> Option<PreparedStylesheet> {
    let path = path?;
    let source_path = PathBuf::from(path);
    let resolved = if source_path.is_absolute() {
        source_path
    } else {
        root_dir.join(&source_path)
    };
    let source = sources.get(&resolved).cloned().unwrap_or_default();
    Some(PreparedStylesheet { path: path.to_string(), source })
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LayoutEvaluationResult {
    layout: Value,
    #[serde(default)]
    dependencies: LayoutEvaluationDependencies,
}
