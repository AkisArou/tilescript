use std::collections::BTreeMap;

use leptos::prelude::Get;

use crate::app_state::AppState;
use crate::editor_files::{EditorFileKey, file_badge, file_by_key};

pub fn is_file_dirty(
    buffers: &BTreeMap<EditorFileKey, String>,
    app_state: AppState,
    file_id: &EditorFileKey,
) -> bool {
    let file = file_by_key(file_id, &app_state.dynamic_layouts.get());
    buffers.get(file_id).map(String::as_str).unwrap_or(file.initial_content.as_str())
        != file.initial_content.as_str()
}

pub fn active_file(app_state: AppState) -> Option<crate::editor_files::EditorFileMeta> {
    let file_id = app_state.active_file_id.get()?;
    Some(file_by_key(&file_id, &app_state.dynamic_layouts.get()))
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
    is_file_dirty(&buffers, app_state, &file.key)
}

pub fn active_buffer_text(app_state: AppState) -> String {
    let Some(file) = active_file(app_state) else {
        return String::new();
    };

    app_state.editor_buffers.get().get(&file.key).cloned().unwrap_or(file.initial_content)
}
