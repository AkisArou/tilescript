use std::collections::BTreeMap;

use leptos::prelude::Get;

use crate::app_state::AppState;
use crate::editor_files::{EditorFile, EditorFileId, file_badge, file_by_id, initial_content};

pub fn is_file_dirty(buffers: &BTreeMap<EditorFileId, String>, file_id: EditorFileId) -> bool {
    buffers.get(&file_id).map(String::as_str).unwrap_or_else(|| initial_content(file_id))
        != initial_content(file_id)
}

pub fn active_file(app_state: AppState) -> Option<&'static EditorFile> {
    app_state.active_file_id.get().map(file_by_id)
}

pub fn active_file_path(app_state: AppState) -> String {
    active_file(app_state)
        .map(|file| file.path.to_string())
        .unwrap_or_else(|| "no file open".to_string())
}

pub fn editor_file_badge(language: &str) -> &'static str {
    file_badge(language)
}

pub fn active_file_is_dirty(app_state: AppState) -> bool {
    let Some(file) = active_file(app_state) else {
        return false;
    };

    let buffers = app_state.editor_buffers.get();
    is_file_dirty(&buffers, file.id)
}

pub fn active_buffer_text(app_state: AppState) -> String {
    let Some(file) = active_file(app_state) else {
        return String::new();
    };

    app_state
        .editor_buffers
        .get()
        .get(&file.id)
        .cloned()
        .unwrap_or_else(|| initial_content(file.id).to_string())
}
