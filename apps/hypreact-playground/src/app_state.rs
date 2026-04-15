use std::collections::BTreeMap;

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::editor_files::{
    DEFAULT_OPEN_FILE_ID, EditorFileId, initial_content, initial_editor_buffers,
    initial_open_editor_files,
};
use crate::layout_runtime::EvaluatedPreview;
use crate::session::PreviewSessionState;
use crate::workspace::initial_open_directories;

const STORAGE_KEY: &str = "hypreact.playground.ui-state.v2";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedAppState {
    editor_buffers: BTreeMap<EditorFileId, String>,
    active_file_id: Option<EditorFileId>,
    open_file_ids: Vec<EditorFileId>,
    directory_open_state: BTreeMap<String, bool>,
    selected_workspace: Option<String>,
    preview_sidebar_open: bool,
}

#[derive(Clone, Copy)]
pub struct AppState {
    pub session: RwSignal<PreviewSessionState>,
    pub editor_buffers: RwSignal<BTreeMap<EditorFileId, String>>,
    pub active_file_id: RwSignal<Option<EditorFileId>>,
    pub open_file_ids: RwSignal<Vec<EditorFileId>>,
    pub directory_open_state: RwSignal<BTreeMap<String, bool>>,
    pub latest_config_request_key: RwSignal<String>,
    pub preview_eval_request: RwSignal<u64>,
    pub loaded_config: RwSignal<Option<hypreact_config::model::Config>>,
    pub preview_sidebar_open: RwSignal<bool>,
}

impl AppState {
    pub fn new() -> Self {
        let persisted = load_persisted_state();
        let buffers = persisted
            .as_ref()
            .map(|state| state.editor_buffers.clone())
            .unwrap_or_else(initial_editor_buffers);
        let active_file_id = persisted
            .as_ref()
            .and_then(|state| state.active_file_id)
            .or(Some(DEFAULT_OPEN_FILE_ID));
        let open_file_ids = persisted
            .as_ref()
            .map(|state| state.open_file_ids.clone())
            .filter(|open_files| !open_files.is_empty())
            .unwrap_or_else(initial_open_editor_files);
        let directory_open_state = persisted
            .as_ref()
            .map(|state| state.directory_open_state.clone())
            .unwrap_or_else(initial_open_directories);
        let preview_sidebar_open =
            persisted.as_ref().map(|state| state.preview_sidebar_open).unwrap_or(true);
        let mut session = PreviewSessionState::new();
        if let Some(workspace_name) =
            persisted.as_ref().and_then(|state| state.selected_workspace.clone())
        {
            session.select_workspace(&workspace_name);
        }

        Self {
            session: RwSignal::new(session),
            editor_buffers: RwSignal::new(buffers),
            active_file_id: RwSignal::new(active_file_id),
            open_file_ids: RwSignal::new(open_file_ids),
            directory_open_state: RwSignal::new(directory_open_state),
            latest_config_request_key: RwSignal::new(String::new()),
            preview_eval_request: RwSignal::new(1),
            loaded_config: RwSignal::new(None),
            preview_sidebar_open: RwSignal::new(preview_sidebar_open),
        }
    }

    pub fn update_buffer(&self, file_id: EditorFileId, next_value: String) {
        self.editor_buffers.update(|buffers| {
            buffers.insert(file_id, next_value);
        });
        self.persist_ui_state();
        self.request_preview_reevaluation();
    }

    pub fn apply_loaded_config(&self, config: hypreact_config::model::Config) {
        self.session.update(|state| state.apply_loaded_config(&config));
        self.loaded_config.set(Some(config));
        self.request_preview_reevaluation();
    }

    pub fn apply_config_error(&self, error: String) {
        self.loaded_config.set(None);
        self.session.update(|state| state.apply_preview_failure(error.clone()));
        self.request_preview_reevaluation();
    }

    pub fn apply_loaded_preview(&self, preview: EvaluatedPreview) {
        self.loaded_config.set(Some(preview.config.clone()));
        self.session.update(|state| state.apply_loaded_preview(preview));
        self.persist_ui_state();
    }

    pub fn apply_preview_failure(&self, error: String) {
        self.session.update(|state| state.apply_preview_failure(error));
    }

    pub fn request_preview_reevaluation(&self) {
        self.preview_eval_request.update(|value| *value += 1);
    }

    pub fn select_editor_file(&self, file_id: EditorFileId) {
        self.open_file_ids.update(|open_files| {
            if !open_files.contains(&file_id) {
                open_files.push(file_id);
            }
        });
        self.active_file_id.set(Some(file_id));
        self.persist_ui_state();
    }

    pub fn close_editor_file(&self, file_id: EditorFileId) {
        self.open_file_ids.update(|open_files| {
            let Some(index) = open_files.iter().position(|open_file_id| *open_file_id == file_id)
            else {
                return;
            };

            open_files.remove(index);
            if self.active_file_id.get_untracked() == Some(file_id) {
                let next_file_id = open_files.get(index.saturating_sub(1)).copied();
                self.active_file_id.set(next_file_id);
            }
        });
        self.persist_ui_state();
    }

    pub fn close_other_editor_files(&self, file_id: EditorFileId) {
        self.open_file_ids.set(vec![file_id]);
        self.active_file_id.set(Some(file_id));
        self.persist_ui_state();
    }

    pub fn close_all_editor_files(&self) {
        self.open_file_ids.set(Vec::new());
        self.active_file_id.set(None);
        self.persist_ui_state();
    }

    pub fn toggle_directory(&self, path: String, default_open: bool) {
        self.directory_open_state.update(|state| {
            let next_value = !state.get(&path).copied().unwrap_or(default_open);
            state.insert(path, next_value);
        });
        self.persist_ui_state();
    }

    pub fn reset_active_buffer(&self) {
        let Some(file_id) = self.active_file_id.get_untracked() else {
            return;
        };
        self.editor_buffers.update(|buffers| {
            buffers.insert(file_id, initial_content(file_id).to_string());
        });
        self.persist_ui_state();
        self.request_preview_reevaluation();
    }

    pub fn toggle_preview_sidebar(&self) {
        self.preview_sidebar_open.update(|open| *open = !*open);
        self.persist_ui_state();
    }

    pub fn persist_ui_state(&self) {
        let state = PersistedAppState {
            editor_buffers: self.editor_buffers.get_untracked(),
            active_file_id: self.active_file_id.get_untracked(),
            open_file_ids: self.open_file_ids.get_untracked(),
            directory_open_state: self.directory_open_state.get_untracked(),
            selected_workspace: Some(self.session.get_untracked().active_workspace_name()),
            preview_sidebar_open: self.preview_sidebar_open.get_untracked(),
        };
        persist_state(&state);
    }
}

fn load_persisted_state() -> Option<PersistedAppState> {
    let window = web_sys::window()?;
    let storage = window.local_storage().ok()??;
    let raw = storage.get_item(STORAGE_KEY).ok()??;
    serde_json::from_str(&raw).ok()
}

fn persist_state(state: &PersistedAppState) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return;
    };
    let Ok(raw) = serde_json::to_string(state) else {
        return;
    };
    let _ = storage.set_item(STORAGE_KEY, &raw);
}
