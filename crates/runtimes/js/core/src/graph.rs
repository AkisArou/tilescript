use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use oxc::allocator::Allocator;
use oxc::ast::ast::Statement;
use oxc::parser::Parser;
use oxc::span::SourceType;
use oxc_resolver::{ResolveOptions, Resolver};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AppKind {
    Config,
    Layout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredApp {
    pub kind: AppKind,
    pub name: String,
    pub entry_path: PathBuf,
    pub root_dir: PathBuf,
    pub stylesheet_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredProject {
    pub config_app: DiscoveredApp,
    pub layout_apps: Vec<DiscoveredApp>,
    pub global_stylesheet_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportedModuleKind {
    Script,
    Stylesheet,
    Virtual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedModule {
    pub specifier: String,
    pub kind: ImportedModuleKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedImport {
    pub specifier: String,
    pub kind: ImportedModuleKind,
    pub module_id: ModuleId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleKind {
    Script,
    Stylesheet,
    Virtual,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ModuleId {
    File(PathBuf),
    Virtual(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleRecord {
    pub id: ModuleId,
    pub kind: ModuleKind,
    pub imports: Vec<ImportedModule>,
    pub resolved_imports: Vec<ResolvedImport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleGraph {
    pub app: DiscoveredApp,
    pub modules: BTreeMap<ModuleId, ModuleRecord>,
    pub order: Vec<ModuleId>,
}

#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error("config entry `{path}` does not exist")]
    MissingConfigEntry { path: PathBuf },
    #[error("config entry `{path}` must be a file")]
    InvalidConfigEntry { path: PathBuf },
    #[error("failed to read module `{path}`")]
    ReadModule { path: PathBuf },
    #[error("failed to infer source type for `{path}`")]
    UnsupportedSourceType { path: PathBuf },
    #[error("module `{path}` has parse errors")]
    ParseModule { path: PathBuf },
    #[error("failed to resolve `{specifier}` from `{from}`: {message}")]
    Resolve { from: PathBuf, specifier: String, message: String },
    #[error("unsupported external import `{specifier}` from `{from}`")]
    UnsupportedImport { from: PathBuf, specifier: String },
}

pub fn discover_project_apps(
    config_entry: impl AsRef<Path>,
) -> Result<DiscoveredProject, GraphError> {
    let config_entry = config_entry.as_ref();
    if !config_entry.exists() {
        return Err(GraphError::MissingConfigEntry { path: config_entry.to_path_buf() });
    }
    if !config_entry.is_file() {
        return Err(GraphError::InvalidConfigEntry { path: config_entry.to_path_buf() });
    }

    let root_dir =
        config_entry.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));

    let config_app = DiscoveredApp {
        kind: AppKind::Config,
        name: "config".into(),
        entry_path: config_entry.to_path_buf(),
        root_dir: root_dir.clone(),
        stylesheet_path: root_dir.join("index.css").exists().then(|| root_dir.join("index.css")),
    };

    let mut layout_apps = Vec::new();
    let layouts_dir = root_dir.join("layouts");
    if layouts_dir.exists() {
        for entry in std::fs::read_dir(&layouts_dir)
            .map_err(|_| GraphError::ReadModule { path: layouts_dir.clone() })?
        {
            let entry = entry.map_err(|_| GraphError::ReadModule { path: layouts_dir.clone() })?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            if let Some(entry_path) = discover_layout_entry(&path) {
                let name =
                    path.file_name().and_then(|name| name.to_str()).unwrap_or_default().to_owned();
                let stylesheet_path = path.join("index.css");
                layout_apps.push(DiscoveredApp {
                    kind: AppKind::Layout,
                    name,
                    entry_path,
                    root_dir: root_dir.clone(),
                    stylesheet_path: stylesheet_path.exists().then_some(stylesheet_path),
                });
            }
        }
    }

    layout_apps.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(DiscoveredProject {
        config_app,
        layout_apps,
        global_stylesheet_path: discover_global_stylesheet(&root_dir),
    })
}

fn discover_global_stylesheet(root_dir: &Path) -> Option<PathBuf> {
    let path = root_dir.join("index.css");
    path.exists().then_some(path)
}

fn discover_layout_entry(layout_dir: &Path) -> Option<PathBuf> {
    ["index.ts", "index.tsx", "index.js", "index.jsx"]
        .into_iter()
        .map(|name| layout_dir.join(name))
        .find(|path| path.exists())
}

#[derive(Debug)]
pub struct ModuleGraphBuilder {
    resolver: Resolver,
}

impl Default for ModuleGraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleGraphBuilder {
    pub fn new() -> Self {
        let options = ResolveOptions {
            condition_names: vec!["node".into(), "import".into()],
            extensions: vec![
                ".ts".into(),
                ".tsx".into(),
                ".js".into(),
                ".jsx".into(),
                ".json".into(),
                ".css".into(),
            ],
            main_files: vec!["index".into()],
            ..ResolveOptions::default()
        };

        Self { resolver: Resolver::new(options) }
    }

    pub fn build(&self, app: &DiscoveredApp) -> Result<ModuleGraph, GraphError> {
        let mut modules = BTreeMap::new();
        let mut order = Vec::new();
        let mut pending = VecDeque::from([ModuleId::File(app.entry_path.clone())]);
        let mut visited = BTreeSet::new();

        while let Some(module_id) = pending.pop_front() {
            if !visited.insert(module_id.clone()) {
                continue;
            }

            let mut record = self.load_module(&module_id)?;
            for import in &record.imports {
                let resolved = self.resolve_import(&module_id, import)?;
                record.resolved_imports.push(ResolvedImport {
                    specifier: import.specifier.clone(),
                    kind: import.kind,
                    module_id: resolved.clone(),
                });
                if !visited.contains(&resolved) {
                    pending.push_back(resolved);
                }
            }
            order.push(module_id.clone());
            modules.insert(module_id, record);
        }

        Ok(ModuleGraph { app: app.clone(), modules, order })
    }

    fn load_module(&self, module_id: &ModuleId) -> Result<ModuleRecord, GraphError> {
        match module_id {
            ModuleId::Virtual(name) => Ok(ModuleRecord {
                id: ModuleId::Virtual(name.clone()),
                kind: ModuleKind::Virtual,
                imports: Vec::new(),
                resolved_imports: Vec::new(),
            }),
            ModuleId::File(path) => {
                let source = std::fs::read_to_string(path)
                    .map_err(|_| GraphError::ReadModule { path: path.clone() })?;

                let kind = if path.extension().and_then(|ext| ext.to_str()) == Some("css") {
                    ModuleKind::Stylesheet
                } else {
                    ModuleKind::Script
                };

                let imports = if kind == ModuleKind::Script {
                    parse_imports(path, &source)?
                } else {
                    Vec::new()
                };

                Ok(ModuleRecord {
                    id: ModuleId::File(path.clone()),
                    kind,
                    imports,
                    resolved_imports: Vec::new(),
                })
            }
        }
    }

    fn resolve_import(
        &self,
        from: &ModuleId,
        import: &ImportedModule,
    ) -> Result<ModuleId, GraphError> {
        if matches!(import.kind, ImportedModuleKind::Virtual)
            || is_virtual_sdk_specifier(&import.specifier)
        {
            return Ok(ModuleId::Virtual(import.specifier.clone()));
        }

        let from_path = match from {
            ModuleId::File(path) => path,
            ModuleId::Virtual(_) => {
                return Ok(ModuleId::Virtual(import.specifier.clone()));
            }
        };
        if !import.specifier.starts_with('.') && !import.specifier.starts_with('/') {
            return Err(GraphError::UnsupportedImport {
                from: from_path.clone(),
                specifier: import.specifier.clone(),
            });
        }
        let from_dir = from_path.parent().unwrap_or_else(|| Path::new("/"));

        let resolution = self.resolver.resolve(from_dir, &import.specifier).map_err(
            |error: oxc_resolver::ResolveError| GraphError::Resolve {
                from: from_path.clone(),
                specifier: import.specifier.clone(),
                message: error.to_string(),
            },
        )?;

        Ok(ModuleId::File(resolution.full_path().to_path_buf()))
    }
}

fn parse_imports(path: &Path, source: &str) -> Result<Vec<ImportedModule>, GraphError> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path)
        .map_err(|_| GraphError::UnsupportedSourceType { path: path.to_path_buf() })?;
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return Err(GraphError::ParseModule { path: path.to_path_buf() });
    }

    let mut imports = Vec::new();
    for statement in &parsed.program.body {
        match statement {
            Statement::ImportDeclaration(decl) => {
                imports.push(classify_import_specifier(decl.source.value.as_str()));
            }
            Statement::ExportNamedDeclaration(decl) => {
                if let Some(source) = &decl.source {
                    imports.push(classify_import_specifier(source.value.as_str()));
                }
            }
            Statement::ExportAllDeclaration(decl) => {
                imports.push(classify_import_specifier(decl.source.value.as_str()));
            }
            _ => {}
        }
    }

    Ok(imports)
}

fn classify_import_specifier(specifier: &str) -> ImportedModule {
    let kind = if is_virtual_sdk_specifier(specifier) {
        ImportedModuleKind::Virtual
    } else if specifier.ends_with(".css") {
        ImportedModuleKind::Stylesheet
    } else {
        ImportedModuleKind::Script
    };

    ImportedModule { specifier: specifier.to_owned(), kind }
}

fn is_virtual_sdk_specifier(specifier: &str) -> bool {
    specifier.starts_with("@hypreact/sdk/")
}
