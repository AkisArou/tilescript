use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tilescript_runtime_js_core::compile::AppBuildPlan;
use tilescript_runtime_js_core::graph::{
    AppKind, DiscoveredApp, ModuleGraphBuilder, discover_project_apps,
};
use lsp_types::Url;

use crate::project::ProjectIndex;

pub trait SourceProvider: std::fmt::Debug + Send + Sync {
    fn read_to_string(&self, path: &Path) -> Option<String>;
    fn path_exists(&self, path: &Path) -> bool;
}

#[derive(Debug, Default)]
pub struct FileSystemSourceProvider;

impl SourceProvider for FileSystemSourceProvider {
    fn read_to_string(&self, path: &Path) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }

    fn path_exists(&self, path: &Path) -> bool {
        path.exists()
    }
}

#[derive(Debug, Default)]
pub struct InMemorySourceProvider {
    files: HashMap<PathBuf, String>,
}

impl InMemorySourceProvider {
    pub fn new(files: HashMap<PathBuf, String>) -> Self {
        Self { files }
    }

    pub fn insert(&mut self, path: PathBuf, source: String) {
        self.files.insert(path, source);
    }

    pub fn remove(&mut self, path: &Path) {
        self.files.remove(path);
    }
}

impl SourceProvider for InMemorySourceProvider {
    fn read_to_string(&self, path: &Path) -> Option<String> {
        self.files.get(path).cloned()
    }

    fn path_exists(&self, path: &Path) -> bool {
        self.files.contains_key(path)
    }
}

#[derive(Debug)]
pub struct WorkspaceState {
    project_index: ProjectIndex,
    open_documents: HashMap<PathBuf, String>,
    source_provider: Arc<dyn SourceProvider>,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self::new(Arc::new(FileSystemSourceProvider))
    }
}

impl WorkspaceState {
    pub fn new(source_provider: Arc<dyn SourceProvider>) -> Self {
        Self {
            project_index: ProjectIndex::default(),
            open_documents: HashMap::new(),
            source_provider,
        }
    }

    pub fn upsert_document(&mut self, uri: &Url, source: &str) {
        let Some(path) = crate::uri::path_from_url(uri) else {
            return;
        };

        self.open_documents.insert(path.clone(), source.to_string());

        if let Some(config_entry) =
            discover_config_entry_for_path(&path, self.source_provider.as_ref())
        {
            self.rebuild_project_from_config(&config_entry);
        }
    }

    pub fn remove_document(&mut self, uri: &Url) {
        let Some(path) = crate::uri::path_from_url(uri) else {
            return;
        };

        self.open_documents.remove(&path);

        if let Some(config_entry) =
            discover_config_entry_for_path(&path, self.source_provider.as_ref())
        {
            self.rebuild_project_from_config(&config_entry);
        }
    }

    pub fn project_index(&self) -> &ProjectIndex {
        &self.project_index
    }

    fn rebuild_project_from_config(&mut self, config_entry: &Path) {
        let Ok(project) = discover_project_apps(config_entry) else {
            return;
        };

        let mut next_index = ProjectIndex::default();
        let graph_builder = ModuleGraphBuilder::new();

        for app in std::iter::once(&project.config_app).chain(project.layout_apps.iter()) {
            let Ok(graph) = graph_builder.build(app) else {
                continue;
            };
            let plan = AppBuildPlan::from_graph(&graph);
            let script_sources =
                collect_script_sources(&plan, &self.open_documents, self.source_provider.as_ref());
            let stylesheet_sources = collect_stylesheet_sources(
                &plan,
                &self.open_documents,
                self.source_provider.as_ref(),
            );
            let scope_id = app_scope_id(app);
            next_index.index_app_scope(scope_id, script_sources, stylesheet_sources);
        }

        self.project_index = next_index;
    }
}

fn collect_script_sources(
    plan: &AppBuildPlan,
    open_documents: &HashMap<PathBuf, String>,
    source_provider: &dyn SourceProvider,
) -> Vec<(PathBuf, String)> {
    plan.script_modules
        .iter()
        .filter_map(|path| {
            read_source(path, open_documents, source_provider).map(|source| (path.clone(), source))
        })
        .collect()
}

fn collect_stylesheet_sources(
    plan: &AppBuildPlan,
    open_documents: &HashMap<PathBuf, String>,
    source_provider: &dyn SourceProvider,
) -> Vec<(PathBuf, String)> {
    plan.stylesheet_modules
        .iter()
        .filter_map(|path| {
            read_source(path, open_documents, source_provider).map(|source| (path.clone(), source))
        })
        .collect()
}

fn read_source(
    path: &Path,
    open_documents: &HashMap<PathBuf, String>,
    source_provider: &dyn SourceProvider,
) -> Option<String> {
    if let Some(source) = open_documents.get(path) {
        return Some(source.clone());
    }
    source_provider.read_to_string(path)
}

fn app_scope_id(app: &DiscoveredApp) -> PathBuf {
    match app.kind {
        AppKind::Config => app.root_dir.join("index.css"),
        AppKind::Layout => app.entry_path.clone(),
    }
}

fn discover_config_entry_for_path(
    path: &Path,
    source_provider: &dyn SourceProvider,
) -> Option<PathBuf> {
    let mut current = if path.is_dir() { path.to_path_buf() } else { path.parent()?.to_path_buf() };

    loop {
        if let Some(config) = config_entry_in_dir(&current, source_provider) {
            return Some(config);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn config_entry_in_dir(dir: &Path, source_provider: &dyn SourceProvider) -> Option<PathBuf> {
    ["config.tsx", "config.ts", "config.jsx", "config.js"]
        .into_iter()
        .map(|name| dir.join(name))
        .find(|path| source_provider.path_exists(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_nearest_config_entry() {
        let root =
            std::env::temp_dir().join(format!("tilescript-css-lsp-workspace-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("layouts/master-stack/components")).unwrap();
        std::fs::write(root.join("config.ts"), "export default {};").unwrap();

        let discovered = discover_config_entry_for_path(
            &root.join("layouts/master-stack/components/Foo.tsx"),
            &FileSystemSourceProvider,
        )
        .unwrap();

        assert_eq!(discovered, root.join("config.ts"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn in_memory_source_provider_tracks_virtual_files() {
        let root = PathBuf::from("/virtual/project");
        let config = root.join("config.ts");
        let mut provider = InMemorySourceProvider::default();
        provider.insert(config.clone(), "export default {};".to_string());

        let discovered =
            discover_config_entry_for_path(&root.join("layouts/master-stack/index.tsx"), &provider)
                .unwrap();

        assert_eq!(discovered, config);
    }
}
