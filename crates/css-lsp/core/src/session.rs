use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use lsp_types::{
    CodeActionOrCommand, CompletionResponse, Diagnostic, DocumentSymbol, GotoDefinitionResponse,
    Hover, Location, Position, SymbolInformation, Url, WorkspaceEdit,
};

use crate::{
    code_actions::code_actions_for,
    completion::completions_for,
    definition::definition_for,
    diagnostics::diagnostics_for,
    documents::DocumentStore,
    hover::hover_for,
    references::references_for,
    rename::rename_for,
    symbols::document_symbols_for,
    workspace::{InMemorySourceProvider, SourceProvider, WorkspaceState},
    workspace_symbols::workspace_symbols_for,
};

#[derive(Debug)]
pub struct Session {
    documents: DocumentStore,
    workspace: WorkspaceState,
}

#[derive(Debug, Clone)]
pub struct SessionChangeResult {
    pub uri: Url,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    pub documents: Vec<(Url, String)>,
}

impl Session {
    pub fn new() -> Self {
        Self { documents: DocumentStore::default(), workspace: WorkspaceState::default() }
    }

    pub fn with_source_provider(source_provider: Arc<dyn SourceProvider>) -> Self {
        Self {
            documents: DocumentStore::default(),
            workspace: WorkspaceState::new(source_provider),
        }
    }

    pub fn with_in_memory_sources(files: HashMap<PathBuf, String>) -> Self {
        Self::with_source_provider(Arc::new(InMemorySourceProvider::new(files)))
    }

    pub fn open(&mut self, uri: Url, text: String) -> SessionChangeResult {
        self.documents.open(uri.clone(), text);
        if let Some(source) = self.documents.get(&uri) {
            self.workspace.upsert_document(&uri, source);
        }
        SessionChangeResult { uri: uri.clone(), diagnostics: self.diagnostics(&uri) }
    }

    pub fn change(&mut self, uri: Url, text: String) -> SessionChangeResult {
        self.documents.update(uri.clone(), text);
        if let Some(source) = self.documents.get(&uri) {
            self.workspace.upsert_document(&uri, source);
        }
        SessionChangeResult { uri: uri.clone(), diagnostics: self.diagnostics(&uri) }
    }

    pub fn close(&mut self, uri: &Url) -> SessionChangeResult {
        self.documents.close(uri);
        self.workspace.remove_document(uri);
        SessionChangeResult { uri: uri.clone(), diagnostics: Vec::new() }
    }

    pub fn diagnostics(&self, uri: &Url) -> Vec<Diagnostic> {
        let Some(source) = self.documents.get(uri) else {
            return Vec::new();
        };
        diagnostics_for(uri, source, self.workspace.project_index())
    }

    pub fn hover(&self, uri: &Url, position: Position) -> Option<Hover> {
        let source = self.documents.get(uri)?;
        hover_for(uri, source, position, self.workspace.project_index())
    }

    pub fn completion(&self, uri: &Url, position: Position) -> Option<CompletionResponse> {
        let source = self.documents.get(uri)?;
        completions_for(uri, source, position, self.workspace.project_index())
    }

    pub fn definition(&self, uri: &Url, position: Position) -> Option<GotoDefinitionResponse> {
        let source = self.documents.get(uri)?;
        definition_for(uri, source, position, self.workspace.project_index())
    }

    pub fn references(
        &self,
        uri: &Url,
        position: Position,
        include_declaration: bool,
    ) -> Vec<Location> {
        let Some(source) = self.documents.get(uri) else {
            return Vec::new();
        };
        references_for(
            uri,
            source,
            position,
            include_declaration,
            self.workspace.project_index(),
            &self.documents.snapshot(),
        )
    }

    pub fn rename(&self, uri: &Url, position: Position, new_name: &str) -> Option<WorkspaceEdit> {
        let source = self.documents.get(uri)?;
        rename_for(
            uri,
            source,
            position,
            new_name,
            self.workspace.project_index(),
            &self.documents.snapshot(),
        )
    }

    pub fn document_symbols(&self, uri: &Url) -> Vec<DocumentSymbol> {
        self.documents.get(uri).map(document_symbols_for).unwrap_or_default()
    }

    pub fn workspace_symbols(&self, query: &str) -> Vec<SymbolInformation> {
        workspace_symbols_for(query, self.workspace.project_index())
    }

    pub fn code_actions(&self, uri: &Url, diagnostics: &[Diagnostic]) -> Vec<CodeActionOrCommand> {
        code_actions_for(uri, self.workspace.project_index(), diagnostics)
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot { documents: self.documents.snapshot() }
    }
}
