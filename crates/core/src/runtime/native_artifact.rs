use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::prepared_layout::PreparedStylesheet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeDependencySnapshot {
    pub path: String,
    pub content_hash: String,
}

pub fn load_text_dependency(path: impl AsRef<Path>) -> Option<(String, NativeDependencySnapshot)> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).ok()?;
    let dependency = NativeDependencySnapshot {
        path: path.to_string_lossy().into_owned(),
        content_hash: hash_string(&source),
    };

    Some((source, dependency))
}

pub fn load_cached_stylesheet(
    path: impl AsRef<Path>,
) -> Option<(PreparedStylesheet, NativeDependencySnapshot)> {
    let path = path.as_ref();
    let (source, dependency) = load_text_dependency(path)?;
    Some((PreparedStylesheet { path: dependency.path.clone(), source }, dependency))
}

pub fn dependencies_match(dependencies: &[NativeDependencySnapshot]) -> bool {
    dependencies.iter().all(|dependency| {
        fs::read_to_string(&dependency.path)
            .ok()
            .map(|source| hash_string(&source) == dependency.content_hash)
            .unwrap_or(false)
    })
}

fn hash_string(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_cached_stylesheet_reads_latest_file_contents() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("index.css");

        fs::write(&path, ".root { width: 1px; }").unwrap();
        let (first, _) = load_cached_stylesheet(&path).unwrap();
        assert!(first.source.contains("1px"));

        fs::write(&path, ".root { width: 2px; }").unwrap();
        let (second, _) = load_cached_stylesheet(&path).unwrap();
        assert!(second.source.contains("2px"));
    }
}
