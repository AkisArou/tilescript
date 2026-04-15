use std::fs;
use std::path::Path;

use hypreact_config::config_decode::decode_config_value;
use hypreact_config::layout_decode::decode_layout_value;
use hypreact_config::model::{Config, ConfigPaths, LayoutConfigError, LayoutDefinition};
use hypreact_config::runtime::RuntimeBundle;
use hypreact_config::selection::validate_layout_selection;
use hypreact_core::SourceLayoutNode;
use hypreact_core::runtime::layout_context::LayoutEvaluationContext;
use hypreact_core::runtime::prepared_layout::{
    PreparedLayout, PreparedStylesheet, PreparedStylesheets,
};
use hypreact_core::runtime::runtime_contract::{LayoutModuleContract, PreparedLayoutRuntime};
use hypreact_core::runtime::runtime_error::{RuntimeError, RuntimeRefreshSummary};
use hypreact_core::runtime::runtime_kind::RuntimeKind;
use hypreact_core::snapshot::{StateSnapshot, WorkspaceSnapshot};
use hypreact_runtime_lua_core::LUA_SDK_SOURCE;
use mlua::{Function, Lua, LuaSerdeExt, Table, Value};

#[derive(Debug, Default, Clone, Copy)]
pub struct LuaPreparedLayoutRuntime;

pub fn build_runtime_bundle(_paths: &ConfigPaths) -> Result<RuntimeBundle, LayoutConfigError> {
    let runtime = LuaPreparedLayoutRuntime;
    Ok(RuntimeBundle {
        config_runtime: Box::new(runtime),
        layout_runtime: Box::new(LuaPreparedLayoutRuntime),
    })
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

        let source = fs::read_to_string(&layout.module)
            .map_err(|_| RuntimeError::MissingRuntimeSource { name: layout.name.clone() })?;

        Ok(Some(PreparedLayout {
            selected: config
                .resolve_selected_layout(workspace)
                .map_err(config_error)?
                .expect("selected layout exists when config.selected_layout returned Some"),
            runtime_payload: serde_json::json!({ "source": source }),
            stylesheets: PreparedStylesheets {
                global: load_stylesheet(config.global_stylesheet_path.as_deref()),
                layout: load_stylesheet(layout.stylesheet_path.as_deref()),
            },
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

        let lua = create_lua_runtime().map_err(runtime_error)?;
        let layout_fn = lua
            .load(source)
            .set_name(&artifact.selected.module)
            .eval::<Function>()
            .map_err(runtime_error)?;
        let context_value = lua.to_value(context).map_err(runtime_error)?;
        let result = layout_fn.call::<Value>(context_value).map_err(runtime_error)?;

        if matches!(result, Value::Nil) {
            return Err(RuntimeError::Other {
                message: format!("lua layout `{}` returned nil", artifact.selected.name),
            });
        }

        let value: serde_json::Value = lua.from_value(result).map_err(runtime_error)?;
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
        write_prepared_config(authored, prepared)
    }

    fn rebuild_prepared_config(
        &self,
        authored: &Path,
        prepared: &Path,
    ) -> Result<RuntimeRefreshSummary, RuntimeError> {
        write_prepared_config(authored, prepared)
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

        let module_path = path.join("index.lua");
        if !module_path.exists() {
            continue;
        }

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

    Ok(RuntimeRefreshSummary { refreshed_files: usize::from(changed), pruned_files: 0 })
}

fn load_stylesheet(path: Option<&str>) -> Option<PreparedStylesheet> {
    let path = path?;
    let source = fs::read_to_string(path).ok()?;
    Some(PreparedStylesheet { path: path.into(), source })
}

fn create_lua_runtime() -> Result<Lua, mlua::Error> {
    let lua = Lua::new();
    preload_hypreact_sdk(&lua)?;
    Ok(lua)
}

fn preload_hypreact_sdk(lua: &Lua) -> Result<(), mlua::Error> {
    let package: Table = lua.globals().get("package")?;
    let preload: Table = package.get("preload")?;
    let loader = lua.create_function(|lua, ()| lua.load(LUA_SDK_SOURCE).eval::<Table>())?;
    preload.set("hypreact", loader)?;
    Ok(())
}

fn config_error(error: LayoutConfigError) -> RuntimeError {
    RuntimeError::Config { message: error.to_string() }
}

fn runtime_error(error: mlua::Error) -> RuntimeError {
    RuntimeError::Other { message: error.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypreact_config::model::supported_authored_config_names;

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

        let runtime = LuaPreparedLayoutRuntime;
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
    }
}
