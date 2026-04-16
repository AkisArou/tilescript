use std::collections::BTreeMap;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArtifactKind {
    Config,
    Layout,
    JsModuleGraph,
    JsBytecode,
    StylesheetAnalysis,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArtifactKey {
    pub kind: ArtifactKind,
    pub identity: String,
}

impl ArtifactKey {
    pub fn config(identity: impl Into<String>) -> Self {
        Self { kind: ArtifactKind::Config, identity: identity.into() }
    }

    pub fn layout(identity: impl Into<String>) -> Self {
        Self { kind: ArtifactKind::Layout, identity: identity.into() }
    }

    pub fn stylesheet_analysis(identity: impl Into<String>) -> Self {
        Self { kind: ArtifactKind::StylesheetAnalysis, identity: identity.into() }
    }

    pub fn js_module_graph(identity: impl Into<String>) -> Self {
        Self { kind: ArtifactKind::JsModuleGraph, identity: identity.into() }
    }

    pub fn js_bytecode(identity: impl Into<String>) -> Self {
        Self { kind: ArtifactKind::JsBytecode, identity: identity.into() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactRecord<T> {
    pub fingerprint: String,
    pub value: T,
}

impl<T> ArtifactRecord<T> {
    pub fn new(fingerprint: impl Into<String>, value: T) -> Self {
        Self { fingerprint: fingerprint.into(), value }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArtifactMap<T> {
    records: BTreeMap<ArtifactKey, ArtifactRecord<T>>,
}

impl<T> ArtifactMap<T> {
    pub fn new() -> Self {
        Self { records: BTreeMap::new() }
    }

    pub fn get(&self, key: &ArtifactKey) -> Option<&ArtifactRecord<T>> {
        self.records.get(key)
    }

    pub fn get_mut(&mut self, key: &ArtifactKey) -> Option<&mut ArtifactRecord<T>> {
        self.records.get_mut(key)
    }

    pub fn insert(
        &mut self,
        key: ArtifactKey,
        record: ArtifactRecord<T>,
    ) -> Option<ArtifactRecord<T>> {
        self.records.insert(key, record)
    }

    pub fn retain(&mut self, f: impl FnMut(&ArtifactKey, &mut ArtifactRecord<T>) -> bool) {
        self.records.retain(f);
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ArtifactKey, &ArtifactRecord<T>)> {
        self.records.iter()
    }

    pub fn values(&self) -> impl Iterator<Item = &ArtifactRecord<T>> {
        self.records.values()
    }
}

pub type ArtifactRegistry<T> = ArtifactMap<T>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArtifactGraph {
    outgoing: BTreeMap<ArtifactKey, BTreeSet<ArtifactKey>>,
}

impl ArtifactGraph {
    pub fn new() -> Self {
        Self { outgoing: BTreeMap::new() }
    }

    pub fn replace_edges(
        &mut self,
        source: ArtifactKey,
        targets: impl IntoIterator<Item = ArtifactKey>,
    ) {
        self.outgoing.insert(source, targets.into_iter().collect());
    }

    pub fn dependents_of(&self, source: &ArtifactKey) -> impl Iterator<Item = &ArtifactKey> {
        self.outgoing.get(source).into_iter().flat_map(|targets| targets.iter())
    }

    pub fn transitive_dependents_of(&self, source: &ArtifactKey) -> BTreeSet<ArtifactKey> {
        let mut visited = BTreeSet::new();
        let mut pending = self.dependents_of(source).cloned().collect::<Vec<_>>();

        while let Some(next) = pending.pop() {
            if !visited.insert(next.clone()) {
                continue;
            }

            pending.extend(self.dependents_of(&next).cloned());
        }

        visited
    }

    pub fn invalidate_dependents_of<T>(
        &mut self,
        source: &ArtifactKey,
        registry: &mut ArtifactRegistry<T>,
    ) {
        let dependents = self.transitive_dependents_of(source);
        for dependent in &dependents {
            registry.records.remove(dependent);
            self.remove(dependent);
        }
    }

    pub fn invalidate<T>(&mut self, key: &ArtifactKey, registry: &mut ArtifactRegistry<T>) {
        registry.records.remove(key);
        self.invalidate_dependents_of(key, registry);
        self.remove(key);
    }

    pub fn remove(&mut self, key: &ArtifactKey) {
        self.outgoing.remove(key);
        for targets in self.outgoing.values_mut() {
            targets.remove(key);
        }
    }

    pub fn clear(&mut self) {
        self.outgoing.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalidates_transitive_dependents_from_source_key() {
        let config = ArtifactKey::config("config");
        let layout = ArtifactKey::layout("master-stack");
        let stylesheet = ArtifactKey::stylesheet_analysis("layouts/master-stack/index.css");

        let mut graph = ArtifactGraph::new();
        graph.replace_edges(config.clone(), [layout.clone()]);
        graph.replace_edges(layout.clone(), [stylesheet.clone()]);

        let mut registry = ArtifactRegistry::new();
        registry.insert(layout.clone(), ArtifactRecord::new("layout-fp", "layout"));
        registry.insert(stylesheet.clone(), ArtifactRecord::new("style-fp", "style"));

        graph.invalidate_dependents_of(&config, &mut registry);

        assert!(registry.get(&layout).is_none());
        assert!(registry.get(&stylesheet).is_none());
        assert!(graph.dependents_of(&config).next().is_none());
    }
}
