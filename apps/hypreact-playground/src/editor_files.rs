use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorFile {
    pub id: StaticEditorFileId,
    pub label: &'static str,
    pub path: &'static str,
    pub language: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EditorFileKey {
    Static(StaticEditorFileId),
    Dynamic(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicEditorFile {
    pub key: EditorFileKey,
    pub label: String,
    pub path: String,
    pub language: String,
    pub initial_content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicLayoutFileSet {
    pub name: String,
    pub directory_path: String,
    pub files: Vec<DynamicEditorFile>,
}

pub const WORKSPACE_ROOT: &str = "~/.config/hypreact";
pub const WORKSPACE_FS_ROOT: &str = "/home/demo/.config/hypreact";

mod generated {
    use super::EditorFile;
    use serde::{Deserialize, Serialize};

    include!(concat!(env!("OUT_DIR"), "/editor_files_manifest.rs"));
}

pub use generated::{
    DEFAULT_OPEN_FILE_ID as DEFAULT_STATIC_OPEN_FILE_ID, EDITOR_FILES, ENTRY_RUNTIME_PATH,
    EditorFileId as StaticEditorFileId,
};

pub fn model_path(file_key: &EditorFileKey, dynamic_layouts: &[DynamicLayoutFileSet]) -> String {
    format!("file://{}", runtime_path(file_key, dynamic_layouts))
}

pub fn runtime_path(file_key: &EditorFileKey, dynamic_layouts: &[DynamicLayoutFileSet]) -> String {
    match file_key {
        EditorFileKey::Static(file_id) => generated::runtime_path(*file_id).to_string(),
        EditorFileKey::Dynamic(key) => iter_dynamic_files(dynamic_layouts)
            .find(|file| matches!(&file.key, EditorFileKey::Dynamic(candidate) if candidate == key))
            .map(|file| workspace_path_to_runtime_path(&file.path))
            .unwrap_or_else(|| format!("/playground/layouts/{key}/index.tsx")),
    }
}

pub fn file_key_by_model_path(
    path: &str,
    dynamic_layouts: &[DynamicLayoutFileSet],
) -> Option<EditorFileKey> {
    EDITOR_FILES
        .iter()
        .find(|file| model_path(&EditorFileKey::Static(file.id), dynamic_layouts) == path)
        .map(|file| EditorFileKey::Static(file.id))
        .or_else(|| {
            iter_dynamic_files(dynamic_layouts)
                .find(|file| model_path(&file.key, dynamic_layouts) == path)
                .map(|file| file.key.clone())
        })
}

pub fn file_badge(language: &str) -> &'static str {
    match language {
        "css" => "css",
        "typescriptreact" => "tsx",
        "typescript" => "ts",
        _ => "txt",
    }
}

pub fn initial_editor_buffers() -> BTreeMap<EditorFileKey, String> {
    EDITOR_FILES
        .iter()
        .map(|file| {
            (EditorFileKey::Static(file.id), generated::initial_content(file.id).to_string())
        })
        .collect()
}

pub fn file_by_key(
    file_key: &EditorFileKey,
    dynamic_layouts: &[DynamicLayoutFileSet],
) -> EditorFileMeta {
    match file_key {
        EditorFileKey::Static(file_id) => {
            let file = EDITOR_FILES
                .iter()
                .find(|file| file.id == *file_id)
                .expect("static editor file should exist");
            EditorFileMeta {
                key: EditorFileKey::Static(*file_id),
                label: file.label.to_string(),
                path: file.path.to_string(),
                language: file.language.to_string(),
                initial_content: generated::initial_content(*file_id).to_string(),
                is_dynamic: false,
            }
        }
        EditorFileKey::Dynamic(key) => {
            let file = iter_dynamic_files(dynamic_layouts)
                .find(|file| matches!(&file.key, EditorFileKey::Dynamic(candidate) if candidate == key))
                .expect("dynamic editor file should exist");
            EditorFileMeta {
                key: file.key.clone(),
                label: file.label.clone(),
                path: file.path.clone(),
                language: file.language.clone(),
                initial_content: file.initial_content.clone(),
                is_dynamic: true,
            }
        }
    }
}

pub fn initial_content_for_key(
    file_key: &EditorFileKey,
    dynamic_layouts: &[DynamicLayoutFileSet],
) -> String {
    file_by_key(file_key, dynamic_layouts).initial_content
}

pub fn initial_open_editor_files() -> Vec<EditorFileKey> {
    vec![EditorFileKey::Static(DEFAULT_STATIC_OPEN_FILE_ID)]
}

pub fn make_dynamic_layout(layout_name: &str) -> DynamicLayoutFileSet {
    let normalized = normalize_layout_name(layout_name);
    let directory_path = format!("{WORKSPACE_ROOT}/layouts/{normalized}");
    let base_key = format!("layout:{normalized}");
    let tsx_path = format!("{directory_path}/index.tsx");
    let css_path = format!("{directory_path}/index.css");

    DynamicLayoutFileSet {
        name: normalized.clone(),
        directory_path,
        files: vec![
            DynamicEditorFile {
                key: EditorFileKey::Dynamic(format!("{base_key}:tsx")),
                label: "index.tsx".to_string(),
                path: tsx_path,
                language: "typescriptreact".to_string(),
                initial_content: concat!(
                    "import type { LayoutContext } from \"@hypreact/sdk/layout\";\n\n",
                    "import \"./index.css\";\n\n",
                    "export default function layout(ctx: LayoutContext) {\n",
                    "  return (\n",
                    "    <workspace>\n",
                    "      <slot />\n",
                    "    </workspace>\n",
                    "  );\n",
                    "}\n",
                )
                .to_string(),
            },
            DynamicEditorFile {
                key: EditorFileKey::Dynamic(format!("{base_key}:css")),
                label: "index.css".to_string(),
                path: css_path,
                language: "css".to_string(),
                initial_content: concat!(
                    "workspace {\n",
                    "  display: flex;\n",
                    "  flex-direction: row;\n",
                    "  gap: 6px;\n",
                    "  padding: 6px;\n",
                    "  width: 100%;\n",
                    "  height: 100%;\n",
                    "}\n\n",
                    "window {\n",
                    "  flex: 1;\n",
                    "}\n",
                )
                .to_string(),
            },
        ],
    }
}

pub fn normalize_layout_name(layout_name: &str) -> String {
    let normalized = layout_name
        .trim()
        .chars()
        .map(|ch| match ch {
            'a'..='z' | '0'..='9' => ch,
            'A'..='Z' => ch.to_ascii_lowercase(),
            _ => '-',
        })
        .collect::<String>();

    let collapsed =
        normalized.split('-').filter(|segment| !segment.is_empty()).collect::<Vec<_>>().join("-");

    if collapsed.is_empty() { "layout".to_string() } else { collapsed }
}

pub fn workspace_path_to_runtime_path(path: &str) -> String {
    path.strip_prefix(&format!("{WORKSPACE_ROOT}/"))
        .map(|relative| format!("/playground/{relative}"))
        .unwrap_or_else(|| path.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorFileMeta {
    pub key: EditorFileKey,
    pub label: String,
    pub path: String,
    pub language: String,
    pub initial_content: String,
    pub is_dynamic: bool,
}

pub fn static_files() -> &'static [EditorFile] {
    &EDITOR_FILES
}

pub fn iter_dynamic_files(
    dynamic_layouts: &[DynamicLayoutFileSet],
) -> impl Iterator<Item = &DynamicEditorFile> {
    dynamic_layouts.iter().flat_map(|layout| layout.files.iter())
}
