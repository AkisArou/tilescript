use std::collections::BTreeMap;

use crate::editor_files::{EDITOR_FILES, EditorFileId, WORKSPACE_ROOT};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorFileTreeDirectory {
    pub name: &'static str,
    pub path: &'static str,
    pub download_root_path: Option<&'static str>,
    pub default_open: bool,
    pub children: Vec<EditorFileTreeNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorFileTreeNode {
    Directory(EditorFileTreeDirectory),
    File(EditorFileId),
}

#[derive(Debug, Clone)]
struct DirectoryBuilder {
    name: &'static str,
    path: &'static str,
    download_root_path: Option<&'static str>,
    default_open: bool,
    directories: BTreeMap<String, DirectoryBuilder>,
    files: Vec<EditorFileId>,
}

impl DirectoryBuilder {
    fn new(name: &'static str, path: &'static str, default_open: bool) -> Self {
        Self {
            name,
            path,
            download_root_path: Some(path),
            default_open,
            directories: BTreeMap::new(),
            files: Vec::new(),
        }
    }

    fn insert_file(&mut self, file_path: &'static str, file_id: EditorFileId) {
        let relative = file_path.strip_prefix("~/.config/hypreact/").unwrap_or(file_path);
        let segments = relative.split('/').collect::<Vec<_>>();

        if segments.len() == 1 {
            self.files.push(file_id);
            return;
        }

        self.insert_segments(&segments, file_id);
    }

    fn insert_segments(&mut self, segments: &[&str], file_id: EditorFileId) {
        if segments.len() == 1 {
            self.files.push(file_id);
            return;
        }

        let dir_name = segments[0];
        let dir_name_static = leak_string(dir_name.to_string());
        let dir_path = path_for_segments(&segments[..segments.len() - 1]);
        self.directories
            .entry(dir_name.to_string())
            .or_insert_with(|| DirectoryBuilder::new(dir_name_static, dir_path, true))
            .insert_segments(&segments[1..], file_id);
    }

    fn build(self) -> EditorFileTreeDirectory {
        let mut children = Vec::new();
        children.extend(self.files.into_iter().map(EditorFileTreeNode::File));
        children.extend(
            self.directories
                .into_values()
                .map(|directory| EditorFileTreeNode::Directory(directory.build())),
        );

        EditorFileTreeDirectory {
            name: self.name,
            path: self.path,
            download_root_path: self.download_root_path,
            default_open: self.default_open,
            children,
        }
    }
}

pub fn initial_open_directories() -> BTreeMap<String, bool> {
    let mut directories = BTreeMap::new();
    collect_default_open_directories(&workspace_file_tree(), &mut directories);
    directories
}

pub fn workspace_file_tree() -> EditorFileTreeDirectory {
    let mut root = DirectoryBuilder::new(WORKSPACE_ROOT, WORKSPACE_ROOT, true);
    for file in EDITOR_FILES {
        root.insert_file(file.path, file.id);
    }
    root.build()
}

fn collect_default_open_directories(
    directory: &EditorFileTreeDirectory,
    out: &mut BTreeMap<String, bool>,
) {
    out.insert(directory.path.to_string(), directory.default_open);
    for child in &directory.children {
        if let EditorFileTreeNode::Directory(directory) = child {
            collect_default_open_directories(directory, out);
        }
    }
}

fn path_for_segments(segments: &[&str]) -> &'static str {
    leak_string(format!("~/.config/hypreact/{}", segments.join("/")))
}

fn leak_string(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}
