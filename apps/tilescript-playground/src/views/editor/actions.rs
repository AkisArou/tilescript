use crate::editor_files::{EDITOR_FILES, EditorFileId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorActionId {
    OpenFile(EditorFileId),
    ResetActiveFile,
    CloseActiveFile,
    CloseOtherFiles,
    CloseAllFiles,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorActionEntry {
    pub id: EditorActionId,
    pub label: String,
    pub detail: String,
}

pub fn command_palette_entries() -> Vec<EditorActionEntry> {
    let mut entries = EDITOR_FILES
        .iter()
        .map(|file| EditorActionEntry {
            id: EditorActionId::OpenFile(file.id),
            label: format!("Open {}", file.path),
            detail: format!("focus {}", file.label),
        })
        .collect::<Vec<_>>();

    entries.extend([
        EditorActionEntry {
            id: EditorActionId::ResetActiveFile,
            label: "Reset active file".to_string(),
            detail: "restore fixture contents".to_string(),
        },
        EditorActionEntry {
            id: EditorActionId::CloseActiveFile,
            label: "Close active file".to_string(),
            detail: "close current tab".to_string(),
        },
        EditorActionEntry {
            id: EditorActionId::CloseOtherFiles,
            label: "Close other files".to_string(),
            detail: "keep only current tab".to_string(),
        },
        EditorActionEntry {
            id: EditorActionId::CloseAllFiles,
            label: "Close all files".to_string(),
            detail: "clear all tabs".to_string(),
        },
    ]);

    entries
}
