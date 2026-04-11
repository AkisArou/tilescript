use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use oxc::span::SourceType;
use serde_json::Value;
use tracing::debug;

use crate::module_graph_runtime::evaluate_entry_export_to_json;
use hypreact_config::model::{Config, LayoutConfigError, LayoutDefinition, LayoutSelectionConfig};
use crate::compile::{AppBuildPlan, compile_app, compiled_app_to_module_graph};
use crate::graph::{
    DiscoveredApp, ModuleGraph, ModuleGraphBuilder, discover_project_apps,
};
use crate::{
    JavaScriptModule, JavaScriptModuleGraph, encode_runtime_graph_payload,
};

pub fn load_authored_config(path: impl AsRef<Path>) -> Result<Config, LayoutConfigError> {
    debug!(path = %path.as_ref().display(), "loading authored project config");
    load_project_config(path.as_ref())
}

pub fn load_prepared_config(path: impl AsRef<Path>) -> Result<Config, LayoutConfigError> {
    debug!(path = %path.as_ref().display(), "loading prepared project config");
    load_project_config(path.as_ref())
}

fn load_project_config(path: &Path) -> Result<Config, LayoutConfigError> {
    let project =
        discover_project_apps(path).map_err(|error| LayoutConfigError::CompileAuthoredConfig {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    let config_graph = ModuleGraphBuilder::new().build(&project.config_app).map_err(|error| {
        LayoutConfigError::CompileAuthoredConfig {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    let config_plan = AppBuildPlan::from_graph(&config_graph);
    let compiled_config =
        compile_app(&config_plan).map_err(|error| LayoutConfigError::CompileAuthoredConfig {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    let config_runtime_graph = compiled_app_to_module_graph(&config_graph, &compiled_config)
        .map_err(|error| LayoutConfigError::CompileAuthoredConfig {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    let config_value = evaluate_compiled_config(path, &config_runtime_graph)?;
    let mut config = decode_config_value(path, &config_value)?;

    config.global_stylesheet_path =
        project.global_stylesheet_path.as_ref().map(|path| path.to_string_lossy().into_owned());

    let mut layout_defs = Vec::new();
    for app in &project.layout_apps {
        let graph = ModuleGraphBuilder::new().build(app).map_err(|error| {
            LayoutConfigError::CompileAuthoredConfig {
                path: app.entry_path.clone(),
                message: error.to_string(),
            }
        })?;
        let plan = AppBuildPlan::from_graph(&graph);
        let compiled =
            compile_app(&plan).map_err(|error| LayoutConfigError::CompileAuthoredConfig {
                path: app.entry_path.clone(),
                message: error.to_string(),
            })?;
        let runtime_graph = compiled_app_to_module_graph(&graph, &compiled).map_err(|error| {
            LayoutConfigError::CompileAuthoredConfig {
                path: app.entry_path.clone(),
                message: error.to_string(),
            }
        })?;

        layout_defs.push(LayoutDefinition {
            name: app.name.clone(),
            directory: app
                .entry_path
                .parent()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
            module: runtime_graph.entry.clone(),
            stylesheet_path: app
                .stylesheet_path
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            runtime_cache_payload: Some(encode_runtime_graph_payload(&runtime_graph)),
        });

        debug!(
            layout = %app.name,
            entry = %app.entry_path.display(),
            stylesheet_path = app
                .stylesheet_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "<none>".into()),
            "discovered layout app during config load"
        );
    }

    validate_layout_selection(path, &config.layout_selection, &layout_defs)?;
    config.layouts = layout_defs;
    Ok(config)
}

pub fn refresh_prepared_config(
    authored_path: impl AsRef<Path>,
    runtime_path: impl AsRef<Path>,
) -> Result<JsRuntimeCacheUpdate, LayoutConfigError> {
    write_runtime_cache(authored_path.as_ref(), runtime_path.as_ref(), false)
}

pub fn rebuild_prepared_config(
    authored_path: impl AsRef<Path>,
    runtime_path: impl AsRef<Path>,
) -> Result<JsRuntimeCacheUpdate, LayoutConfigError> {
    write_runtime_cache(authored_path.as_ref(), runtime_path.as_ref(), true)
}

fn write_runtime_cache(
    authored_path: &Path,
    runtime_path: &Path,
    force_rebuild: bool,
) -> Result<JsRuntimeCacheUpdate, LayoutConfigError> {
    let runtime_root = runtime_path.parent().unwrap_or_else(|| Path::new("."));
    let mut update = JsRuntimeCacheUpdate::default();
    let mut expected_paths = BTreeSet::new();
    let project = discover_project_apps(authored_path).map_err(|error| {
        LayoutConfigError::CompileAuthoredConfig {
            path: authored_path.to_path_buf(),
            message: error.to_string(),
        }
    })?;

    let config_outputs =
        write_compiled_app(&project.config_app, runtime_root, runtime_path, force_rebuild)?;
    update.rebuilt_files += config_outputs.written_files;
    expected_paths.extend(config_outputs.paths);
    for app in &project.layout_apps {
        let outputs = write_compiled_app(app, runtime_root, runtime_path, force_rebuild)?;
        update.rebuilt_files += outputs.written_files;
        expected_paths.extend(outputs.paths);
    }

    if let Some(stylesheet) = &project.global_stylesheet_path {
        let destination = runtime_root.join(
            stylesheet.file_name().map(PathBuf::from).unwrap_or_else(|| PathBuf::from("index.css")),
        );
        if copy_stylesheet_if_stale(stylesheet, &destination, force_rebuild)? {
            update.copied_stylesheets += 1;
        }
        expected_paths.insert(destination);
    }
    update.pruned_files = prune_stale_runtime_cache(runtime_root, &expected_paths)?;

    Ok(update)
}

fn evaluate_compiled_config(
    path: &Path,
    graph: &JavaScriptModuleGraph,
) -> Result<Value, LayoutConfigError> {
    evaluate_entry_export_to_json(graph, &graph.entry, "default")
        .map_err(|error| LayoutConfigError::EvaluateAuthoredConfig {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?
        .ok_or_else(|| LayoutConfigError::DecodeAuthoredConfig {
            path: path.to_path_buf(),
            message: "config app returned undefined".into(),
        })
}

fn write_compiled_app(
    app: &DiscoveredApp,
    runtime_root: &Path,
    runtime_entry_path: &Path,
    force_rebuild: bool,
) -> Result<CompiledRuntimeAppOutputs, LayoutConfigError> {
    let graph = ModuleGraphBuilder::new().build(app).map_err(|error| {
        LayoutConfigError::CompileAuthoredConfig {
            path: app.entry_path.clone(),
            message: error.to_string(),
        }
    })?;
    let expected_paths = graph
        .modules
        .values()
        .filter_map(|record| match &record.id {
            crate::graph::ModuleId::File(path) => match record.kind {
                crate::graph::ModuleKind::Script => {
                    Some(runtime_root.join(runtime_relative_path(
                        path,
                        &graph.app.root_dir,
                        runtime_entry_path.file_name().and_then(|name| name.to_str()),
                    )))
                }
                crate::graph::ModuleKind::Stylesheet => {
                    Some(runtime_root.join(runtime_static_relative_path(path, &graph.app.root_dir)))
                }
                crate::graph::ModuleKind::Virtual => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    if !force_rebuild && app_cache_is_fresh(&graph, runtime_root, runtime_entry_path) {
        return Ok(CompiledRuntimeAppOutputs { paths: expected_paths, written_files: 0 });
    }
    let plan = AppBuildPlan::from_graph(&graph);
    let compiled =
        compile_app(&plan).map_err(|error| LayoutConfigError::CompileAuthoredConfig {
            path: app.entry_path.clone(),
            message: error.to_string(),
        })?;
    let module_graph = compiled_app_to_module_graph(&graph, &compiled).map_err(|error| {
        LayoutConfigError::CompileAuthoredConfig {
            path: app.entry_path.clone(),
            message: error.to_string(),
        }
    })?;

    let mut written_files = 0usize;
    for module in &module_graph.modules {
        if is_virtual_sdk_specifier(&module.specifier) {
            continue;
        }
        let destination = runtime_destination_for_specifier(
            &module.specifier,
            runtime_root,
            runtime_entry_path,
            &graph.app.root_dir,
        );
        let rewritten = rewrite_module_for_runtime(
            &module,
            &destination,
            runtime_root,
            runtime_entry_path,
            &graph.app.root_dir,
        )?;
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|_| LayoutConfigError::ReadConfig { path: parent.to_path_buf() })?;
        }
        std::fs::write(&destination, rewritten)
            .map_err(|_| LayoutConfigError::ReadConfig { path: destination.clone() })?;
        written_files += 1;
    }

    if let Some(stylesheet_path) = app.stylesheet_path.as_ref() {
        let destination =
            runtime_root.join(runtime_static_relative_path(stylesheet_path, &graph.app.root_dir));
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|_| LayoutConfigError::ReadConfig { path: parent.to_path_buf() })?;
        }
        std::fs::write(&destination, &compiled.stylesheet)
            .map_err(|_| LayoutConfigError::ReadConfig { path: destination.clone() })?;
        written_files += 1;
    }

    for stylesheet in &plan.stylesheet_modules {
        if app.stylesheet_path.as_ref() == Some(stylesheet) {
            continue;
        }
        let destination =
            runtime_root.join(runtime_static_relative_path(stylesheet, &graph.app.root_dir));
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|_| LayoutConfigError::ReadConfig { path: parent.to_path_buf() })?;
        }
        std::fs::copy(stylesheet, &destination)
            .map_err(|_| LayoutConfigError::ReadConfig { path: destination.clone() })?;
        written_files += 1;
    }

    Ok(CompiledRuntimeAppOutputs { paths: expected_paths, written_files })
}

fn app_cache_is_fresh(graph: &ModuleGraph, runtime_root: &Path, runtime_entry_path: &Path) -> bool {
    let plan = AppBuildPlan::from_graph(graph);
    let source_files = graph
        .modules
        .values()
        .filter_map(|record| match &record.id {
            crate::graph::ModuleId::File(path)
                if matches!(
                    record.kind,
                    crate::graph::ModuleKind::Script
                        | crate::graph::ModuleKind::Stylesheet
                ) =>
            {
                Some(path)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if source_files.is_empty() {
        return false;
    }

    let newest_source =
        source_files.iter().filter_map(|path| std::fs::metadata(path).ok()?.modified().ok()).max();
    let Some(newest_source) = newest_source else {
        return false;
    };

    source_files.iter().all(|path| {
        let relative = if path.extension().and_then(|ext| ext.to_str()) == Some("css") {
            runtime_static_relative_path(path, &graph.app.root_dir)
        } else {
            runtime_relative_path(
                path,
                &graph.app.root_dir,
                runtime_entry_path.file_name().and_then(|name| name.to_str()),
            )
        };
        let destination = runtime_root.join(relative);
        std::fs::metadata(destination)
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .map(|modified| modified >= newest_source)
            .unwrap_or(false)
    }) && generated_stylesheet_matches_cached(&plan, graph, runtime_root)
}

fn generated_stylesheet_matches_cached(
    plan: &AppBuildPlan,
    graph: &ModuleGraph,
    runtime_root: &Path,
) -> bool {
    let Some(stylesheet_path) = graph.app.stylesheet_path.as_ref() else {
        return true;
    };

    let destination =
        runtime_root.join(runtime_static_relative_path(stylesheet_path, &graph.app.root_dir));
    let expected = plan
        .stylesheet_modules
        .iter()
        .map(std::fs::read_to_string)
        .collect::<Result<Vec<_>, _>>()
        .ok()
        .map(|chunks| chunks.join("\n"));
    let actual = std::fs::read_to_string(destination).ok();

    expected.is_some() && expected == actual
}

fn runtime_destination_for_specifier(
    specifier: &str,
    runtime_root: &Path,
    runtime_entry_path: &Path,
    authored_root: &Path,
) -> PathBuf {
    let entry_relative = runtime_relative_path(
        &authored_root.join(specifier),
        authored_root,
        runtime_entry_path.file_name().and_then(|name| name.to_str()),
    );
    runtime_root.join(entry_relative)
}

fn runtime_relative_path(
    source_path: &Path,
    authored_root: &Path,
    config_file_name: Option<&str>,
) -> PathBuf {
    let Ok(relative) = source_path.strip_prefix(authored_root) else {
        return external_runtime_relative_path(source_path, "js");
    };
    let mut destination = relative.to_path_buf();
    destination.set_extension("js");
    if relative.parent().is_none() {
        if let Some(config_file_name) = config_file_name {
            destination = PathBuf::from(config_file_name);
        }
    }
    destination
}

fn runtime_static_relative_path(source_path: &Path, authored_root: &Path) -> PathBuf {
    source_path.strip_prefix(authored_root).map(Path::to_path_buf).unwrap_or_else(|_| {
        let extension = source_path.extension().and_then(|ext| ext.to_str()).unwrap_or("asset");
        external_runtime_relative_path(source_path, extension)
    })
}

fn external_runtime_relative_path(source_path: &Path, extension: &str) -> PathBuf {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    source_path.hash(&mut hasher);
    let hash = hasher.finish();
    let stem = source_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("module");

    PathBuf::from("__external").join(format!("{stem}-{hash:016x}.{extension}"))
}

fn rewrite_module_for_runtime(
    module: &JavaScriptModule,
    destination: &Path,
    runtime_root: &Path,
    runtime_entry_path: &Path,
    authored_root: &Path,
) -> Result<String, LayoutConfigError> {
    let source_path = authored_root.join(&module.specifier);
    let source_type = SourceType::from_path(&source_path)
        .or_else(|_| SourceType::from_path(Path::new("module.js")))
        .map_err(|_| LayoutConfigError::CompileAuthoredConfig {
            path: source_path.clone(),
            message: "failed to infer source type".into(),
        })?;
    let allocator = oxc::allocator::Allocator::default();
    let parsed = oxc::parser::Parser::new(&allocator, &module.source, source_type).parse();
    if !parsed.errors.is_empty() {
        return Err(LayoutConfigError::CompileAuthoredConfig {
            path: source_path,
            message: "failed to parse compiled module".into(),
        });
    }

    let replacements = module
        .resolved_imports
        .iter()
        .map(|(specifier, target)| {
            if is_virtual_sdk_specifier(target) {
                return (specifier.clone(), target.clone());
            }
            let target_destination = runtime_destination_for_specifier(
                target,
                runtime_root,
                runtime_entry_path,
                authored_root,
            );
            let mut relative =
                relative_path_from(destination.parent().unwrap(), &target_destination)
                    .to_string_lossy()
                    .replace('\\', "/");
            if !relative.starts_with('.') {
                relative = format!("./{relative}");
            }
            (specifier.clone(), relative)
        })
        .collect::<BTreeMap<_, _>>();

    let mut out = String::new();
    let mut cursor = 0usize;
    for statement in &parsed.program.body {
        match statement {
            oxc::ast::ast::Statement::ImportDeclaration(decl) => {
                let span = decl.source.span;
                out.push_str(&module.source[cursor..span.start as usize]);
                out.push_str(
                    &serde_json::to_string(
                        replacements
                            .get(decl.source.value.as_str())
                            .map(String::as_str)
                            .unwrap_or(decl.source.value.as_str()),
                    )
                    .unwrap(),
                );
                cursor = span.end as usize;
            }
            oxc::ast::ast::Statement::ExportNamedDeclaration(decl) => {
                if let Some(source) = &decl.source {
                    let span = source.span;
                    out.push_str(&module.source[cursor..span.start as usize]);
                    out.push_str(
                        &serde_json::to_string(
                            replacements
                                .get(source.value.as_str())
                                .map(String::as_str)
                                .unwrap_or(source.value.as_str()),
                        )
                        .unwrap(),
                    );
                    cursor = span.end as usize;
                }
            }
            oxc::ast::ast::Statement::ExportAllDeclaration(decl) => {
                let span = decl.source.span;
                out.push_str(&module.source[cursor..span.start as usize]);
                out.push_str(
                    &serde_json::to_string(
                        replacements
                            .get(decl.source.value.as_str())
                            .map(String::as_str)
                            .unwrap_or(decl.source.value.as_str()),
                    )
                    .unwrap(),
                );
                cursor = span.end as usize;
            }
            _ => {}
        }
    }
    out.push_str(&module.source[cursor..]);
    Ok(out)
}

fn is_virtual_sdk_specifier(specifier: &str) -> bool {
    specifier.starts_with("@hypreact/sdk/")
}

fn relative_path_from(base: &Path, target: &Path) -> PathBuf {
    let base_components = base.components().collect::<Vec<_>>();
    let target_components = target.components().collect::<Vec<_>>();
    let common_len = base_components
        .iter()
        .zip(target_components.iter())
        .take_while(|(left, right)| left == right)
        .count();

    let mut relative = PathBuf::new();
    for _ in common_len..base_components.len() {
        relative.push("..");
    }
    for component in &target_components[common_len..] {
        relative.push(component.as_os_str());
    }
    relative
}

fn copy_stylesheet_if_stale(
    from: &Path,
    to: &Path,
    force_rebuild: bool,
) -> Result<bool, LayoutConfigError> {
    let source_modified = std::fs::metadata(from)
        .and_then(|metadata| metadata.modified())
        .map_err(|_| LayoutConfigError::ReadConfig { path: from.into() })?;
    if !force_rebuild
        && let Ok(destination_modified) =
            std::fs::metadata(to).and_then(|metadata| metadata.modified())
    {
        if destination_modified >= source_modified {
            return Ok(false);
        }
    }
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|_| LayoutConfigError::ReadConfig { path: parent.to_path_buf() })?;
    }
    std::fs::copy(from, to).map_err(|_| LayoutConfigError::ReadConfig { path: from.into() })?;
    Ok(true)
}

fn prune_stale_runtime_cache(
    runtime_root: &Path,
    expected_paths: &BTreeSet<PathBuf>,
) -> Result<usize, LayoutConfigError> {
    if !runtime_root.exists() {
        return Ok(0);
    }
    prune_stale_runtime_cache_dir(runtime_root, runtime_root, expected_paths)
}

fn prune_stale_runtime_cache_dir(
    runtime_root: &Path,
    dir: &Path,
    expected_paths: &BTreeSet<PathBuf>,
) -> Result<usize, LayoutConfigError> {
    let mut pruned_files = 0usize;
    for entry in std::fs::read_dir(dir)
        .map_err(|_| LayoutConfigError::ReadConfig { path: dir.to_path_buf() })?
    {
        let entry = entry.map_err(|_| LayoutConfigError::ReadConfig { path: dir.to_path_buf() })?;
        let path = entry.path();
        let file_type =
            entry.file_type().map_err(|_| LayoutConfigError::ReadConfig { path: path.clone() })?;

        if file_type.is_dir() {
            pruned_files += prune_stale_runtime_cache_dir(runtime_root, &path, expected_paths)?;
            if path != runtime_root
                && std::fs::read_dir(&path)
                    .map_err(|_| LayoutConfigError::ReadConfig { path: path.clone() })?
                    .next()
                    .is_none()
            {
                std::fs::remove_dir(&path)
                    .map_err(|_| LayoutConfigError::ReadConfig { path: path.clone() })?;
            }
            continue;
        }

        let should_consider =
            matches!(path.extension().and_then(|ext| ext.to_str()), Some("js" | "css"));
        if should_consider && !expected_paths.contains(&path) {
            std::fs::remove_file(&path)
                .map_err(|_| LayoutConfigError::ReadConfig { path: path.clone() })?;
            pruned_files += 1;
        }
    }

    Ok(pruned_files)
}

struct CompiledRuntimeAppOutputs {
    paths: Vec<PathBuf>,
    written_files: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct JsRuntimeCacheUpdate {
    pub rebuilt_files: usize,
    pub copied_stylesheets: usize,
    pub pruned_files: usize,
}

fn decode_config_value(path: &Path, value: &Value) -> Result<Config, LayoutConfigError> {
    let root = expect_object(path, value, "root")?;

    Ok(Config {
        layouts: Vec::new(),
        global_stylesheet_path: None,
        layout_selection: decode_layout_selection(root.get("layouts"), path)?,
    })
}

fn validate_layout_selection(
    path: &Path,
    selection: &LayoutSelectionConfig,
    layouts: &[LayoutDefinition],
) -> Result<(), LayoutConfigError> {
    let known = layouts.iter().map(|layout| layout.name.as_str()).collect::<Vec<_>>();
    let is_known = |name: &str| known.iter().any(|known_name| *known_name == name);

    if let Some(default) = &selection.default {
        if !is_known(default) {
            return Err(LayoutConfigError::DecodeAuthoredConfig {
                path: path.to_path_buf(),
                message: format!(
                    "selected layout `{default}` is not defined by discovered layout modules"
                ),
            });
        }
    }

    for layout in selection.per_workspace.iter().chain(selection.per_monitor.values()) {
        if !is_known(layout) {
            return Err(LayoutConfigError::DecodeAuthoredConfig {
                path: path.to_path_buf(),
                message: format!(
                    "selected layout `{layout}` is not defined by discovered layout modules"
                ),
            });
        }
    }

    Ok(())
}

fn decode_layout_selection(
    value: Option<&Value>,
    path: &Path,
) -> Result<LayoutSelectionConfig, LayoutConfigError> {
    let Some(value) = value else {
        return Ok(LayoutSelectionConfig::default());
    };
    let object = expect_object(path, value, "root.layouts")?;
    let per_monitor = match object.get("per_monitor") {
        Some(value) => {
            let map = expect_object(path, value, "root.layouts.per_monitor")?;
            let mut out = BTreeMap::new();
            for (name, value) in map {
                out.insert(
                    name.clone(),
                    expect_string(path, value, &format!("root.layouts.per_monitor.{name}"))?
                        .to_owned(),
                );
            }
            out
        }
        None => BTreeMap::new(),
    };

    Ok(LayoutSelectionConfig {
        default: decode_optional_string(object.get("default"), path, "root.layouts.default")?,
        per_workspace: decode_string_array(
            object.get("per_workspace"),
            path,
            "root.layouts.per_workspace",
        )?,
        per_monitor,
    })
}


fn decode_optional_string(
    value: Option<&Value>,
    path: &Path,
    field: &str,
) -> Result<Option<String>, LayoutConfigError> {
    value.map(|value| expect_string(path, value, field).map(str::to_owned)).transpose()
}

fn decode_string_array(
    value: Option<&Value>,
    path: &Path,
    field: &str,
) -> Result<Vec<String>, LayoutConfigError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let items = expect_array(path, value, field)?;
    items.iter().map(|value| expect_string(path, value, field).map(str::to_owned)).collect()
}

fn expect_object<'a>(
    path: &Path,
    value: &'a Value,
    field: &str,
) -> Result<&'a serde_json::Map<String, Value>, LayoutConfigError> {
    value.as_object().ok_or_else(|| LayoutConfigError::DecodeAuthoredConfig {
        path: path.to_path_buf(),
        message: format!("expected object at {field}"),
    })
}

fn expect_array<'a>(
    path: &Path,
    value: &'a Value,
    field: &str,
) -> Result<&'a Vec<Value>, LayoutConfigError> {
    value.as_array().ok_or_else(|| LayoutConfigError::DecodeAuthoredConfig {
        path: path.to_path_buf(),
        message: format!("expected array at {field}"),
    })
}

fn expect_string<'a>(
    path: &Path,
    value: &'a Value,
    field: &str,
) -> Result<&'a str, LayoutConfigError> {
    value.as_str().ok_or_else(|| LayoutConfigError::DecodeAuthoredConfig {
        path: path.to_path_buf(),
        message: format!("expected string at {field}"),
    })
}


#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use crate::decode_runtime_graph_payload;

    use super::*;

    fn unique_root(name: &str) -> PathBuf {
        let unique =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("hypreact-config-authored-{name}-{unique}"))
    }

    #[test]
    fn loads_authored_config_and_prepared_layouts() {
        let root = unique_root("project");
        fs::create_dir_all(root.join("layouts/master-stack")).unwrap();
        fs::create_dir_all(root.join("config")).unwrap();
        fs::write(root.join("index.css"), "window { appearance: none; }").unwrap();
        fs::write(
            root.join("config.ts"),
            r#"
                import type { HypreactConfig } from "@hypreact/sdk/config";
                import { layouts } from "./config/layouts";

                export default {
                  layouts,
                } satisfies HypreactConfig;
            "#,
        )
        .unwrap();
        fs::write(
            root.join("config/layouts.ts"),
            r#"
                export const layouts = {
                  default: "master-stack",
                  per_workspace: ["master-stack", "master-stack"],
                  per_monitor: { "eDP-1": "master-stack" },
                };
            "#,
        )
        .unwrap();
        fs::write(
            root.join("layouts/master-stack/index.tsx"),
            r#"
                import "./index.css";
                export default function layout() {
                  return { type: "workspace", children: [] };
                }
            "#,
        )
        .unwrap();
        fs::write(root.join("layouts/master-stack/index.css"), ".master {}").unwrap();

        let config = load_authored_config(root.join("config.ts")).unwrap();

        assert_eq!(config.layout_selection.default.as_deref(), Some("master-stack"));
        assert_eq!(config.layouts.len(), 1);
        assert_eq!(config.layouts[0].module, "layouts/master-stack/index.tsx");
        let runtime_graph =
            decode_runtime_graph_payload(config.layouts[0].runtime_cache_payload.as_ref().unwrap())
                .unwrap();
        assert_eq!(runtime_graph.entry, "layouts/master-stack/index.tsx");
        assert!(
            runtime_graph
                .modules
                .iter()
                .any(|module| module.source.contains("export default function layout"))
        );
        let stylesheet_path = config.layouts[0].stylesheet_path.clone().unwrap();
        assert!(stylesheet_path.ends_with("layouts/master-stack/index.css"));
    }

    #[test]
    fn refresh_prepared_config_skips_rewriting_fresh_outputs() {
        let root = unique_root("cache-sync");
        let cache_root = root.join("runtime-cache");
        fs::create_dir_all(root.join("layouts/master-stack")).unwrap();
        fs::write(
            root.join("config.ts"),
            r#"
                export default {
                  layouts: { default: "master-stack" },
                };
            "#,
        )
        .unwrap();
        fs::write(
            root.join("layouts/master-stack/index.ts"),
            r#"
                export default function layout() {
                  return { type: "workspace", children: [] };
                }
            "#,
        )
        .unwrap();
        fs::write(root.join("layouts/master-stack/index.css"), ".master {}").unwrap();

        let runtime_entry = cache_root.join("config.js");
        rebuild_prepared_config(root.join("config.ts"), &runtime_entry).unwrap();
        fs::write(cache_root.join("stale.js"), "export default {};").unwrap();
        fs::write(cache_root.join("stale.css"), ".stale {}").unwrap();
        let first_modified = fs::metadata(&runtime_entry).unwrap().modified().unwrap();

        std::thread::sleep(std::time::Duration::from_millis(20));
        refresh_prepared_config(root.join("config.ts"), &runtime_entry).unwrap();
        let second_modified = fs::metadata(&runtime_entry).unwrap().modified().unwrap();

        assert_eq!(first_modified, second_modified);
        assert!(cache_root.join("layouts/master-stack/index.js").exists());
        assert!(!cache_root.join("stale.js").exists());
        assert!(!cache_root.join("stale.css").exists());
    }

    #[test]
    fn load_prepared_config_preserves_virtual_commands_imports() {
        let root = unique_root("prepared-virtual-commands");
        let cache_root = root.join("runtime-cache");
        fs::create_dir_all(root.join("layouts/master-stack")).unwrap();
        fs::write(
            root.join("config.ts"),
            r#"
                import type { HypreactConfig } from "@hypreact/sdk/config";

                export default {
                  layouts: { default: "master-stack" },
                } satisfies HypreactConfig;
            "#,
        )
        .unwrap();
        fs::write(
            root.join("layouts/master-stack/index.ts"),
            r#"
                export default function layout() {
                  return { type: "workspace", children: [] };
                }
            "#,
        )
        .unwrap();

        let runtime_entry = cache_root.join("config.js");
        rebuild_prepared_config(root.join("config.ts"), &runtime_entry).unwrap();

        let config = load_prepared_config(&runtime_entry).unwrap();
        assert_eq!(config.layout_selection.default.as_deref(), Some("master-stack"));
    }

}
