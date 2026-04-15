use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::response::FfiError;

const SDK_EXCLUDED_DIRS: &[&str] = &["node_modules"];

pub fn sync_sdk_support(config_root: &Path) -> Result<bool, FfiError> {
    let managed_root = config_root.join(".sdk");
    let mut expected_paths = BTreeSet::new();
    let mut changed = sync_directory(&js_sdk_source_root(), &managed_root, &mut expected_paths)?;
    changed |= sync_directory(&lua_sdk_source_root(), &managed_root.join("lua"), &mut expected_paths)?;

    prune_stale_files(&managed_root, &expected_paths, &mut changed)?;
    changed |= remove_legacy_node_modules_link(config_root)?;

    Ok(changed)
}

fn js_sdk_source_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/sdk/js")
        .canonicalize()
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/sdk/js"))
}

fn lua_sdk_source_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/sdk/lua")
        .canonicalize()
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/sdk/lua"))
}

fn sync_directory(
    source_root: &Path,
    destination_root: &Path,
    expected_paths: &mut BTreeSet<PathBuf>,
) -> Result<bool, FfiError> {
    let mut changed = false;

    for entry in std::fs::read_dir(source_root).map_err(|error| {
        FfiError::InvalidInput(format!("failed to read `{}`: {error}", source_root.display()))
    })? {
        let entry = entry.map_err(|error| {
            FfiError::InvalidInput(format!("failed to inspect `{}`: {error}", source_root.display()))
        })?;
        let source_path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            FfiError::InvalidInput(format!("failed to inspect `{}`: {error}", source_path.display()))
        })?;
        let name = entry.file_name();
        let destination_path = destination_root.join(name);

        if file_type.is_dir() {
            if source_path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| SDK_EXCLUDED_DIRS.contains(&name))
            {
                continue;
            }
            changed |= sync_directory(&source_path, &destination_path, expected_paths)?;
            continue;
        }

        let contents = std::fs::read_to_string(&source_path).map_err(|error| {
            FfiError::InvalidInput(format!("failed to read `{}`: {error}", source_path.display()))
        })?;
        expected_paths.insert(destination_path.clone());
        changed |= write_if_changed(&destination_path, &contents)?;
    }

    Ok(changed)
}

fn write_if_changed(path: &Path, contents: &str) -> Result<bool, FfiError> {
    if let Ok(existing) = std::fs::read_to_string(path)
        && existing == contents
    {
        return Ok(false);
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| FfiError::InvalidInput(format!("failed to create `{}`: {error}", parent.display())))?;
    }

    std::fs::write(path, contents)
        .map_err(|error| FfiError::InvalidInput(format!("failed to write `{}`: {error}", path.display())))?;
    Ok(true)
}

fn prune_stale_files(
    root: &Path,
    expected_paths: &BTreeSet<PathBuf>,
    changed: &mut bool,
) -> Result<(), FfiError> {
    if !root.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(root)
        .map_err(|error| FfiError::InvalidInput(format!("failed to read `{}`: {error}", root.display())))?
    {
        let entry = entry.map_err(|error| {
            FfiError::InvalidInput(format!("failed to inspect `{}`: {error}", root.display()))
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            FfiError::InvalidInput(format!("failed to inspect `{}`: {error}", path.display()))
        })?;

        if file_type.is_dir() {
            prune_stale_files(&path, expected_paths, changed)?;
            if std::fs::read_dir(&path)
                .map_err(|error| {
                    FfiError::InvalidInput(format!("failed to read `{}`: {error}", path.display()))
                })?
                .next()
                .is_none()
            {
                std::fs::remove_dir(&path).map_err(|error| {
                    FfiError::InvalidInput(format!("failed to remove `{}`: {error}", path.display()))
                })?;
                *changed = true;
            }
            continue;
        }

        if !expected_paths.contains(&path) {
            std::fs::remove_file(&path).map_err(|error| {
                FfiError::InvalidInput(format!("failed to remove `{}`: {error}", path.display()))
            })?;
            *changed = true;
        }
    }

    Ok(())
}

fn remove_legacy_node_modules_link(config_root: &Path) -> Result<bool, FfiError> {
    let legacy_sdk = config_root.join("node_modules/@hypreact/sdk");
    let legacy_scope = config_root.join("node_modules/@hypreact");
    let legacy_root = config_root.join("node_modules");
    let mut changed = false;

    if let Ok(metadata) = std::fs::symlink_metadata(&legacy_sdk) {
        if metadata.file_type().is_symlink() || metadata.is_file() {
            std::fs::remove_file(&legacy_sdk).map_err(|error| {
                FfiError::InvalidInput(format!("failed to remove `{}`: {error}", legacy_sdk.display()))
            })?;
            changed = true;
        }
    }

    if legacy_scope.exists()
        && std::fs::read_dir(&legacy_scope)
            .map_err(|error| {
                FfiError::InvalidInput(format!("failed to read `{}`: {error}", legacy_scope.display()))
            })?
            .next()
            .is_none()
    {
        std::fs::remove_dir(&legacy_scope).map_err(|error| {
            FfiError::InvalidInput(format!("failed to remove `{}`: {error}", legacy_scope.display()))
        })?;
        changed = true;
    }

    if legacy_root.exists()
        && std::fs::read_dir(&legacy_root)
            .map_err(|error| {
                FfiError::InvalidInput(format!("failed to read `{}`: {error}", legacy_root.display()))
            })?
            .next()
            .is_none()
    {
        std::fs::remove_dir(&legacy_root).map_err(|error| {
            FfiError::InvalidInput(format!("failed to remove `{}`: {error}", legacy_root.display()))
        })?;
        changed = true;
    }

    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::sync_sdk_support;

    #[test]
    fn sync_sdk_support_writes_managed_sdk_files() {
        let root = std::env::temp_dir().join(format!(
            "hypreact-sdk-sync-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let changed = sync_sdk_support(&root).expect("sync sdk support");

        assert!(changed);
        assert!(root.join(".sdk/tsconfig.json").exists());
        assert!(root.join(".sdk/src/config.d.ts").exists());
        assert!(root.join(".sdk/lua/hypreact.lua").exists());

        let changed_again = sync_sdk_support(&root).expect("resync sdk support");
        assert!(!changed_again);

        let _ = std::fs::remove_dir_all(root);
    }
}
