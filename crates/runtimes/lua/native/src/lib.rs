use std::collections::HashMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use hypreact_config::config_decode::decode_config_value;
use hypreact_config::layout_decode::decode_layout_value;
use hypreact_config::model::{Config, ConfigPaths, LayoutConfigError, LayoutDefinition};
use hypreact_config::runtime::RuntimeBundle;
use hypreact_config::selection::validate_layout_selection;
use hypreact_core::SourceLayoutNode;
use hypreact_core::runtime::layout_context::LayoutEvaluationContext;
use hypreact_core::runtime::native_artifact::{
    NativeDependencySnapshot, load_cached_stylesheet, load_text_dependency,
};
use hypreact_core::runtime::prepared_layout::{
    PreparedLayout, PreparedStylesheet, PreparedStylesheets,
};
use hypreact_core::runtime::runtime_contract::{LayoutModuleContract, PreparedLayoutRuntime};
use hypreact_core::runtime::runtime_error::{RuntimeError, RuntimeRefreshSummary};
use hypreact_core::runtime::runtime_kind::RuntimeKind;
use hypreact_core::snapshot::{StateSnapshot, WorkspaceSnapshot};
use hypreact_runtime_fennel_core::FENNEL_COMPILER_SOURCE;
use hypreact_runtime_lua_core::LUA_SDK_SOURCE;
use mlua::{Function, Lua, LuaSerdeExt, RegistryKey, Table, Value};

const LUA_BYTECODE_SCHEMA_TOKEN: &str = "lua-bytecode-v1";
const FENNEL_COMPILED_SOURCE_SCHEMA_TOKEN: &str = "fennel-compiled-lua-v1";

#[derive(Debug, Clone, Default)]
pub struct LuaPreparedLayoutRuntime {
    execution_cache: Arc<Mutex<Option<LuaExecutionCache>>>,
    bytecode_root: Option<PathBuf>,
}

#[derive(Debug)]
struct LuaExecutionCache {
    lua: Lua,
    functions: HashMap<String, RegistryKey>,
    bytecode_store: Option<LuaBytecodeStore>,
}

#[derive(Debug, Clone)]
struct LuaBytecodeStore {
    root: PathBuf,
}

pub fn build_runtime_bundle(paths: &ConfigPaths) -> Result<RuntimeBundle, LayoutConfigError> {
    let prepared_root = paths
        .prepared_config
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let runtime = LuaPreparedLayoutRuntime::with_bytecode_root(Some(prepared_root.join(".lua-bytecode")));
    Ok(RuntimeBundle {
        config_runtime: Box::new(runtime.clone()),
        layout_runtime: Box::new(runtime),
    })
}

impl LuaPreparedLayoutRuntime {
    pub fn with_bytecode_root(bytecode_root: Option<PathBuf>) -> Self {
        Self {
            execution_cache: Arc::new(Mutex::new(Some(LuaExecutionCache {
                lua: create_lua_runtime().expect("lua runtime initialization should succeed"),
                functions: HashMap::new(),
                bytecode_store: bytecode_root.clone().map(LuaBytecodeStore::new),
            }))),
            bytecode_root,
        }
    }

    fn reset_execution_cache(&self) -> Result<(), RuntimeError> {
        self.execution_cache
            .lock()
            .map_err(|_| RuntimeError::Other {
                message: "lua execution cache mutex is poisoned".into(),
            })?
            .take();
        Ok(())
    }
}

impl PreparedLayoutRuntime for LuaPreparedLayoutRuntime {
    type Config = Config;

    fn prepare_layout(
        &self,
        config: &Self::Config,
        workspace: &WorkspaceSnapshot,
    ) -> Result<Option<PreparedLayout>, RuntimeError> {
        let Some(layout) = config.selected_layout(workspace) else {
            return Ok(None);
        };

        let (source, module_dependency) = load_text_dependency(&layout.module)
            .ok_or_else(|| RuntimeError::MissingRuntimeSource { name: layout.name.clone() })?;
        let lua = create_lua_runtime().map_err(runtime_error)?;
        let source = compile_authored_source(&lua, Path::new(&layout.module), &source)
            .map_err(runtime_error)?;
        let global_stylesheet = load_stylesheet(config.global_stylesheet_path.as_deref());
        let layout_stylesheet = load_stylesheet(layout.stylesheet_path.as_deref());
        let mut dependencies = vec![module_dependency];
        if let Some(stylesheet) = global_stylesheet.as_ref() {
            dependencies.push(NativeDependencySnapshot {
                path: stylesheet.path.clone(),
                content_hash: hash_source(&stylesheet.source),
            });
        }
        if let Some(stylesheet) = layout_stylesheet.as_ref() {
            dependencies.push(NativeDependencySnapshot {
                path: stylesheet.path.clone(),
                content_hash: hash_source(&stylesheet.source),
            });
        }

        Ok(Some(PreparedLayout {
            selected: config
                .resolve_selected_layout(workspace)
                .map_err(config_error)?
                .expect("selected layout exists when config.selected_layout returned Some"),
            runtime_payload: serde_json::json!({
                "source": source,
                "sourceModule": layout.module,
            }),
            stylesheets: PreparedStylesheets {
                global: global_stylesheet,
                layout: layout_stylesheet,
            },
            dependencies,
        }))
    }

    fn build_context(
        &self,
        state: &StateSnapshot,
        workspace: &WorkspaceSnapshot,
        artifact: Option<&PreparedLayout>,
    ) -> LayoutEvaluationContext {
        state.layout_context(workspace, artifact.map(|artifact| artifact.selected.clone()))
    }

    fn evaluate_layout(
        &self,
        artifact: &PreparedLayout,
        context: &LayoutEvaluationContext,
    ) -> Result<SourceLayoutNode, RuntimeError> {
        let source =
            artifact.runtime_payload.get("source").and_then(serde_json::Value::as_str).ok_or_else(
                || RuntimeError::Other {
                    message: format!(
                        "lua runtime payload for `{}` is missing source",
                        artifact.selected.name
                    ),
                },
            )?;

        let mut cache_guard = self.execution_cache.lock().map_err(|_| RuntimeError::Other {
            message: "lua execution cache mutex is poisoned".into(),
        })?;
        if cache_guard.is_none() {
            *cache_guard = Some(LuaExecutionCache {
                lua: create_lua_runtime().map_err(runtime_error)?,
                functions: HashMap::new(),
                bytecode_store: self.bytecode_root.clone().map(LuaBytecodeStore::new),
            });
        }
        let cache = cache_guard.as_mut().expect("lua execution cache initialized");
        let function_key = executable_function_key(&artifact.selected.module, source);
        if !cache.functions.contains_key(&function_key) {
            let function = load_or_compile_function(cache, &artifact.selected.module, source)
                .map_err(runtime_error)?;
            let registry_key = cache.lua.create_registry_value(function).map_err(runtime_error)?;
            cache.functions.insert(function_key.clone(), registry_key);
        }
        let layout_fn: Function = cache
            .lua
            .registry_value(cache.functions.get(&function_key).expect("cached registry key exists"))
            .map_err(runtime_error)?;
        let context_value = cache.lua.to_value(context).map_err(runtime_error)?;
        let result = layout_fn.call::<Value>(context_value).map_err(runtime_error)?;

        if matches!(result, Value::Nil) {
            return Err(RuntimeError::Other {
                message: format!("lua layout `{}` returned nil", artifact.selected.name),
            });
        }

        let value: serde_json::Value = cache.lua.from_value(result).map_err(runtime_error)?;
        decode_layout_value(&value).map_err(|message| RuntimeError::Other {
            message: format!(
                "lua to layout conversion failed for `{}`: {message}",
                artifact.selected.name
            ),
        })
    }

    fn contract(&self) -> LayoutModuleContract {
        LayoutModuleContract::default()
    }
}

impl hypreact_config::runtime::AuthoringConfigRuntime for LuaPreparedLayoutRuntime {
    fn load_authored_config(&self, path: &Path) -> Result<Config, RuntimeError> {
        load_authored_config(path).map_err(config_error)
    }

    fn load_prepared_config(&self, path: &Path) -> Result<Config, RuntimeError> {
        Config::from_path(path).map_err(config_error)
    }

    fn refresh_prepared_config(
        &self,
        authored: &Path,
        prepared: &Path,
    ) -> Result<RuntimeRefreshSummary, RuntimeError> {
        let result = write_prepared_config(authored, prepared);
        if result.is_ok() {
            self.reset_execution_cache()?;
        }
        result
    }

    fn rebuild_prepared_config(
        &self,
        authored: &Path,
        prepared: &Path,
    ) -> Result<RuntimeRefreshSummary, RuntimeError> {
        let result = write_prepared_config(authored, prepared);
        if result.is_ok() {
            self.reset_execution_cache()?;
        }
        result
    }
}

fn load_authored_config(path: &Path) -> Result<Config, LayoutConfigError> {
    let source = fs::read_to_string(path)
        .map_err(|_| LayoutConfigError::ReadConfig { path: path.to_path_buf() })?;
    let root_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let lua = create_lua_runtime().map_err(|error| LayoutConfigError::EvaluateAuthoredConfig {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let source = compile_authored_source(&lua, path, &source).map_err(|error| {
        LayoutConfigError::CompileAuthoredConfig {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    let raw =
        lua.load(&source).set_name(path.to_string_lossy().into_owned()).eval::<Value>().map_err(
            |error| LayoutConfigError::EvaluateAuthoredConfig {
                path: path.to_path_buf(),
                message: error.to_string(),
            },
        )?;
    let value: serde_json::Value =
        lua.from_value(raw).map_err(|error| LayoutConfigError::DecodeAuthoredConfig {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    let mut config = decode_config_value(path, &value)?;
    config.global_stylesheet_path = root_dir
        .join("index.css")
        .exists()
        .then(|| root_dir.join("index.css").to_string_lossy().into_owned());
    config.layouts = discover_layout_definitions(root_dir)?;
    validate_layout_selection(
        path,
        config.default_layout.as_deref(),
        &config.layout_rules,
        &config.layouts,
    )?;
    Ok(config)
}

fn discover_layout_definitions(
    root_dir: &Path,
) -> Result<Vec<LayoutDefinition>, LayoutConfigError> {
    let layouts_dir = root_dir.join("layouts");
    if !layouts_dir.exists() {
        return Ok(Vec::new());
    }

    let mut layouts = Vec::new();
    for entry in fs::read_dir(&layouts_dir)
        .map_err(|_| LayoutConfigError::ReadConfig { path: layouts_dir.clone() })?
    {
        let entry =
            entry.map_err(|_| LayoutConfigError::ReadConfig { path: layouts_dir.clone() })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let Some(module_path) = discover_layout_module_path(&path) else {
            continue;
        };

        let stylesheet_path = path.join("index.css");
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default().to_owned();
        layouts.push(LayoutDefinition {
            name,
            runtime: RuntimeKind::Lua,
            directory: path.to_string_lossy().into_owned(),
            module: module_path.to_string_lossy().into_owned(),
            stylesheet_path: stylesheet_path
                .exists()
                .then(|| stylesheet_path.to_string_lossy().into_owned()),
            runtime_cache_payload: None,
        });
    }

    layouts.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(layouts)
}

fn write_prepared_config(
    authored: &Path,
    prepared: &Path,
) -> Result<RuntimeRefreshSummary, RuntimeError> {
    let config = load_authored_config(authored).map_err(config_error)?;
    let bytecode_root = prepared
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".lua-bytecode");
    let expected_bytecode_keys = expected_lua_bytecode_cache_keys(&config).map_err(config_error)?;
    let serialized = serde_json::to_string_pretty(&config)
        .map_err(|error| RuntimeError::Config { message: error.to_string() })?;

    if let Some(parent) = prepared.parent() {
        fs::create_dir_all(parent).map_err(|_| RuntimeError::Config {
            message: format!("failed to create prepared config directory `{}`", parent.display()),
        })?;
    }

    let changed = fs::read_to_string(prepared).map_or(true, |existing| existing != serialized);
    if changed {
        fs::write(prepared, serialized).map_err(|_| RuntimeError::Config {
            message: format!("failed to write prepared config `{}`", prepared.display()),
        })?;
    }

    let pruned_files =
        prune_stale_lua_bytecode_cache(&bytecode_root, &expected_bytecode_keys).map_err(config_error)?;

    Ok(RuntimeRefreshSummary { refreshed_files: usize::from(changed), pruned_files })
}

fn load_stylesheet(path: Option<&str>) -> Option<PreparedStylesheet> {
    let path = path?;
    load_cached_stylesheet(path).map(|(stylesheet, _)| stylesheet)
}

fn create_lua_runtime() -> Result<Lua, mlua::Error> {
    let lua = Lua::new();
    preload_hypreact_sdk(&lua)?;
    preload_fennel_compiler(&lua)?;
    Ok(lua)
}

impl LuaBytecodeStore {
    fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn path_for_key(&self, key: &str) -> PathBuf {
        self.root.join(format!("{key}.luac"))
    }

    fn load(&self, key: &str) -> std::io::Result<Option<Vec<u8>>> {
        let path = self.path_for_key(key);
        match fs::read(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn store(&self, key: &str, bytecode: &[u8]) -> std::io::Result<()> {
        fs::create_dir_all(&self.root)?;
        fs::write(self.path_for_key(key), bytecode)
    }
}

fn load_or_compile_function(
    cache: &LuaExecutionCache,
    module_name: &str,
    source: &str,
) -> Result<Function, mlua::Error> {
    let bytecode_key = lua_bytecode_artifact_key(module_name, source);

    if let Some(store) = cache.bytecode_store.as_ref()
        && let Some(bytecode) = store.load(&bytecode_key).map_err(mlua::Error::external)?
    {
        if let Ok(chunk_fn) = cache.lua.load(&bytecode).set_name(module_name).into_function() {
            let value = chunk_fn.call::<Value>(())?;
            if let Value::Function(function) = value {
                return Ok(function);
            }
        }
    }

    let chunk_fn = cache
        .lua
        .load(source)
        .set_name(module_name)
        .into_function()?;
    let bytecode = chunk_fn.dump(false);
    let value = chunk_fn.call::<Value>(())?;
    let function = match value {
        Value::Function(function) => function,
        other => {
            return Err(mlua::Error::external(format!(
                "lua chunk `{module_name}` did not return a function (got {other:?})"
            )))
        }
    };
    if let Some(store) = cache.bytecode_store.as_ref() {
        store.store(&bytecode_key, &bytecode).map_err(mlua::Error::external)?;
    }
    Ok(function)
}

fn expected_lua_bytecode_cache_keys(config: &Config) -> Result<BTreeSet<String>, LayoutConfigError> {
    let lua = create_lua_runtime().map_err(|error| LayoutConfigError::CompileAuthoredConfig {
        path: PathBuf::from("<lua-runtime>"),
        message: error.to_string(),
    })?;
    let mut keys = BTreeSet::new();

    for layout in &config.layouts {
        let source = fs::read_to_string(&layout.module)
            .map_err(|_| LayoutConfigError::ReadConfig { path: PathBuf::from(&layout.module) })?;
        let compiled = compile_authored_source(&lua, Path::new(&layout.module), &source).map_err(
            |error| LayoutConfigError::CompileAuthoredConfig {
                path: PathBuf::from(&layout.module),
                message: error.to_string(),
            },
        )?;
        keys.insert(lua_bytecode_artifact_key(&layout.module, &compiled));
    }

    Ok(keys)
}

fn prune_stale_lua_bytecode_cache(
    bytecode_root: &Path,
    expected_keys: &BTreeSet<String>,
) -> Result<usize, LayoutConfigError> {
    if !bytecode_root.exists() {
        return Ok(0);
    }

    let entries = match fs::read_dir(bytecode_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(_) => return Err(LayoutConfigError::ReadConfig { path: bytecode_root.to_path_buf() }),
    };

    let mut pruned = 0usize;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                return Err(LayoutConfigError::ReadConfig { path: bytecode_root.to_path_buf() });
            }
        };
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(LayoutConfigError::ReadConfig { path: path.clone() }),
        };

        if !file_type.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("luac") {
            continue;
        }

        let Some(key) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if expected_keys.contains(key) {
            continue;
        }

        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(LayoutConfigError::ReadConfig { path: path.clone() }),
        }
        pruned += 1;
    }

    Ok(pruned)
}

fn preload_hypreact_sdk(lua: &Lua) -> Result<(), mlua::Error> {
    let package: Table = lua.globals().get("package")?;
    let preload: Table = package.get("preload")?;
    let loader = lua.create_function(|lua, ()| lua.load(LUA_SDK_SOURCE).eval::<Table>())?;
    preload.set("hypreact", loader)?;
    Ok(())
}

fn preload_fennel_compiler(lua: &Lua) -> Result<(), mlua::Error> {
    let package: Table = lua.globals().get("package")?;
    let preload: Table = package.get("preload")?;
    let loader = lua.create_function(|lua, ()| lua.load(FENNEL_COMPILER_SOURCE).eval::<Table>())?;
    preload.set("fennel", loader)?;
    Ok(())
}

fn compile_authored_source(lua: &Lua, path: &Path, source: &str) -> Result<String, mlua::Error> {
    if path.extension().and_then(|ext| ext.to_str()) != Some("fnl") {
        return Ok(source.to_owned());
    }

    let fennel: Table = lua.load("return require('fennel')").eval()?;
    let compile_string: Function = fennel.get("compileString")?;
    let options = lua.create_table()?;
    options.set("filename", path.to_string_lossy().into_owned())?;
    compile_string.call((source, options))
}

fn discover_layout_module_path(path: &Path) -> Option<PathBuf> {
    let lua_module_path = path.join("index.lua");
    if lua_module_path.exists() {
        return Some(lua_module_path);
    }

    let fennel_module_path = path.join("index.fnl");
    fennel_module_path.exists().then_some(fennel_module_path)
}

fn config_error(error: LayoutConfigError) -> RuntimeError {
    RuntimeError::Config { message: error.to_string() }
}

fn runtime_error(error: mlua::Error) -> RuntimeError {
    RuntimeError::Other { message: error.to_string() }
}

fn hash_source(source: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn executable_function_key(module: &str, source: &str) -> String {
    let module_hash = hash_source(module);
    let source_hash = hash_source(source);
    format!("{module_hash}-{source_hash}")
}

pub fn lua_executable_artifact_key(module: &str, source: &str) -> String {
    executable_function_key(module, source)
}

pub fn lua_compiled_source_artifact_key(module: &str, source: &str) -> String {
    let module_hash = hash_source(module);
    let source_hash = hash_source(source);
    let schema_hash = hash_source(FENNEL_COMPILED_SOURCE_SCHEMA_TOKEN);
    format!("{module_hash}-{source_hash}-{schema_hash}")
}

pub fn lua_bytecode_artifact_key(module: &str, source: &str) -> String {
    let mut key = executable_function_key(module, source);
    key.push('-');
    key.push_str(&hash_source(LUA_BYTECODE_SCHEMA_TOKEN));
    key
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypreact_config::model::supported_authored_config_names;
    use hypreact_config::runtime::AuthoringConfigRuntime;

    #[test]
    fn loads_lua_authored_config_and_discovers_lua_layouts() {
        let root = std::env::temp_dir().join(format!(
            "hypreact-lua-config-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(root.join("layouts/master-stack")).unwrap();
        std::fs::write(
            root.join("config.lua"),
            "return { defaultLayout = 'master-stack', layoutRules = { { index = 0, layout = 'master-stack' } } }",
        )
        .unwrap();
        std::fs::write(
            root.join("layouts/master-stack/index.lua"),
            "local h = require('hypreact') return function(ctx) return h.workspace({ id = 'root' }) { h.slot({ id = 'main', take = 1 }) } end",
        )
        .unwrap();

        let config = load_authored_config(&root.join("config.lua")).unwrap();

        assert_eq!(config.default_layout.as_deref(), Some("master-stack"));
        assert_eq!(config.layouts.len(), 1);
        assert_eq!(config.layouts[0].runtime, RuntimeKind::Lua);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn evaluates_lua_layout_dsl() {
        let root = std::env::temp_dir().join(format!(
            "hypreact-lua-layout-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(root.join("layouts/master-stack")).unwrap();
        std::fs::write(
            root.join("layouts/master-stack/index.lua"),
            "local h = require('hypreact') return function(ctx) return h.workspace({ id = 'frame' }) { h.slot({ id = 'master', take = 1 }), h.when(#ctx.windows > 1) { h.group({ class = 'stack' }) { h.slot({ id = 'stack-slot' }) } } } end",
        )
        .unwrap();

        let runtime = LuaPreparedLayoutRuntime::default();
        let config = Config {
            layouts: vec![LayoutDefinition {
                name: "master-stack".into(),
                runtime: RuntimeKind::Lua,
                directory: root.join("layouts/master-stack").to_string_lossy().into_owned(),
                module: root.join("layouts/master-stack/index.lua").to_string_lossy().into_owned(),
                stylesheet_path: None,
                runtime_cache_payload: None,
            }],
            default_layout: Some("master-stack".into()),
            layout_rules: vec![],
            global_stylesheet_path: None,
            resize: Default::default(),
        };
        let workspace = WorkspaceSnapshot {
            id: hypreact_core::WorkspaceId::from("ws-1"),
            name: "1".into(),
            output_id: None,
            layout_space: None,
            active_workspaces: vec!["1".into()],
            focused: true,
            visible: true,
            effective_layout: Some(hypreact_core::types::LayoutRef { name: "master-stack".into() }),
        };
        let state = StateSnapshot {
            focused_window_id: None,
            current_output_id: None,
            current_workspace_id: Some(hypreact_core::WorkspaceId::from("ws-1")),
            outputs: vec![],
            workspaces: vec![workspace.clone()],
            windows: vec![],
            visible_window_ids: vec![],
            workspace_names: vec!["1".into()],
            resize_state: Default::default(),
        };

        let artifact = runtime.prepare_layout(&config, &workspace).unwrap().unwrap();
        let context = runtime.build_context(&state, &workspace, Some(&artifact));
        let layout = runtime.evaluate_layout(&artifact, &context).unwrap();

        match layout {
            SourceLayoutNode::Workspace { meta, children } => {
                assert_eq!(meta.id.as_deref(), Some("frame"));
                assert_eq!(children.len(), 1);
            }
            _ => panic!("expected workspace root"),
        }

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn supported_config_names_include_lua() {
        assert!(supported_authored_config_names().contains(&"config.lua"));
        assert!(supported_authored_config_names().contains(&"config.fnl"));
    }

    #[test]
    fn loads_fennel_authored_config_and_discovers_fennel_layouts() {
        let root = std::env::temp_dir().join(format!(
            "hypreact-fennel-config-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(root.join("layouts/master-stack")).unwrap();
        std::fs::write(
            root.join("config.fnl"),
            "{:defaultLayout \"master-stack\" :layoutRules [{:index 0 :layout \"master-stack\"}]}",
        )
        .unwrap();
        std::fs::write(
            root.join("layouts/master-stack/index.fnl"),
            "(local h (require \"hypreact\"))\n(fn [ctx] ((h.workspace {:id \"root\"}) [(h.slot {:id \"main\" :take 1})]))",
        )
        .unwrap();

        let config = load_authored_config(&root.join("config.fnl")).unwrap();

        assert_eq!(config.default_layout.as_deref(), Some("master-stack"));
        assert_eq!(config.layouts.len(), 1);
        assert_eq!(config.layouts[0].runtime, RuntimeKind::Lua);
        assert!(config.layouts[0].module.ends_with("index.fnl"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn evaluates_fennel_layout_dsl() {
        let root = std::env::temp_dir().join(format!(
            "hypreact-fennel-layout-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(root.join("layouts/master-stack")).unwrap();
        std::fs::write(
            root.join("layouts/master-stack/index.fnl"),
            "(local h (require \"hypreact\"))\n(fn [ctx] ((h.workspace {:id \"frame\"}) [(h.slot {:id \"master\" :take 1})]))",
        )
        .unwrap();

        let runtime = LuaPreparedLayoutRuntime::default();
        let config = Config {
            layouts: vec![LayoutDefinition {
                name: "master-stack".into(),
                runtime: RuntimeKind::Lua,
                directory: root.join("layouts/master-stack").to_string_lossy().into_owned(),
                module: root.join("layouts/master-stack/index.fnl").to_string_lossy().into_owned(),
                stylesheet_path: None,
                runtime_cache_payload: None,
            }],
            default_layout: Some("master-stack".into()),
            layout_rules: vec![],
            global_stylesheet_path: None,
            resize: Default::default(),
        };
        let workspace = WorkspaceSnapshot {
            id: hypreact_core::WorkspaceId::from("ws-1"),
            name: "1".into(),
            output_id: None,
            layout_space: None,
            active_workspaces: vec!["1".into()],
            focused: true,
            visible: true,
            effective_layout: Some(hypreact_core::types::LayoutRef { name: "master-stack".into() }),
        };
        let state = StateSnapshot {
            focused_window_id: None,
            current_output_id: None,
            current_workspace_id: Some(hypreact_core::WorkspaceId::from("ws-1")),
            outputs: vec![],
            workspaces: vec![workspace.clone()],
            windows: vec![],
            visible_window_ids: vec![],
            workspace_names: vec!["1".into()],
            resize_state: Default::default(),
        };

        let artifact = runtime.prepare_layout(&config, &workspace).unwrap().unwrap();
        let context = runtime.build_context(&state, &workspace, Some(&artifact));
        let layout = runtime.evaluate_layout(&artifact, &context).unwrap();

        match layout {
            SourceLayoutNode::Workspace { meta, children } => {
                assert_eq!(meta.id.as_deref(), Some("frame"));
                assert_eq!(children.len(), 1);
            }
            _ => panic!("expected workspace root"),
        }

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn persists_lua_bytecode_and_reloads_it() {
        let temp = tempfile::TempDir::new().unwrap();
        let runtime = LuaPreparedLayoutRuntime::with_bytecode_root(Some(temp.path().join(".lua-bytecode")));
        let config = Config {
            layouts: vec![LayoutDefinition {
                name: "master-stack".into(),
                runtime: RuntimeKind::Lua,
                directory: temp.path().join("layouts/master-stack").to_string_lossy().into_owned(),
                module: temp.path().join("layouts/master-stack/index.lua").to_string_lossy().into_owned(),
                stylesheet_path: None,
                runtime_cache_payload: None,
            }],
            default_layout: Some("master-stack".into()),
            layout_rules: vec![],
            global_stylesheet_path: None,
            resize: Default::default(),
        };
        let workspace = WorkspaceSnapshot {
            id: hypreact_core::WorkspaceId::from("ws-1"),
            name: "1".into(),
            output_id: None,
            layout_space: None,
            active_workspaces: vec!["1".into()],
            focused: true,
            visible: true,
            effective_layout: Some(hypreact_core::types::LayoutRef { name: "master-stack".into() }),
        };
        let state = StateSnapshot {
            focused_window_id: None,
            current_output_id: None,
            current_workspace_id: Some(hypreact_core::WorkspaceId::from("ws-1")),
            outputs: vec![],
            workspaces: vec![workspace.clone()],
            windows: vec![],
            visible_window_ids: vec![],
            workspace_names: vec!["1".into()],
            resize_state: Default::default(),
        };

        std::fs::create_dir_all(temp.path().join("layouts/master-stack")).unwrap();
        std::fs::write(
            temp.path().join("layouts/master-stack/index.lua"),
            "local h = require('hypreact') return function(ctx) return h.workspace({ id = 'frame' }) { h.slot({ id = 'master', take = 1 }) } end",
        )
        .unwrap();

        let artifact = runtime.prepare_layout(&config, &workspace).unwrap().unwrap();
        let context = runtime.build_context(&state, &workspace, Some(&artifact));
        let _ = runtime.evaluate_layout(&artifact, &context).unwrap();

        let bytecode_dir = temp.path().join(".lua-bytecode");
        assert!(std::fs::read_dir(&bytecode_dir).unwrap().next().is_some());
    }

    #[test]
    fn rebuild_prepared_config_prunes_stale_lua_bytecode_entries() {
        let temp = tempfile::TempDir::new().unwrap();
        let authored_config = temp.path().join("config.lua");
        let prepared_config = temp.path().join(".hypreact-build/config.js");
        let bytecode_dir = temp.path().join(".hypreact-build/.lua-bytecode");
        let runtime = LuaPreparedLayoutRuntime::with_bytecode_root(Some(bytecode_dir.clone()));
        let layout_dir = temp.path().join("layouts/master-stack");
        std::fs::create_dir_all(&layout_dir).unwrap();
        std::fs::create_dir_all(&bytecode_dir).unwrap();
        std::fs::write(
            &authored_config,
            "return { defaultLayout = 'master-stack', layoutRules = { { index = 0, layout = 'master-stack' } } }",
        )
        .unwrap();

        let original_source =
            "local h = require('hypreact') return function(ctx) return h.workspace({ id = 'frame' }) { h.slot({ id = 'master', take = 1 }) } end";
        std::fs::write(layout_dir.join("index.lua"), original_source).unwrap();

        let refresh = runtime.rebuild_prepared_config(&authored_config, &prepared_config).unwrap();
        assert_eq!(refresh.pruned_files, 0);

        let config = load_authored_config(&authored_config).unwrap();
        let workspace = WorkspaceSnapshot {
            id: hypreact_core::WorkspaceId::from("ws-1"),
            name: "1".into(),
            output_id: None,
            layout_space: None,
            active_workspaces: vec!["1".into()],
            focused: true,
            visible: true,
            effective_layout: Some(hypreact_core::types::LayoutRef { name: "master-stack".into() }),
        };
        let state = StateSnapshot {
            focused_window_id: None,
            current_output_id: None,
            current_workspace_id: Some(hypreact_core::WorkspaceId::from("ws-1")),
            outputs: vec![],
            workspaces: vec![workspace.clone()],
            windows: vec![],
            visible_window_ids: vec![],
            workspace_names: vec!["1".into()],
            resize_state: Default::default(),
        };

        let artifact = runtime.prepare_layout(&config, &workspace).unwrap().unwrap();
        let context = runtime.build_context(&state, &workspace, Some(&artifact));
        let _ = runtime.evaluate_layout(&artifact, &context).unwrap();

        let old_key = lua_bytecode_artifact_key(&config.layouts[0].module, original_source);
        assert!(bytecode_dir.join(format!("{old_key}.luac")).exists());

        std::fs::write(
            layout_dir.join("index.lua"),
            "local h = require('hypreact') return function(ctx) return h.workspace({ id = 'changed' }) { h.slot({ id = 'master', take = 2 }) } end",
        )
        .unwrap();

        let refresh = runtime.rebuild_prepared_config(&authored_config, &prepared_config).unwrap();
        assert_eq!(refresh.pruned_files, 1);
        assert!(!bytecode_dir.join(format!("{old_key}.luac")).exists());
    }

    #[test]
    fn rebuild_prepared_config_prunes_stale_fennel_bytecode_entries() {
        let temp = tempfile::TempDir::new().unwrap();
        let authored_config = temp.path().join("config.fnl");
        let prepared_config = temp.path().join(".hypreact-build/config.js");
        let bytecode_dir = temp.path().join(".hypreact-build/.lua-bytecode");
        let runtime = LuaPreparedLayoutRuntime::with_bytecode_root(Some(bytecode_dir.clone()));
        let layout_dir = temp.path().join("layouts/master-stack");
        std::fs::create_dir_all(&layout_dir).unwrap();
        std::fs::create_dir_all(&bytecode_dir).unwrap();
        std::fs::write(
            &authored_config,
            "{:defaultLayout \"master-stack\" :layoutRules [{:index 0 :layout \"master-stack\"}]}",
        )
        .unwrap();

        let original_source =
            "(local h (require \"hypreact\"))\n(fn [ctx] ((h.workspace {:id \"frame\"}) [(h.slot {:id \"master\" :take 1})]))";
        std::fs::write(layout_dir.join("index.fnl"), original_source).unwrap();

        let refresh = runtime.rebuild_prepared_config(&authored_config, &prepared_config).unwrap();
        assert_eq!(refresh.pruned_files, 0);

        let config = load_authored_config(&authored_config).unwrap();
        let workspace = WorkspaceSnapshot {
            id: hypreact_core::WorkspaceId::from("ws-1"),
            name: "1".into(),
            output_id: None,
            layout_space: None,
            active_workspaces: vec!["1".into()],
            focused: true,
            visible: true,
            effective_layout: Some(hypreact_core::types::LayoutRef { name: "master-stack".into() }),
        };
        let state = StateSnapshot {
            focused_window_id: None,
            current_output_id: None,
            current_workspace_id: Some(hypreact_core::WorkspaceId::from("ws-1")),
            outputs: vec![],
            workspaces: vec![workspace.clone()],
            windows: vec![],
            visible_window_ids: vec![],
            workspace_names: vec!["1".into()],
            resize_state: Default::default(),
        };

        let artifact = runtime.prepare_layout(&config, &workspace).unwrap().unwrap();
        let compiled_source = artifact.runtime_payload.get("source").and_then(serde_json::Value::as_str).unwrap();
        let context = runtime.build_context(&state, &workspace, Some(&artifact));
        let _ = runtime.evaluate_layout(&artifact, &context).unwrap();

        let old_key = lua_bytecode_artifact_key(&config.layouts[0].module, compiled_source);
        assert!(bytecode_dir.join(format!("{old_key}.luac")).exists());

        std::fs::write(
            layout_dir.join("index.fnl"),
            "(local h (require \"hypreact\"))\n(fn [ctx] ((h.workspace {:id \"changed\"}) [(h.slot {:id \"master\" :take 2})]))",
        )
        .unwrap();

        let refresh = runtime.rebuild_prepared_config(&authored_config, &prepared_config).unwrap();
        assert_eq!(refresh.pruned_files, 1);
        assert!(!bytecode_dir.join(format!("{old_key}.luac")).exists());
    }
}
