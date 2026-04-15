use leptos::prelude::Get;

use crate::app_state::AppState;
use crate::editor_files::file_by_key;

pub fn active_file(app_state: AppState) -> Option<crate::editor_files::EditorFileMeta> {
    let file_id = app_state.active_file_id.get()?;
    Some(file_by_key(&file_id, &app_state.dynamic_layouts.get()))
}

pub fn active_buffer_text(app_state: AppState) -> String {
    let Some(file) = active_file(app_state) else {
        return String::new();
    };

    app_state.editor_buffers.get().get(&file.key).cloned().unwrap_or(file.initial_content)
}
