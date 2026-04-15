use std::collections::BTreeMap;

use leptos::prelude::Get;

use crate::app_state::AppState;
use crate::editor_files::{EditorFileId, file_by_id, initial_content};

pub fn active_file_path(app_state: AppState) -> String {
    app_state
        .active_file_id
        .get()
        .map(file_by_id)
        .map(|file| file.path.to_string())
        .unwrap_or_else(|| "no file open".to_string())
}

pub fn dirty_file_count(buffers: &BTreeMap<EditorFileId, String>) -> usize {
    buffers.iter().filter(|(file_id, value)| value.as_str() != initial_content(**file_id)).count()
}
