use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::response::FfiError;

pub fn bootstrap_config_root(config_root: &Path) -> Result<bool, FfiError> {
    std::fs::create_dir_all(config_root).map_err(|error| {
        FfiError::InvalidInput(format!(
            "failed to create config root `{}`: {error}",
            config_root.display()
        ))
    })?;

    let mut expected_paths = BTreeSet::new();
    sync_missing_template_files(&template_source_root(), config_root, &mut expected_paths)
}

fn template_source_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/js")
        .canonicalize()
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/js"))
}

fn sync_missing_template_files(
    source_root: &Path,
    destination_root: &Path,
    expected_paths: &mut BTreeSet<PathBuf>,
) -> Result<bool, FfiError> {
    let mut changed = false;

    for entry in std::fs::read_dir(source_root).map_err(|error| {
        FfiError::InvalidInput(format!("failed to read `{}`: {error}", source_root.display()))
    })? {
        let entry = entry.map_err(|error| {
            FfiError::InvalidInput(format!(
                "failed to inspect `{}`: {error}",
                source_root.display()
            ))
        })?;
        let source_path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            FfiError::InvalidInput(format!(
                "failed to inspect `{}`: {error}",
                source_path.display()
            ))
        })?;
        let name = entry.file_name();
        let destination_path = destination_root.join(name);

        if file_type.is_dir() {
            std::fs::create_dir_all(&destination_path).map_err(|error| {
                FfiError::InvalidInput(format!(
                    "failed to create `{}`: {error}",
                    destination_path.display()
                ))
            })?;
            changed |=
                sync_missing_template_files(&source_path, &destination_path, expected_paths)?;
            continue;
        }

        expected_paths.insert(destination_path.clone());
        if destination_path.exists() {
            continue;
        }

        let contents = std::fs::read(&source_path).map_err(|error| {
            FfiError::InvalidInput(format!("failed to read `{}`: {error}", source_path.display()))
        })?;
        std::fs::write(&destination_path, contents).map_err(|error| {
            FfiError::InvalidInput(format!(
                "failed to write `{}`: {error}",
                destination_path.display()
            ))
        })?;
        changed = true;
    }

    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::bootstrap_config_root;

    #[test]
    fn bootstrap_config_root_writes_template_files() {
        let root = std::env::temp_dir().join(format!(
            "hypreact-bootstrap-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));

        let changed = bootstrap_config_root(&root).expect("bootstrap config root");
        assert!(changed);
        assert!(root.join("config.ts").exists());
        assert!(root.join(".gitignore").exists());
        assert!(root.join("tsconfig.json").exists());
        assert!(root.join("layouts/master-stack/index.tsx").exists());

        let changed_again = bootstrap_config_root(&root).expect("rebootstrap config root");
        assert!(!changed_again);

        let _ = std::fs::remove_dir_all(root);
    }
}
