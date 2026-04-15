use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::mem;
use std::path::{Path, PathBuf};

use oxc::CompilerInterface;
use oxc::allocator::Allocator;
use oxc::ast::ast::Statement;
use oxc::codegen::CodegenReturn;
use oxc::diagnostics::OxcDiagnostic;
use oxc::parser::Parser;
use oxc::span::{GetSpan, SourceType};
use oxc::transformer::{JsxRuntime, TransformOptions};

use crate::module_graph::{JavaScriptModule, JavaScriptModuleGraph};
use crate::virtual_modules::source_for_virtual_module;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum SourceModuleId {
    File(PathBuf),
    Virtual(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportedModuleKind {
    Script,
    Stylesheet,
    Virtual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportedModule {
    specifier: String,
    kind: ImportedModuleKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedImport {
    specifier: String,
    kind: ImportedModuleKind,
    module_id: SourceModuleId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceModuleKind {
    Script,
    Stylesheet,
    Virtual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceModuleRecord {
    kind: SourceModuleKind,
    imports: Vec<ImportedModule>,
    resolved_imports: Vec<ResolvedImport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceModuleGraph {
    root_dir: PathBuf,
    entry_path: PathBuf,
    modules: BTreeMap<SourceModuleId, SourceModuleRecord>,
    order: Vec<SourceModuleId>,
}

pub fn compile_source_bundle_to_module_graph(
    root_dir: &Path,
    entry_path: &Path,
    sources: &BTreeMap<PathBuf, String>,
) -> Result<JavaScriptModuleGraph, String> {
    let graph = build_source_graph(root_dir, entry_path, sources)?;
    let mut modules = Vec::new();
    let mut compiled_scripts = BTreeMap::new();

    for module_id in &graph.order {
        let SourceModuleId::File(path) = module_id else {
            continue;
        };

        if path.extension().and_then(|extension| extension.to_str()) == Some("css") {
            continue;
        }

        let source = sources
            .get(path)
            .ok_or_else(|| format!("source for {} is unavailable", path.display()))?;
        let compiled = compile_script(path, source)?;
        compiled_scripts.insert(path.clone(), compiled);
    }

    for module_id in &graph.order {
        let Some(record) = graph.modules.get(module_id) else {
            continue;
        };
        if !matches!(record.kind, SourceModuleKind::Script | SourceModuleKind::Virtual) {
            continue;
        }

        let source = match module_id {
            SourceModuleId::File(path) => compiled_scripts
                .get(path)
                .cloned()
                .ok_or_else(|| format!("compiled output for {} is unavailable", path.display()))?,
            SourceModuleId::Virtual(specifier) => read_virtual_module_source(specifier)?,
        };
        let mut resolved_imports = record
            .resolved_imports
            .iter()
            .filter(|import| !matches!(import.kind, ImportedModuleKind::Stylesheet))
            .map(|import| {
                (import.specifier.clone(), module_key(&graph.root_dir, &import.module_id))
            })
            .collect::<BTreeMap<_, _>>();

        if matches!(module_id, SourceModuleId::File(path) if matches!(path.extension().and_then(|extension| extension.to_str()), Some("tsx" | "jsx")))
        {
            resolved_imports.insert(
                "@hypreact/sdk/jsx-runtime".to_string(),
                "@hypreact/sdk/jsx-runtime".to_string(),
            );
        }

        modules.push(JavaScriptModule {
            specifier: module_key(&graph.root_dir, module_id),
            source,
            resolved_imports,
        });
    }

    if !modules.iter().any(|module| module.specifier == "@hypreact/sdk/jsx-runtime") {
        modules.push(JavaScriptModule {
            specifier: "@hypreact/sdk/jsx-runtime".to_string(),
            source: read_virtual_module_source("@hypreact/sdk/jsx-runtime")?,
            resolved_imports: BTreeMap::new(),
        });
    }

    Ok(JavaScriptModuleGraph {
        entry: module_key(&graph.root_dir, &SourceModuleId::File(graph.entry_path.clone())),
        modules,
    })
}

fn build_source_graph(
    root_dir: &Path,
    entry_path: &Path,
    sources: &BTreeMap<PathBuf, String>,
) -> Result<SourceModuleGraph, String> {
    let mut modules = BTreeMap::new();
    let mut order = Vec::new();
    let mut pending = VecDeque::from([SourceModuleId::File(entry_path.to_path_buf())]);
    let mut visited = BTreeSet::new();

    while let Some(module_id) = pending.pop_front() {
        if !visited.insert(module_id.clone()) {
            continue;
        }

        let mut record = load_module_record(&module_id, sources)?;
        for import in &record.imports {
            let resolved = resolve_import(&module_id, import, sources, root_dir)?;
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

    Ok(SourceModuleGraph {
        root_dir: root_dir.to_path_buf(),
        entry_path: entry_path.to_path_buf(),
        modules,
        order,
    })
}

fn load_module_record(
    module_id: &SourceModuleId,
    sources: &BTreeMap<PathBuf, String>,
) -> Result<SourceModuleRecord, String> {
    match module_id {
        SourceModuleId::Virtual(_) => Ok(SourceModuleRecord {
            kind: SourceModuleKind::Virtual,
            imports: Vec::new(),
            resolved_imports: Vec::new(),
        }),
        SourceModuleId::File(path) => {
            let source = sources
                .get(path)
                .ok_or_else(|| format!("missing source file {}", path.display()))?;
            let kind = if path.extension().and_then(|ext| ext.to_str()) == Some("css") {
                SourceModuleKind::Stylesheet
            } else {
                SourceModuleKind::Script
            };
            let imports = if kind == SourceModuleKind::Script {
                parse_imports(path, source)?
            } else {
                Vec::new()
            };

            Ok(SourceModuleRecord { kind, imports, resolved_imports: Vec::new() })
        }
    }
}

fn parse_imports(path: &Path, source: &str) -> Result<Vec<ImportedModule>, String> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path)
        .map_err(|_| format!("failed to infer source type for {}", path.display()))?;
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return Err(format!("module {} has parse errors", path.display()));
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

    ImportedModule { specifier: specifier.to_string(), kind }
}

fn resolve_import(
    from: &SourceModuleId,
    import: &ImportedModule,
    sources: &BTreeMap<PathBuf, String>,
    root_dir: &Path,
) -> Result<SourceModuleId, String> {
    if matches!(import.kind, ImportedModuleKind::Virtual) {
        return Ok(SourceModuleId::Virtual(import.specifier.clone()));
    }

    let from_path = match from {
        SourceModuleId::File(path) => path,
        SourceModuleId::Virtual(_) => return Ok(SourceModuleId::Virtual(import.specifier.clone())),
    };

    if !import.specifier.starts_with('.') && !import.specifier.starts_with('/') {
        return Err(format!(
            "unsupported external import {} from {}",
            import.specifier,
            from_path.display()
        ));
    }

    let resolved_path =
        resolve_source_path(from_path.parent().unwrap_or(root_dir), &import.specifier, sources)?;

    Ok(SourceModuleId::File(resolved_path))
}

fn resolve_source_path(
    from_dir: &Path,
    specifier: &str,
    sources: &BTreeMap<PathBuf, String>,
) -> Result<PathBuf, String> {
    let base = if specifier.starts_with('/') {
        PathBuf::from(specifier)
    } else {
        normalize_path(&from_dir.join(specifier))
    };

    for candidate in resolution_candidates(&base) {
        if sources.contains_key(&candidate) {
            return Ok(candidate);
        }
    }

    Err(format!("failed to resolve {} from {}", specifier, from_dir.display()))
}

fn resolution_candidates(base: &Path) -> Vec<PathBuf> {
    const EXTENSIONS: [&str; 6] = ["ts", "tsx", "js", "jsx", "json", "css"];

    let mut candidates = Vec::new();
    let base = normalize_path(base);
    let has_extension = base.extension().is_some();

    candidates.push(base.clone());
    if !has_extension {
        for extension in EXTENSIONS {
            candidates.push(base.with_extension(extension));
        }
        for extension in EXTENSIONS {
            candidates.push(base.join(format!("index.{extension}")));
        }
    }

    candidates
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn compile_script(path: &Path, source: &str) -> Result<String, String> {
    let source_type = SourceType::from_path(path)
        .map_err(|_| format!("failed to infer source type for {}", path.display()))?;
    let injected_source =
        if matches!(path.extension().and_then(|extension| extension.to_str()), Some("tsx" | "jsx"))
        {
            format!("import {{ sp, Fragment }} from \"@hypreact/sdk/jsx-runtime\";\n{source}")
        } else {
            source.to_string()
        };
    let mut compiler = AppScriptCompiler::default();
    let compiled = compiler
        .execute(&injected_source, source_type, path)
        .map_err(|_| format!("failed to transpile {}", path.display()))?;

    strip_stylesheet_imports(path, &compiled)
}

#[derive(Default)]
struct AppScriptCompiler {
    printed: String,
    errors: Vec<OxcDiagnostic>,
    transform: TransformOptions,
}

impl AppScriptCompiler {
    fn execute(
        &mut self,
        source_text: &str,
        source_type: SourceType,
        source_path: &Path,
    ) -> Result<String, Vec<OxcDiagnostic>> {
        if self.transform.jsx.pragma.is_none() {
            self.transform.jsx.runtime = JsxRuntime::Classic;
            self.transform.jsx.pragma = Some("sp".into());
            self.transform.jsx.pragma_frag = Some("Fragment".into());
        }
        self.compile(source_text, source_type, source_path);
        if self.errors.is_empty() {
            Ok(mem::take(&mut self.printed))
        } else {
            Err(mem::take(&mut self.errors))
        }
    }
}

impl CompilerInterface for AppScriptCompiler {
    fn handle_errors(&mut self, errors: Vec<OxcDiagnostic>) {
        self.errors.extend(errors);
    }

    fn transform_options(&self) -> Option<&TransformOptions> {
        Some(&self.transform)
    }

    fn after_codegen(&mut self, ret: CodegenReturn) {
        self.printed = ret.code;
    }
}

fn strip_stylesheet_imports(path: &Path, source: &str) -> Result<String, String> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path)
        .map_err(|_| format!("failed to infer source type for {}", path.display()))?;
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return Err(format!("module {} has parse errors", path.display()));
    }

    let mut out = String::new();
    let mut cursor = 0usize;
    for statement in &parsed.program.body {
        let span = statement.span();
        let start = span.start as usize;
        let end = span.end as usize;
        out.push_str(&source[cursor..start]);
        match statement {
            Statement::ImportDeclaration(decl) if decl.source.value.as_str().ends_with(".css") => {}
            _ => out.push_str(&source[start..end]),
        }
        cursor = end;
    }
    out.push_str(&source[cursor..]);

    Ok(out)
}

fn read_virtual_module_source(specifier: &str) -> Result<String, String> {
    source_for_virtual_module(specifier)
        .map(str::to_string)
        .ok_or_else(|| format!("unsupported virtual module {specifier}"))
}

fn module_key(root_dir: &Path, module_id: &SourceModuleId) -> String {
    match module_id {
        SourceModuleId::File(path) => {
            path.strip_prefix(root_dir).unwrap_or(path).to_string_lossy().replace('\\', "/")
        }
        SourceModuleId::Virtual(specifier) => specifier.clone(),
    }
}

fn is_virtual_sdk_specifier(specifier: &str) -> bool {
    specifier.starts_with("@hypreact/sdk/")
}
