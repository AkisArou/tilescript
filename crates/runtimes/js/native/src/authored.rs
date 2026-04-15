use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use oxc::span::SourceType;
use serde_json::Value;
use tracing::debug;

use hypreact_config::model::{Config, LayoutConfigError, LayoutDefinition};
use hypreact_runtime_js_core::compile::{AppBuildPlan, compile_app, compiled_app_to_module_graph};
use hypreact_runtime_js_core::{decode_config_value, validate_layout_selection};
use hypreact_runtime_js_core::graph::{
    DiscoveredApp, ModuleGraph, ModuleGraphBuilder, discover_project_apps,
};
use hypreact_runtime_js_core::encode_runtime_graph_payload;

use crate::evaluate_entry_export_to_json;

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

    validate_layout_selection(path, config.default_layout.as_deref(), &config.layout_rules, &layout_defs)?;
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
    graph: &hypreact_runtime_js_core::JavaScriptModuleGraph,
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
            hypreact_runtime_js_core::graph::ModuleId::File(path) => match record.kind {
                hypreact_runtime_js_core::graph::ModuleKind::Script => {
                    Some(runtime_root.join(runtime_relative_path(
                        path,
                        &graph.app.root_dir,
                        runtime_entry_path.file_name().and_then(|name| name.to_str()),
                    )))
                }
                hypreact_runtime_js_core::graph::ModuleKind::Stylesheet => {
                    Some(runtime_root.join(runtime_static_relative_path(path, &graph.app.root_dir)))
                }
                hypreact_runtime_js_core::graph::ModuleKind::Virtual => None,
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
            module,
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
            hypreact_runtime_js_core::graph::ModuleId::File(path)
                if matches!(
                    record.kind,
                    hypreact_runtime_js_core::graph::ModuleKind::Script
                        | hypreact_runtime_js_core::graph::ModuleKind::Stylesheet
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
    module: &hypreact_runtime_js_core::JavaScriptModule,
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
