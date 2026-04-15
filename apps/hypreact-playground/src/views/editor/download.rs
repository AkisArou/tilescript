use crate::editor_files::{EditorFileId, file_by_id, initial_content};
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
    buffers: &std::collections::BTreeMap<EditorFileId, String>,
) -> Vec<DirectoryDownloadItem> {
    let Some(root_path) = directory.download_root_path else {
        return Vec::new();
    };

    let mut items = Vec::new();
    collect_directory_download_items_recursive(directory, root_path, buffers, &mut items);
    items
}

fn collect_directory_download_items_recursive(
    directory: &EditorFileTreeDirectory,
    root_path: &str,
    buffers: &std::collections::BTreeMap<EditorFileId, String>,
    items: &mut Vec<DirectoryDownloadItem>,
) {
    for child in &directory.children {
        match child {
            EditorFileTreeNode::Directory(child_directory) => {
                collect_directory_download_items_recursive(
                    child_directory,
                    root_path,
                    buffers,
                    items,
                );
            }
            EditorFileTreeNode::File(file_id) => {
                let file = file_by_id(*file_id);
                let relative_path =
                    file.path.strip_prefix(root_path).unwrap_or(file.path).trim_start_matches('/');
                let content = buffers
                    .get(file_id)
                    .cloned()
                    .unwrap_or_else(|| initial_content(*file_id).to_string());

                items.push(DirectoryDownloadItem {
                    relative_path: relative_path.to_string(),
                    content,
                });
            }
        }
    }
}
