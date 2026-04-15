use crate::editor_files::{AuthoringLanguage, DynamicLayoutFileSet, EditorFileKey, file_by_key};
use crate::editor_host::DirectoryDownloadItem;
use crate::workspace::{EditorFileTreeDirectory, EditorFileTreeNode};

pub fn download_directory_title(directory: &EditorFileTreeDirectory) -> String {
    let Some(root_path) = directory.download_root_path else {
        return "Download directory".to_string();
    };

    let parent_path = root_path.rsplit_once('/').map(|(parent, _)| parent).unwrap_or_default();

    if parent_path.is_empty() {
        format!(
            "Choose the parent directory so {}/ is created there. If folder picking is unavailable, files will download individually instead.",
            directory.name,
        )
    } else {
        format!(
            "Choose {parent_path}/ so {}/ is created there and its files are copied inside it. If folder picking is unavailable, files will download individually instead.",
            directory.name,
        )
    }
}

pub fn collect_directory_download_items(
    directory: &EditorFileTreeDirectory,
    language: AuthoringLanguage,
    buffers: &std::collections::BTreeMap<EditorFileKey, String>,
    dynamic_layouts: &[DynamicLayoutFileSet],
) -> Vec<DirectoryDownloadItem> {
    let Some(root_path) = directory.download_root_path else {
        return Vec::new();
    };

    let mut items = Vec::new();
    collect_directory_download_items_recursive(
        directory,
        root_path,
        language,
        buffers,
        dynamic_layouts,
        &mut items,
    );
    items
}

fn collect_directory_download_items_recursive(
    directory: &EditorFileTreeDirectory,
    root_path: &str,
    language: AuthoringLanguage,
    buffers: &std::collections::BTreeMap<EditorFileKey, String>,
    dynamic_layouts: &[DynamicLayoutFileSet],
    items: &mut Vec<DirectoryDownloadItem>,
) {
    for child in &directory.children {
        match child {
            EditorFileTreeNode::Directory(child_directory) => {
                collect_directory_download_items_recursive(
                    child_directory,
                    root_path,
                    language,
                    buffers,
                    dynamic_layouts,
                    items,
                );
            }
            EditorFileTreeNode::File(file_id) => {
                let file = file_by_key(file_id, dynamic_layouts);
                let relative_path =
                    file.path.strip_prefix(root_path).unwrap_or(&file.path).trim_start_matches('/');
                let content = buffers.get(file_id).cloned().unwrap_or(file.initial_content);

                items.push(DirectoryDownloadItem {
                    relative_path: relative_path.to_string(),
                    content,
                });
            }
        }
    }
}
