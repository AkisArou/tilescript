use std::collections::BTreeMap;
use std::mem;
use std::path::Path;
use std::path::PathBuf;

use oxc::CompilerInterface;
use oxc::allocator::Allocator;
use oxc::ast::ast::Statement;
use oxc::codegen::CodegenReturn;
use oxc::diagnostics::OxcDiagnostic;
use oxc::parser::Parser;
use oxc::span::GetSpan;
use oxc::span::SourceType;
use oxc::transformer::{JsxRuntime, TransformOptions};

use crate::graph::{ImportedModuleKind, ModuleGraph, ModuleId, ModuleKind};
use crate::module_graph::{JavaScriptModule, JavaScriptModuleGraph};
use crate::virtual_modules::source_for_virtual_module;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppBuildPlan {
    pub script_modules: Vec<PathBuf>,
    pub stylesheet_modules: Vec<PathBuf>,
    pub virtual_modules: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledApp {
    pub scripts: Vec<CompiledScriptModule>,
    pub stylesheet: String,
    pub virtual_modules: Vec<CompiledVirtualModule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledScriptModule {
    pub path: PathBuf,
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledVirtualModule {
    pub specifier: String,
    pub code: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CompileError {
    #[error("failed to read script module `{path}`")]
    ReadScript { path: PathBuf },
    #[error("failed to infer source type for `{path}`")]
    UnsupportedSourceType { path: PathBuf },
    #[error("failed to transpile `{path}`")]
    Transpile { path: PathBuf },
    #[error("failed to read stylesheet `{path}`")]
    ReadStylesheet { path: PathBuf },
    #[error("unsupported virtual module `{specifier}`")]
    UnsupportedVirtualModule { specifier: String },
    #[error("failed to read virtual module source `{path}`")]
    ReadVirtualModule { path: PathBuf },
    #[error("compiled output for module `{module}` is unavailable")]
    MissingCompiledModule { module: String },
}

impl AppBuildPlan {
    pub fn from_graph(graph: &ModuleGraph) -> Self {
        let mut script_modules = Vec::new();
        let mut stylesheet_modules = Vec::new();
        let mut virtual_modules = Vec::new();
        let mut needs_jsx_runtime = false;

        if let Some(stylesheet_path) = graph.app.stylesheet_path.as_ref() {
            stylesheet_modules.push(stylesheet_path.clone());
        }

        for module_id in &graph.order {
            let Some(module) = graph.modules.get(module_id) else {
                continue;
            };

            if module.kind != ModuleKind::Stylesheet {
                continue;
            }

            let ModuleId::File(path) = &module.id else {
                continue;
            };

            if stylesheet_modules.iter().any(|existing| existing == path) {
                continue;
            }

            stylesheet_modules.push(path.clone());
        }

        for module_id in &graph.order {
            let Some(module) = graph.modules.get(module_id) else {
                continue;
            };

            match (&module.id, module.kind) {
                (ModuleId::File(path), ModuleKind::Script) => {
                    if matches!(
                        path.extension().and_then(|extension| extension.to_str()),
                        Some("tsx" | "jsx")
                    ) {
                        needs_jsx_runtime = true;
                    }
                    script_modules.push(path.clone())
                }
                (ModuleId::File(_), ModuleKind::Stylesheet) => {}
                (ModuleId::Virtual(name), ModuleKind::Virtual) => {
                    virtual_modules.push(name.clone())
                }
                _ => {}
            }
        }

        if needs_jsx_runtime
            && !virtual_modules.iter().any(|name| name == "@hypreact/sdk/jsx-runtime")
        {
            virtual_modules.push("@hypreact/sdk/jsx-runtime".into());
        }

        Self { script_modules, stylesheet_modules, virtual_modules }
    }
}

struct AppScriptCompiler {
    printed: String,
    errors: Vec<OxcDiagnostic>,
    transform: TransformOptions,
}

impl Default for AppScriptCompiler {
    fn default() -> Self {
        let mut transform = TransformOptions::default();
        transform.jsx.runtime = JsxRuntime::Classic;
        transform.jsx.pragma = Some("sp".into());
        transform.jsx.pragma_frag = Some("Fragment".into());

        Self { printed: String::new(), errors: Vec::new(), transform }
    }
}

impl AppScriptCompiler {
    fn execute(
        &mut self,
        source_text: &str,
        source_type: SourceType,
        source_path: &std::path::Path,
    ) -> Result<String, Vec<OxcDiagnostic>> {
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

pub fn compile_app(plan: &AppBuildPlan) -> Result<CompiledApp, CompileError> {
    let mut scripts = Vec::new();
    for path in &plan.script_modules {
        let source = std::fs::read_to_string(path)
            .map_err(|_| CompileError::ReadScript { path: path.clone() })?;
        let source_type = SourceType::from_path(path)
            .map_err(|_| CompileError::UnsupportedSourceType { path: path.clone() })?;
        let mut compiler = AppScriptCompiler::default();
        let injected_source = if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("tsx" | "jsx")
        ) {
            format!("import {{ sp, Fragment }} from \"@hypreact/sdk/jsx-runtime\";\n{source}")
        } else {
            source.clone()
        };
        let code = compiler
            .execute(&injected_source, source_type, path)
            .map_err(|_| CompileError::Transpile { path: path.clone() })?;
        let code = strip_stylesheet_imports(path, &code)?;
        scripts.push(CompiledScriptModule { path: path.clone(), code });
    }

    let mut stylesheet_chunks = Vec::new();
    for path in &plan.stylesheet_modules {
        let source = std::fs::read_to_string(path)
            .map_err(|_| CompileError::ReadStylesheet { path: path.clone() })?;
        stylesheet_chunks.push(source);
    }

    let virtual_modules = plan
        .virtual_modules
        .iter()
        .map(|specifier| {
            Ok(CompiledVirtualModule {
                specifier: specifier.clone(),
                code: read_virtual_module_source(specifier)?,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;

    Ok(CompiledApp { scripts, stylesheet: stylesheet_chunks.join("\n"), virtual_modules })
}

pub fn compiled_app_to_module_graph(
    graph: &ModuleGraph,
    compiled: &CompiledApp,
) -> Result<JavaScriptModuleGraph, CompileError> {
    let mut modules = Vec::new();
    let compiled_scripts = compiled
        .scripts
        .iter()
        .map(|module| (ModuleId::File(module.path.clone()), module))
        .collect::<BTreeMap<_, _>>();
    let compiled_virtuals = compiled
        .virtual_modules
        .iter()
        .map(|module| (ModuleId::Virtual(module.specifier.clone()), module))
        .collect::<BTreeMap<_, _>>();

    for module_id in &graph.order {
        let Some(record) = graph.modules.get(module_id) else {
            continue;
        };
        if !matches!(record.kind, ModuleKind::Script | ModuleKind::Virtual) {
            continue;
        }

        let source = match module_id {
            ModuleId::File(_) => compiled_scripts
                .get(module_id)
                .ok_or_else(|| CompileError::MissingCompiledModule {
                    module: module_key(&graph.app.root_dir, module_id),
                })?
                .code
                .clone(),
            ModuleId::Virtual(_) => compiled_virtuals
                .get(module_id)
                .ok_or_else(|| CompileError::MissingCompiledModule {
                    module: module_key(&graph.app.root_dir, module_id),
                })?
                .code
                .clone(),
        };

        let mut resolved_imports = record
            .resolved_imports
            .iter()
            .filter(|import| !matches!(import.kind, ImportedModuleKind::Stylesheet))
            .map(|import| {
                (import.specifier.clone(), module_key(&graph.app.root_dir, &import.module_id))
            })
            .collect::<BTreeMap<_, _>>();

        if matches!(module_id, ModuleId::File(path) if matches!(path.extension().and_then(|extension| extension.to_str()), Some("tsx" | "jsx")))
            && compiled_virtuals
                .contains_key(&ModuleId::Virtual("@hypreact/sdk/jsx-runtime".into()))
        {
            resolved_imports
                .insert("@hypreact/sdk/jsx-runtime".into(), "@hypreact/sdk/jsx-runtime".into());
        }

        modules.push(JavaScriptModule {
            specifier: module_key(&graph.app.root_dir, module_id),
            source,
            resolved_imports,
        });
    }

    for module in &compiled.virtual_modules {
        if modules.iter().any(|existing| existing.specifier == module.specifier) {
            continue;
        }

        modules.push(JavaScriptModule {
            specifier: module.specifier.clone(),
            source: module.code.clone(),
            resolved_imports: BTreeMap::new(),
        });
    }

    Ok(JavaScriptModuleGraph {
        entry: module_key(&graph.app.root_dir, &ModuleId::File(graph.app.entry_path.clone())),
        modules,
    })
}

fn strip_stylesheet_imports(path: &Path, source: &str) -> Result<String, CompileError> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path)
        .map_err(|_| CompileError::UnsupportedSourceType { path: path.to_path_buf() })?;
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return Err(CompileError::Transpile { path: path.to_path_buf() });
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

fn read_virtual_module_source(specifier: &str) -> Result<String, CompileError> {
    source_for_virtual_module(specifier)
        .map(str::to_string)
        .ok_or_else(|| CompileError::UnsupportedVirtualModule { specifier: specifier.into() })
}

fn module_key(root_dir: &Path, module_id: &ModuleId) -> String {
    match module_id {
        ModuleId::File(path) => {
            path.strip_prefix(root_dir).unwrap_or(path).to_string_lossy().replace('\\', "/")
        }
        ModuleId::Virtual(specifier) => specifier.clone(),
    }
}
