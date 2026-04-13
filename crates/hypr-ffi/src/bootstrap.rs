use std::path::Path;

use crate::response::FfiError;

struct TemplateAsset {
    relative_path: &'static str,
    contents: &'static str,
}

const TEMPLATE_ASSETS: &[TemplateAsset] = &[
    TemplateAsset {
        relative_path: "config.ts",
        contents: include_str!("../../../template/config.ts"),
    },
    TemplateAsset {
        relative_path: "tsconfig.json",
        contents: include_str!("../../../template/tsconfig.json"),
    },
    TemplateAsset {
        relative_path: "index.css",
        contents: include_str!("../../../template/index.css"),
    },
    TemplateAsset {
        relative_path: "layouts/master-stack/index.tsx",
        contents: include_str!("../../../template/layouts/master-stack/index.tsx"),
    },
    TemplateAsset {
        relative_path: "layouts/master-stack/index.css",
        contents: include_str!("../../../template/layouts/master-stack/index.css"),
    },
    TemplateAsset {
        relative_path: "components/StackGroup.tsx",
        contents: include_str!("../../../template/components/StackGroup.tsx"),
    },
    TemplateAsset {
        relative_path: "components/StackGroup.css",
        contents: include_str!("../../../template/components/StackGroup.css"),
    },
    TemplateAsset {
        relative_path: "components/common/StackSlot.tsx",
        contents: include_str!("../../../template/components/common/StackSlot.tsx"),
    },
];

pub fn bootstrap_config_root(config_root: &Path) -> Result<bool, FfiError> {
    let mut changed = false;

    std::fs::create_dir_all(config_root).map_err(|error| {
        FfiError::InvalidInput(format!(
            "failed to create config root `{}`: {error}",
            config_root.display()
        ))
    })?;

    for asset in TEMPLATE_ASSETS {
        let destination = config_root.join(asset.relative_path);
        if destination.exists() {
            continue;
        }

        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                FfiError::InvalidInput(format!("failed to create `{}`: {error}", parent.display()))
            })?;
        }

        std::fs::write(&destination, asset.contents).map_err(|error| {
            FfiError::InvalidInput(format!("failed to write `{}`: {error}", destination.display()))
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
        assert!(root.join("tsconfig.json").exists());
        assert!(root.join("layouts/master-stack/index.tsx").exists());

        let changed_again = bootstrap_config_root(&root).expect("rebootstrap config root");
        assert!(!changed_again);

        let _ = std::fs::remove_dir_all(root);
    }
}
