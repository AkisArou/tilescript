use std::collections::HashMap;

use lsp_types::Url;

#[derive(Debug, Default)]
pub struct DocumentStore {
    documents: HashMap<Url, String>,
}

impl DocumentStore {
    pub fn open(&mut self, uri: Url, text: String) {
        self.documents.insert(uri, text);
    }

    pub fn update(&mut self, uri: Url, text: String) {
        self.documents.insert(uri, text);
    }

    pub fn close(&mut self, uri: &Url) {
        self.documents.remove(uri);
    }

    pub fn get(&self, uri: &Url) -> Option<&str> {
        self.documents.get(uri).map(String::as_str)
    }

    pub fn snapshot(&self) -> Vec<(Url, String)> {
        self.documents.iter().map(|(uri, source)| (uri.clone(), source.clone())).collect()
    }
}
