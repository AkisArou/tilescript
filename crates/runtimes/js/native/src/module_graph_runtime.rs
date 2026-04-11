use std::collections::BTreeMap;

use rquickjs::{
    loader::{Loader, Resolver},
    Context as JsContext, Ctx, Function, Module, Object, Runtime as JsRuntime, String as JsString,
    Value,
};

use crate::JavaScriptModuleGraph;

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
    with_entry_namespace(graph, |ctx, namespace| {
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
    with_entry_namespace(graph, |ctx, namespace| {
        let function = namespace_function(ctx.clone(), &namespace, module_name, export_name)?;
        let arg_source = format!(
            "JSON.parse({})",
            serde_json::to_string(&arg.to_string()).unwrap()
        );
        let arg: Value =
            ctx.eval(arg_source)
                .map_err(|error| ModuleGraphRuntimeError::JavaScript {
                    message: format_js_error(ctx.clone(), error),
                })?;
        let value: Value =
            function
                .call((arg,))
                .map_err(|error| ModuleGraphRuntimeError::JavaScript {
                    message: format_js_error(ctx.clone(), error),
                })?;
        js_value_to_json(ctx, value)
            .map_err(|message| ModuleGraphRuntimeError::JavaScript { message })
    })
}

fn with_entry_namespace<T, F>(
    graph: &JavaScriptModuleGraph,
    f: F,
) -> Result<T, ModuleGraphRuntimeError>
where
    F: for<'js> FnOnce(Ctx<'js>, Object<'js>) -> Result<T, ModuleGraphRuntimeError>,
{
    let runtime = JsRuntime::new().map_err(|error| ModuleGraphRuntimeError::JavaScript {
        message: error.to_string(),
    })?;
    runtime.set_loader(GraphResolver::new(graph), GraphLoader::new(graph));
    let context =
        JsContext::full(&runtime).map_err(|error| ModuleGraphRuntimeError::JavaScript {
            message: error.to_string(),
        })?;

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
    namespace_export(ctx.clone(), namespace, module_name, export_name)?
        .into_function()
        .ok_or_else(|| ModuleGraphRuntimeError::NonCallableExport {
            name: module_name.to_owned(),
            export: export_name.to_owned(),
        })
}

fn js_value_to_json<'js>(
    ctx: Ctx<'js>,
    value: Value<'js>,
) -> Result<Option<serde_json::Value>, String> {
    let globals = ctx.globals();
    globals
        .set("__spiders_tmp", value)
        .map_err(|error| format_js_error(ctx.clone(), error))?;

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
        let source = self
            .modules
            .get(name)
            .ok_or_else(|| rquickjs::Error::new_loading(name))?
            .clone();
        Module::declare(ctx.clone(), name, source)
    }
}
