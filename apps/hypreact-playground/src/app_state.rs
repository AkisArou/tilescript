use std::collections::BTreeMap;

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::editor_files::{
    AuthoringLanguage, DynamicLayoutFileSet, EditorFileKey, default_authoring_language,
    initial_editor_buffers, initial_open_editor_files,
};
use crate::layout_runtime::EvaluatedPreview;
use crate::session::PreviewSessionState;
use crate::workspace_tree::initial_open_directories;

const STORAGE_KEY: &str = "hypreact.playground.ui-state.v3";

const fn default_preview_animations_enabled() -> bool {
    true
}

const fn default_vim_mode_enabled() -> bool {
    false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedAppState {
    authoring_language: AuthoringLanguage,
    editor_buffers: BTreeMap<EditorFileKey, String>,
    active_file_id: Option<EditorFileKey>,
    open_file_ids: Vec<EditorFileKey>,
    directory_open_state: BTreeMap<String, bool>,
    selected_workspace: Option<String>,
    dynamic_layouts: Vec<DynamicLayoutFileSet>,
    #[serde(default = "default_preview_animations_enabled")]
    preview_animations_enabled: bool,
    #[serde(default = "default_vim_mode_enabled")]
    vim_mode_enabled: bool,
}

#[derive(Clone, Copy)]
pub struct AppState {
    pub authoring_language: RwSignal<AuthoringLanguage>,
    pub session: RwSignal<PreviewSessionState>,
    pub editor_buffers: RwSignal<BTreeMap<EditorFileKey, String>>,
    pub active_file_id: RwSignal<Option<EditorFileKey>>,
    pub open_file_ids: RwSignal<Vec<EditorFileKey>>,
    pub directory_open_state: RwSignal<BTreeMap<String, bool>>,
    pub dynamic_layouts: RwSignal<Vec<DynamicLayoutFileSet>>,
    pub preview_animations_enabled: RwSignal<bool>,
    pub vim_mode_enabled: RwSignal<bool>,
    pub latest_config_request_id: RwSignal<u64>,
    pub preview_eval_request: RwSignal<u64>,
    pub loaded_config: RwSignal<Option<hypreact_config::model::Config>>,
}

impl AppState {
    pub fn new() -> Self {
        let persisted = load_persisted_state();
        let authoring_language = persisted
            .as_ref()
            .map(|state| state.authoring_language)
            .unwrap_or_else(default_authoring_language);
        let buffers = persisted
            .as_ref()
            .map(|state| state.editor_buffers.clone())
            .unwrap_or_else(|| initial_editor_buffers(authoring_language));
        let dynamic_layouts =
            persisted.as_ref().map(|state| state.dynamic_layouts.clone()).unwrap_or_default();
        let preview_animations_enabled = persisted
            .as_ref()
            .map(|state| state.preview_animations_enabled)
            .unwrap_or(default_preview_animations_enabled());
        let vim_mode_enabled = persisted
            .as_ref()
            .map(|state| state.vim_mode_enabled)
            .unwrap_or(default_vim_mode_enabled());
        let active_file_id = persisted
            .as_ref()
            .and_then(|state| state.active_file_id.clone())
            .or_else(|| initial_open_editor_files(authoring_language).first().cloned());
        let open_file_ids = persisted
            .as_ref()
            .map(|state| state.open_file_ids.clone())
            .filter(|open_files| !open_files.is_empty())
            .unwrap_or_else(|| initial_open_editor_files(authoring_language));
        let directory_open_state = persisted
            .as_ref()
            .map(|state| state.directory_open_state.clone())
            .unwrap_or_else(initial_open_directories);
        let mut session = PreviewSessionState::new();
        if let Some(workspace_name) =
            persisted.as_ref().and_then(|state| state.selected_workspace.clone())
        {
            session.select_workspace(&workspace_name);
        }

        Self {
            authoring_language: RwSignal::new(authoring_language),
            session: RwSignal::new(session),
            editor_buffers: RwSignal::new(buffers),
            active_file_id: RwSignal::new(active_file_id),
            open_file_ids: RwSignal::new(open_file_ids),
            directory_open_state: RwSignal::new(directory_open_state),
            dynamic_layouts: RwSignal::new(dynamic_layouts),
            preview_animations_enabled: RwSignal::new(preview_animations_enabled),
            vim_mode_enabled: RwSignal::new(vim_mode_enabled),
            latest_config_request_id: RwSignal::new(1),
            preview_eval_request: RwSignal::new(1),
            loaded_config: RwSignal::new(None),
        }
    }

    pub fn update_buffer(&self, file_id: EditorFileKey, next_value: String) {
        self.editor_buffers.update(|buffers| {
            buffers.insert(file_id, next_value);
        });
        self.persist_ui_state();
        self.request_config_reload();
    }

    pub fn set_authoring_language(&self, next_language: AuthoringLanguage) {
        if self.authoring_language.get_untracked() == next_language {
            return;
        }

        self.authoring_language.set(next_language);
        self.open_file_ids.set(initial_open_editor_files(next_language));
        self.active_file_id.set(initial_open_editor_files(next_language).first().cloned());
        self.persist_ui_state();
        self.request_config_reload();
    }

    pub fn apply_loaded_config(&self, config: hypreact_config::model::Config) {
        self.session.update(|state| state.apply_loaded_config(&config));
        self.loaded_config.set(Some(config));
        self.request_preview_reevaluation();
    }

    pub fn apply_config_error(&self, error: String) {
        self.session.update(|state| state.apply_preview_failure(error.clone()));
        self.request_preview_reevaluation();
    }

    pub fn apply_loaded_preview(&self, preview: EvaluatedPreview) {
        self.session.update(|state| state.apply_loaded_preview(preview));
        self.persist_ui_state();
    }

    pub fn apply_preview_failure(&self, error: String) {
        self.session.update(|state| state.apply_preview_failure(error));
    }

    pub fn request_preview_reevaluation(&self) {
        self.preview_eval_request.update(|value| *value += 1);
    }

    pub fn request_config_reload(&self) {
        self.latest_config_request_id.update(|value| *value += 1);
    }

    pub fn select_editor_file(&self, file_id: EditorFileKey) {
        self.open_file_ids.update(|open_files| {
            if !open_files.contains(&file_id) {
                open_files.push(file_id.clone());
            }
        });
        self.active_file_id.set(Some(file_id));
        self.persist_ui_state();
    }

    pub fn close_editor_file(&self, file_id: EditorFileKey) {
        self.open_file_ids.update(|open_files| {
            let Some(index) = open_files.iter().position(|open_file_id| *open_file_id == file_id)
            else {
                return;
            };

            open_files.remove(index);
            if self.active_file_id.get_untracked() == Some(file_id) {
                let next_file_id = open_files.get(index.saturating_sub(1)).cloned();
                self.active_file_id.set(next_file_id);
            }
        });
        self.persist_ui_state();
    }

    pub fn close_other_editor_files(&self, file_id: EditorFileKey) {
        self.open_file_ids.set(vec![file_id.clone()]);
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

    pub fn set_preview_animations_enabled(&self, enabled: bool) {
        if self.preview_animations_enabled.get_untracked() == enabled {
            return;
        }

        self.preview_animations_enabled.set(enabled);
        self.persist_ui_state();
    }

    pub fn set_vim_mode_enabled(&self, enabled: bool) {
        if self.vim_mode_enabled.get_untracked() == enabled {
            return;
        }

        self.vim_mode_enabled.set(enabled);
        self.persist_ui_state();
    }

    pub fn create_layout(&self, layout: DynamicLayoutFileSet) {
        self.dynamic_layouts.update(|layouts| {
            layouts.retain(|candidate| candidate.name != layout.name);
            layouts.push(layout.clone());
            layouts.sort_by(|left, right| left.name.cmp(&right.name));
        });
        self.editor_buffers.update(|buffers| {
            for file in &layout.files {
                buffers.insert(file.key.clone(), file.initial_content.clone());
            }
        });
        self.directory_open_state.update(|state| {
            state.insert(format!("{}/layouts", crate::editor_files::WORKSPACE_ROOT), true);
            state.insert(layout.directory_path.clone(), true);
        });

        if let Some(first_file) = layout.files.first() {
            self.select_editor_file(first_file.key.clone());
        } else {
            self.persist_ui_state();
            self.request_config_reload();
        }
    }

    pub fn persist_ui_state(&self) {
        let state = PersistedAppState {
            authoring_language: self.authoring_language.get_untracked(),
            editor_buffers: self.editor_buffers.get_untracked(),
            active_file_id: self.active_file_id.get_untracked(),
            open_file_ids: self.open_file_ids.get_untracked(),
            directory_open_state: self.directory_open_state.get_untracked(),
            selected_workspace: Some(self.session.get_untracked().active_workspace_name()),
            dynamic_layouts: self.dynamic_layouts.get_untracked(),
            preview_animations_enabled: self.preview_animations_enabled.get_untracked(),
            vim_mode_enabled: self.vim_mode_enabled.get_untracked(),
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
