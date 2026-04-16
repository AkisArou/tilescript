use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rquickjs::{
    Context as JsContext, Ctx, Function, Module, Object, Persistent, Runtime as JsRuntime,
    String as JsString, Value,
    loader::{Loader, Resolver},
};

use hypreact_runtime_js_core::JavaScriptModuleGraph;

pub struct QuickJsExecutionCache {
    entries: HashMap<String, QuickJsExecutionCacheEntry>,
    session: Option<QuickJsRuntimeSession>,
    bytecode_store: Option<QuickJsBytecodeStore>,
}

#[derive(Debug, Clone)]
struct QuickJsExecutionCacheEntry {
    graph: JavaScriptModuleGraph,
    bytecode_graph: Option<QuickJsBytecodeGraph>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QuickJsBytecodeGraph {
    modules: BTreeMap<String, Vec<u8>>,
}

#[derive(Debug, Clone)]
struct QuickJsBytecodeStore {
    root: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PersistedBytecodeManifest {
    schema_version: String,
    engine_version: String,
    graph_key: String,
    entry: String,
    modules: Vec<PersistedBytecodeModule>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PersistedBytecodeModule {
    specifier: String,
    file_name: String,
}

const QUICKJS_BYTECODE_SCHEMA_VERSION: &str = "hypreact-quickjs-bytecode-v1";
const QUICKJS_ENGINE_VERSION: &str = "rquickjs-core-0.11.0";

impl std::fmt::Debug for QuickJsExecutionCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuickJsExecutionCache")
            .field("entry_count", &self.entries.len())
            .field("has_session", &self.session.is_some())
            .field("has_bytecode_store", &self.bytecode_store.is_some())
            .finish()
    }
}

impl QuickJsExecutionCache {
    pub fn new(bytecode_root: Option<PathBuf>) -> Self {
        Self {
            entries: HashMap::new(),
            session: None,
            bytecode_store: bytecode_root.map(QuickJsBytecodeStore::new),
        }
    }

    pub fn call_entry_export_with_json_arg(
        &mut self,
        key: &str,
        graph: &JavaScriptModuleGraph,
        module_name: &str,
        export_name: &str,
        arg: &serde_json::Value,
    ) -> Result<Option<serde_json::Value>, ModuleGraphRuntimeError> {
        {
            let entry = self.entries.entry(key.to_string()).or_insert_with(|| {
                QuickJsExecutionCacheEntry { graph: graph.clone(), bytecode_graph: None }
            });

            if entry.bytecode_graph.is_none() {
                entry.bytecode_graph = Some(load_or_compile_graph_bytecodes(
                    self.bytecode_store.as_ref(),
                    key,
                    &entry.graph,
                    module_name,
                    export_name,
                )?);
            }

            let compatible = self
                .session
                .as_ref()
                .map(|session| {
                    let entry = self.entries.get(key).expect("cached js graph exists");
                    session.is_compatible_with(&entry.graph, entry.bytecode_graph.as_ref())
                })
                .transpose()?;
            if compatible == Some(false) {
                self.session = None;
            }
        }

        let mut session = match self.session.take() {
            Some(session) => session,
            None => QuickJsRuntimeSession::new()?,
        };

        let result = {
            let entry = self.entries.get(key).expect("cached js graph exists");
            session.call_entry_export_with_json_arg(
                key,
                &entry.graph,
                entry.bytecode_graph.as_ref(),
                module_name,
                export_name,
                arg,
            )
        };

        self.session = Some(session);
        result
    }

    pub fn reset(&mut self) {
        self.entries.clear();
        self.session = None;
    }
}

impl QuickJsBytecodeStore {
    fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn load(
        &self,
        graph_key: &str,
        graph: &JavaScriptModuleGraph,
    ) -> Result<Option<QuickJsBytecodeGraph>, ModuleGraphRuntimeError> {
        let graph_dir = self.root.join(graph_key);
        let manifest_path = graph_dir.join("manifest.json");
        let manifest_text = match fs::read_to_string(&manifest_path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(ModuleGraphRuntimeError::JavaScript {
                    message: format!(
                        "failed to read quickjs bytecode manifest `{}`: {error}",
                        manifest_path.display()
                    ),
                });
            }
        };
        let manifest: PersistedBytecodeManifest = serde_json::from_str(&manifest_text).map_err(|error| {
            ModuleGraphRuntimeError::JavaScript {
                message: format!(
                    "failed to parse quickjs bytecode manifest `{}`: {error}",
                    manifest_path.display()
                ),
            }
        })?;

        if manifest.schema_version != QUICKJS_BYTECODE_SCHEMA_VERSION
            || manifest.engine_version != QUICKJS_ENGINE_VERSION
            || manifest.graph_key != graph_key
            || manifest.entry != graph.entry
            || manifest.modules.len() != graph.modules.len()
        {
            return Ok(None);
        }

        let mut modules = BTreeMap::new();
        for module in &manifest.modules {
            if !graph.modules.iter().any(|candidate| candidate.specifier == module.specifier) {
                return Ok(None);
            }
            let bytecode = match fs::read(graph_dir.join(&module.file_name)) {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => {
                    return Err(ModuleGraphRuntimeError::JavaScript {
                        message: format!(
                            "failed to read quickjs bytecode module `{}`: {error}",
                            module.specifier
                        ),
                    });
                }
            };
            modules.insert(module.specifier.clone(), bytecode);
        }

        Ok(Some(QuickJsBytecodeGraph { modules }))
    }

    fn save(
        &self,
        graph_key: &str,
        graph: &JavaScriptModuleGraph,
        bytecode_graph: &QuickJsBytecodeGraph,
    ) -> Result<(), ModuleGraphRuntimeError> {
        let graph_dir = self.root.join(graph_key);
        fs::create_dir_all(&graph_dir).map_err(|error| ModuleGraphRuntimeError::JavaScript {
            message: format!(
                "failed to create quickjs bytecode directory `{}`: {error}",
                graph_dir.display()
            ),
        })?;

        let modules = graph
            .modules
            .iter()
            .enumerate()
            .map(|(index, module)| {
                let file_name = format!("module-{index:03}.qjsc");
                let bytecode = bytecode_graph.modules.get(&module.specifier).ok_or_else(|| {
                    ModuleGraphRuntimeError::JavaScript {
                        message: format!(
                            "missing compiled quickjs bytecode for `{}`",
                            module.specifier
                        ),
                    }
                })?;
                fs::write(graph_dir.join(&file_name), bytecode).map_err(|error| {
                    ModuleGraphRuntimeError::JavaScript {
                        message: format!(
                            "failed to write quickjs bytecode for `{}`: {error}",
                            module.specifier
                        ),
                    }
                })?;

                Ok::<PersistedBytecodeModule, ModuleGraphRuntimeError>(PersistedBytecodeModule {
                    specifier: module.specifier.clone(),
                    file_name,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let manifest = PersistedBytecodeManifest {
            schema_version: QUICKJS_BYTECODE_SCHEMA_VERSION.into(),
            engine_version: QUICKJS_ENGINE_VERSION.into(),
            graph_key: graph_key.into(),
            entry: graph.entry.clone(),
            modules,
        };
        let manifest_path = graph_dir.join("manifest.json");
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).map_err(|error| ModuleGraphRuntimeError::JavaScript {
                message: format!(
                    "failed to serialize quickjs bytecode manifest `{}`: {error}",
                    manifest_path.display()
                ),
            })?,
        )
        .map_err(|error| ModuleGraphRuntimeError::JavaScript {
            message: format!(
                "failed to write quickjs bytecode manifest `{}`: {error}",
                manifest_path.display()
            ),
        })?;

        Ok(())
    }
}

fn load_or_compile_graph_bytecodes(
    store: Option<&QuickJsBytecodeStore>,
    graph_key: &str,
    graph: &JavaScriptModuleGraph,
    module_name: &str,
    export_name: &str,
) -> Result<QuickJsBytecodeGraph, ModuleGraphRuntimeError> {
    if let Some(store) = store
        && let Some(bytecode_graph) = store.load(graph_key, graph)?
    {
        return Ok(bytecode_graph);
    }

    let bytecode_graph = compile_graph_bytecodes(graph, module_name, export_name)?;
    if let Some(store) = store {
        store.save(graph_key, graph, &bytecode_graph)?;
    }
    Ok(bytecode_graph)
}

struct QuickJsRuntimeSession {
    context: JsContext,
    registry: Arc<Mutex<SessionModuleRegistry>>,
    functions: HashMap<String, Persistent<Function<'static>>>,
    _runtime: JsRuntime,
}

impl std::fmt::Debug for QuickJsRuntimeSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let registry = self.registry.lock().ok();
        let module_count = registry.as_ref().map(|registry| registry.modules.len()).unwrap_or(0);
        f.debug_struct("QuickJsRuntimeSession")
            .field("module_count", &module_count)
            .field("function_count", &self.functions.len())
            .finish()
    }
}

impl QuickJsRuntimeSession {
    fn new() -> Result<Self, ModuleGraphRuntimeError> {
        let runtime = JsRuntime::new()
            .map_err(|error| ModuleGraphRuntimeError::JavaScript { message: error.to_string() })?;
        let registry = Arc::new(Mutex::new(SessionModuleRegistry::default()));
        runtime.set_loader(
            SessionResolver::new(registry.clone()),
            SessionLoader::new(registry.clone()),
        );
        let context = JsContext::full(&runtime)
            .map_err(|error| ModuleGraphRuntimeError::JavaScript { message: error.to_string() })?;

        Ok(Self { context, registry, functions: HashMap::new(), _runtime: runtime })
    }

    fn is_compatible_with(
        &self,
        graph: &JavaScriptModuleGraph,
        bytecode_graph: Option<&QuickJsBytecodeGraph>,
    ) -> Result<bool, ModuleGraphRuntimeError> {
        let registry = self.registry.lock().map_err(|_| ModuleGraphRuntimeError::JavaScript {
            message: "js session registry mutex is poisoned".into(),
        })?;
        Ok(registry.is_compatible_with(graph, bytecode_graph))
    }

    fn call_entry_export_with_json_arg(
        &mut self,
        key: &str,
        graph: &JavaScriptModuleGraph,
        bytecode_graph: Option<&QuickJsBytecodeGraph>,
        module_name: &str,
        export_name: &str,
        arg: &serde_json::Value,
    ) -> Result<Option<serde_json::Value>, ModuleGraphRuntimeError> {
        self.register_graph(graph, bytecode_graph)?;

        let function_key = entry_function_key(key, module_name, export_name);
        if !self.functions.contains_key(&function_key) {
            let persistent = self.context.with(|ctx| {
                let namespace = Module::import(&ctx, graph.entry.as_str())
                    .map_err(|error| ModuleGraphRuntimeError::JavaScript {
                        message: format_js_error(ctx.clone(), error),
                    })?
                    .finish::<Object>()
                    .map_err(|error| ModuleGraphRuntimeError::JavaScript {
                        message: format_js_error(ctx.clone(), error),
                    })?;
                let function = namespace_function(ctx.clone(), &namespace, module_name, export_name)?;
                Ok::<Persistent<Function<'static>>, ModuleGraphRuntimeError>(Persistent::save(
                    &ctx, function,
                ))
            })?;
            self.functions.insert(function_key.clone(), persistent);
        }

        let function = self
            .functions
            .get(&function_key)
            .cloned()
            .expect("cached js function exists");
        self.context.with(|ctx| {
            let function = function.restore(&ctx).map_err(|error| {
                ModuleGraphRuntimeError::JavaScript { message: error.to_string() }
            })?;
            let arg_source =
                format!("JSON.parse({})", serde_json::to_string(&arg.to_string()).unwrap());
            let arg: Value = ctx.eval(arg_source).map_err(|error| {
                ModuleGraphRuntimeError::JavaScript { message: format_js_error(ctx.clone(), error) }
            })?;
            let value: Value = function.call((arg,)).map_err(|error| {
                ModuleGraphRuntimeError::JavaScript { message: format_js_error(ctx.clone(), error) }
            })?;
            js_value_to_json(ctx, value)
                .map_err(|message| ModuleGraphRuntimeError::JavaScript { message })
        })
    }

    fn register_graph(
        &mut self,
        graph: &JavaScriptModuleGraph,
        bytecode_graph: Option<&QuickJsBytecodeGraph>,
    ) -> Result<(), ModuleGraphRuntimeError> {
        self.registry.lock().map_err(|_| ModuleGraphRuntimeError::JavaScript {
            message: "js session registry mutex is poisoned".into(),
        })?
        .register_graph(graph, bytecode_graph)
    }
}

impl Drop for QuickJsRuntimeSession {
    fn drop(&mut self) {
        self.functions.clear();
    }
}

fn entry_function_key(key: &str, module_name: &str, export_name: &str) -> String {
    format!("{key}|{module_name}|{export_name}")
}

#[derive(Debug, Default)]
struct SessionModuleRegistry {
    modules: BTreeMap<String, SessionModuleRecord>,
    imports: BTreeMap<String, BTreeMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionModuleRecord {
    source: String,
    bytecode: Option<Vec<u8>>,
}

impl SessionModuleRegistry {
    fn is_compatible_with(
        &self,
        graph: &JavaScriptModuleGraph,
        bytecode_graph: Option<&QuickJsBytecodeGraph>,
    ) -> bool {
        graph.modules.iter().all(|module| {
            let expected = SessionModuleRecord {
                source: module.source.clone(),
                bytecode: bytecode_graph.and_then(|graph| graph.modules.get(&module.specifier).cloned()),
            };
            self.modules.get(&module.specifier).map_or(true, |current| current == &expected)
                && self
                    .imports
                    .get(&module.specifier)
                    .map_or(true, |current| current == &module.resolved_imports)
        })
    }

    fn register_graph(
        &mut self,
        graph: &JavaScriptModuleGraph,
        bytecode_graph: Option<&QuickJsBytecodeGraph>,
    ) -> Result<(), ModuleGraphRuntimeError> {
        for module in &graph.modules {
            let record = SessionModuleRecord {
                source: module.source.clone(),
                bytecode: bytecode_graph.and_then(|graph| graph.modules.get(&module.specifier).cloned()),
            };
            if let Some(current) = self.modules.get(&module.specifier) {
                if current != &record {
                    return Err(ModuleGraphRuntimeError::JavaScript {
                        message: format!(
                            "js runtime session saw conflicting module contents for `{}`",
                            module.specifier
                        ),
                    });
                }
            }
            self.modules.insert(module.specifier.clone(), record);

            if let Some(current) = self.imports.get(&module.specifier) {
                if current != &module.resolved_imports {
                    return Err(ModuleGraphRuntimeError::JavaScript {
                        message: format!(
                            "js runtime session saw conflicting import graph for `{}`",
                            module.specifier
                        ),
                    });
                }
            }
            self.imports.insert(module.specifier.clone(), module.resolved_imports.clone());
        }

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ModuleGraphRuntimeError {
    #[error("javascript evaluation failed: {message}")]
    JavaScript { message: String },
    #[error("module `{name}` did not provide `{export}` export")]
    MissingExport { name: String, export: String },
    #[error("module `{name}` export `{export}` is not callable")]
    NonCallableExport { name: String, export: String },
}

pub fn evaluate_entry_export_to_json(
    graph: &JavaScriptModuleGraph,
    module_name: &str,
    export_name: &str,
) -> Result<Option<serde_json::Value>, ModuleGraphRuntimeError> {
    with_entry_namespace(graph, None, |ctx, namespace| {
        let value = namespace_export(ctx.clone(), &namespace, module_name, export_name)?;
        js_value_to_json(ctx, value)
            .map_err(|message| ModuleGraphRuntimeError::JavaScript { message })
    })
}

pub fn call_entry_export_with_json_arg(
    graph: &JavaScriptModuleGraph,
    module_name: &str,
    export_name: &str,
    arg: &serde_json::Value,
) -> Result<Option<serde_json::Value>, ModuleGraphRuntimeError> {
    with_entry_namespace(graph, None, |ctx, namespace| {
        let function = namespace_function(ctx.clone(), &namespace, module_name, export_name)?;
        let arg_source =
            format!("JSON.parse({})", serde_json::to_string(&arg.to_string()).unwrap());
        let arg: Value = ctx.eval(arg_source).map_err(|error| {
            ModuleGraphRuntimeError::JavaScript { message: format_js_error(ctx.clone(), error) }
        })?;
        let value: Value = function.call((arg,)).map_err(|error| {
            ModuleGraphRuntimeError::JavaScript { message: format_js_error(ctx.clone(), error) }
        })?;
        js_value_to_json(ctx, value)
            .map_err(|message| ModuleGraphRuntimeError::JavaScript { message })
    })
}

pub fn module_graph_execution_key(graph: &JavaScriptModuleGraph) -> String {
    let mut source = String::new();
    source.push_str(&graph.entry);
    for module in &graph.modules {
        source.push('|');
        source.push_str(&module.specifier);
        source.push('|');
        source.push_str(&module.source);
    }

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn with_entry_namespace<T, F>(
    graph: &JavaScriptModuleGraph,
    bytecode_graph: Option<&QuickJsBytecodeGraph>,
    f: F,
) -> Result<T, ModuleGraphRuntimeError>
where
    F: for<'js> FnOnce(Ctx<'js>, Object<'js>) -> Result<T, ModuleGraphRuntimeError>,
{
    let runtime = JsRuntime::new()
        .map_err(|error| ModuleGraphRuntimeError::JavaScript { message: error.to_string() })?;
    match bytecode_graph {
        Some(bytecode_graph) => runtime.set_loader(
            GraphResolver::new(graph),
            (BytecodeLoader::new(bytecode_graph), GraphLoader::new(graph)),
        ),
        None => runtime.set_loader(GraphResolver::new(graph), GraphLoader::new(graph)),
    }
    let context = JsContext::full(&runtime)
        .map_err(|error| ModuleGraphRuntimeError::JavaScript { message: error.to_string() })?;

    context.with(|ctx| {
        let namespace = Module::import(&ctx, graph.entry.as_str())
            .map_err(|error| ModuleGraphRuntimeError::JavaScript {
                message: format_js_error(ctx.clone(), error),
            })?
            .finish::<Object>()
            .map_err(|error| ModuleGraphRuntimeError::JavaScript {
                message: format_js_error(ctx.clone(), error),
            })?;

        f(ctx, namespace)
    })
}

fn compile_graph_bytecodes(
    graph: &JavaScriptModuleGraph,
    module_name: &str,
    export_name: &str,
) -> Result<QuickJsBytecodeGraph, ModuleGraphRuntimeError> {
    let runtime = JsRuntime::new()
        .map_err(|error| ModuleGraphRuntimeError::JavaScript { message: error.to_string() })?;
    let compile = rquickjs::loader::Compile::new();
    runtime.set_loader(
        compile.resolver(GraphResolver::new(graph)),
        compile.loader(GraphLoader::new(graph)),
    );
    let context = JsContext::full(&runtime)
        .map_err(|error| ModuleGraphRuntimeError::JavaScript { message: error.to_string() })?;

    context.with(|ctx| {
        let namespace = Module::import(&ctx, graph.entry.as_str())
            .map_err(|error| ModuleGraphRuntimeError::JavaScript {
                message: format_js_error(ctx.clone(), error),
            })?
            .finish::<Object>()
            .map_err(|error| ModuleGraphRuntimeError::JavaScript {
                message: format_js_error(ctx.clone(), error),
            })?;
        let _ = namespace_function(ctx, &namespace, module_name, export_name)?;

        Ok(QuickJsBytecodeGraph {
            modules: compile
                .bytecodes()
                .into_iter()
                .map(|(name, bytes)| (name.to_string(), bytes.to_vec()))
                .collect(),
        })
    })
}

fn namespace_export<'js>(
    ctx: Ctx<'js>,
    namespace: &Object<'js>,
    module_name: &str,
    export_name: &str,
) -> Result<Value<'js>, ModuleGraphRuntimeError> {
    namespace
        .contains_key(export_name)
        .map_err(|error| ModuleGraphRuntimeError::JavaScript {
            message: format_js_error(ctx.clone(), error),
        })?
        .then(|| namespace.get(export_name))
        .transpose()
        .map_err(|error| ModuleGraphRuntimeError::JavaScript {
            message: format_js_error(ctx.clone(), error),
        })?
        .ok_or_else(|| ModuleGraphRuntimeError::MissingExport {
            name: module_name.to_owned(),
            export: export_name.to_owned(),
        })
}

fn namespace_function<'js>(
    ctx: Ctx<'js>,
    namespace: &Object<'js>,
    module_name: &str,
    export_name: &str,
) -> Result<Function<'js>, ModuleGraphRuntimeError> {
    namespace_export(ctx.clone(), namespace, module_name, export_name)?.into_function().ok_or_else(
        || ModuleGraphRuntimeError::NonCallableExport {
            name: module_name.to_owned(),
            export: export_name.to_owned(),
        },
    )
}

fn js_value_to_json<'js>(
    ctx: Ctx<'js>,
    value: Value<'js>,
) -> Result<Option<serde_json::Value>, String> {
    let globals = ctx.globals();
    globals.set("__spiders_tmp", value).map_err(|error| format_js_error(ctx.clone(), error))?;

    let json_result = (|| {
        let json_text: Value = ctx
            .eval("globalThis.__spiders_tmp === undefined ? undefined : JSON.stringify(globalThis.__spiders_tmp)")
            .map_err(|error| format_js_error(ctx.clone(), error))?;

        if json_text.is_undefined() {
            return Ok(None);
        }

        let json_text = json_text
            .into_string()
            .ok_or_else(|| "JSON.stringify returned a non-string result".to_owned())?;
        let json_text = json_text.to_string().map_err(|error| error.to_string())?;
        let json = serde_json::from_str(&json_text).map_err(|error| error.to_string())?;
        Ok(Some(json))
    })();

    let _ = globals.remove("__spiders_tmp");
    json_result
}

pub fn format_js_error(ctx: Ctx<'_>, error: rquickjs::Error) -> String {
    if error.is_exception() {
        let globals = ctx.globals();
        let caught = ctx.catch();
        if globals.set("__spiders_error", caught).is_ok() {
            let rendered = ctx
                .eval::<JsString, _>(
                    "(() => { const error = globalThis.__spiders_error; return error && error.stack ? String(error.stack) : String(error); })()",
                )
                .and_then(|text| text.to_string())
                .ok();
            let _ = globals.remove("__spiders_error");
            if let Some(rendered) = rendered {
                return rendered;
            }
        }
    }

    error.to_string()
}

#[derive(Debug, Clone)]
struct GraphResolver {
    imports: BTreeMap<String, BTreeMap<String, String>>,
    known_modules: BTreeMap<String, ()>,
}

impl GraphResolver {
    fn new(graph: &JavaScriptModuleGraph) -> Self {
        Self {
            imports: graph
                .modules
                .iter()
                .map(|module| (module.specifier.clone(), module.resolved_imports.clone()))
                .collect(),
            known_modules: graph
                .modules
                .iter()
                .map(|module| (module.specifier.clone(), ()))
                .collect(),
        }
    }
}

impl Resolver for GraphResolver {
    fn resolve<'js>(
        &mut self,
        _ctx: &Ctx<'js>,
        base: &str,
        name: &str,
    ) -> rquickjs::Result<String> {
        if let Some(resolved) = self.imports.get(base).and_then(|imports| imports.get(name)) {
            return Ok(resolved.clone());
        }
        if self.known_modules.contains_key(name) {
            return Ok(name.to_owned());
        }
        Err(rquickjs::Error::new_resolving(base, name))
    }
}

#[derive(Debug, Clone)]
struct GraphLoader {
    modules: BTreeMap<String, String>,
}

impl GraphLoader {
    fn new(graph: &JavaScriptModuleGraph) -> Self {
        Self {
            modules: graph
                .modules
                .iter()
                .map(|module| (module.specifier.clone(), module.source.clone()))
                .collect(),
        }
    }
}

impl Loader for GraphLoader {
    fn load<'js>(&mut self, ctx: &Ctx<'js>, name: &str) -> rquickjs::Result<Module<'js>> {
        let source =
            self.modules.get(name).ok_or_else(|| rquickjs::Error::new_loading(name))?.clone();
        Module::declare(ctx.clone(), name, source)
    }
}

#[derive(Debug, Clone)]
struct BytecodeLoader {
    modules: BTreeMap<String, Vec<u8>>,
}

impl BytecodeLoader {
    fn new(graph: &QuickJsBytecodeGraph) -> Self {
        Self { modules: graph.modules.clone() }
    }
}

impl Loader for BytecodeLoader {
    fn load<'js>(&mut self, ctx: &Ctx<'js>, name: &str) -> rquickjs::Result<Module<'js>> {
        let bytecode = self.modules.get(name).ok_or_else(|| rquickjs::Error::new_loading(name))?;
        unsafe { Module::load(ctx.clone(), bytecode) }
    }
}

#[derive(Debug, Clone)]
struct SessionResolver {
    registry: Arc<Mutex<SessionModuleRegistry>>,
}

impl SessionResolver {
    fn new(registry: Arc<Mutex<SessionModuleRegistry>>) -> Self {
        Self { registry }
    }
}

impl Resolver for SessionResolver {
    fn resolve<'js>(
        &mut self,
        _ctx: &Ctx<'js>,
        base: &str,
        name: &str,
    ) -> rquickjs::Result<String> {
        let registry = self.registry.lock().map_err(|_| {
            rquickjs::Error::new_resolving_message(base, name, "js session registry mutex is poisoned")
        })?;
        if let Some(resolved) = registry.imports.get(base).and_then(|imports| imports.get(name)) {
            return Ok(resolved.clone());
        }
        if registry.modules.contains_key(name) {
            return Ok(name.to_owned());
        }
        Err(rquickjs::Error::new_resolving(base, name))
    }
}

#[derive(Debug, Clone)]
struct SessionLoader {
    registry: Arc<Mutex<SessionModuleRegistry>>,
}

impl SessionLoader {
    fn new(registry: Arc<Mutex<SessionModuleRegistry>>) -> Self {
        Self { registry }
    }
}

impl Loader for SessionLoader {
    fn load<'js>(&mut self, ctx: &Ctx<'js>, name: &str) -> rquickjs::Result<Module<'js>> {
        let record = self
            .registry
            .lock()
            .map_err(|_| rquickjs::Error::new_loading_message(name, "js session registry mutex is poisoned"))?
            .modules
            .get(name)
            .cloned()
            .ok_or_else(|| rquickjs::Error::new_loading(name))?;

        if let Some(bytecode) = record.bytecode {
            unsafe { Module::load(ctx.clone(), &bytecode) }
        } else {
            Module::declare(ctx.clone(), name, record.source)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypreact_runtime_js_core::JavaScriptModule;

    #[test]
    fn execution_cache_reuses_compiled_entry_graph() {
        let mut cache = QuickJsExecutionCache::new(None);
        let graph = JavaScriptModuleGraph {
            entry: "layouts/master-stack/index.js".into(),
            modules: vec![JavaScriptModule {
                specifier: "layouts/master-stack/index.js".into(),
                source: "export default () => ({ type: 'workspace', children: [] });".into(),
                resolved_imports: BTreeMap::new(),
            }],
        };
        let key = module_graph_execution_key(&graph);

        let _ = cache.call_entry_export_with_json_arg(
            &key,
            &graph,
            "layouts/master-stack/index.js",
            "default",
            &serde_json::Value::Null,
        );
        assert!(cache.entries.contains_key(&key));
        assert!(cache.entries.get(&key).and_then(|entry| entry.bytecode_graph.as_ref()).is_some());
    }
}
