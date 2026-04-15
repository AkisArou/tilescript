use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorFile {
    pub id: EditorFileId,
    pub label: &'static str,
    pub path: &'static str,
    pub language: &'static str,
}

pub const WORKSPACE_ROOT: &str = "~/.config/hypreact";
pub const WORKSPACE_FS_ROOT: &str = "/home/demo/.config/hypreact";

include!(concat!(env!("OUT_DIR"), "/editor_files_manifest.rs"));

pub fn model_path(file_id: EditorFileId) -> String {
    format!("file://{}", runtime_path(file_id))
}

pub fn file_id_by_model_path(path: &str) -> Option<EditorFileId> {
    EDITOR_FILES.iter().find(|file| model_path(file.id) == path).map(|file| file.id)
}

pub fn file_badge(language: &str) -> &'static str {
    match language {
        "css" => "css",
        "typescriptreact" => "tsx",
        "typescript" => "ts",
        _ => "txt",
    }
}

pub fn initial_editor_buffers() -> BTreeMap<EditorFileId, String> {
    EDITOR_FILES.iter().map(|file| (file.id, initial_content(file.id).to_string())).collect()
}

pub fn file_by_id(file_id: EditorFileId) -> &'static EditorFile {
    EDITOR_FILES.iter().find(|file| file.id == file_id).expect("editor file id should exist")
}
