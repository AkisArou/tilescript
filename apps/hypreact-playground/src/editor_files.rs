use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AuthoringLanguage {
    JavaScript,
    Lua,
    Fennel,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EditorFileKey {
    StaticJavaScript(JavaScriptStaticEditorFileId),
    StaticLua(LuaStaticEditorFileId),
    StaticFennel(FennelStaticEditorFileId),
    Dynamic(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicEditorFile {
    pub key: EditorFileKey,
    pub label: String,
    pub path: String,
    pub language: String,
    pub initial_content: String,
    pub is_reference_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicLayoutFileSet {
    pub language: AuthoringLanguage,
    pub name: String,
    pub directory_path: String,
    pub files: Vec<DynamicEditorFile>,
}

pub const WORKSPACE_ROOT: &str = "~/.config/hypreact";
pub const WORKSPACE_FS_ROOT: &str = "/home/demo/.config/hypreact";

mod generated {
    include!(concat!(env!("OUT_DIR"), "/editor_files_manifest.rs"));
}

pub use generated::fennel::EditorFileId as FennelStaticEditorFileId;
pub use generated::javascript::EditorFileId as JavaScriptStaticEditorFileId;
pub use generated::lua::EditorFileId as LuaStaticEditorFileId;

pub fn default_authoring_language() -> AuthoringLanguage {
    AuthoringLanguage::JavaScript
}

pub fn model_path(file_key: &EditorFileKey, dynamic_layouts: &[DynamicLayoutFileSet]) -> String {
    format!("file://{}", runtime_path(file_key, dynamic_layouts))
}

pub fn entry_runtime_path(language: AuthoringLanguage) -> &'static str {
    match language {
        AuthoringLanguage::JavaScript => generated::javascript::ENTRY_RUNTIME_PATH,
        AuthoringLanguage::Lua => generated::lua::ENTRY_RUNTIME_PATH,
        AuthoringLanguage::Fennel => generated::fennel::ENTRY_RUNTIME_PATH,
    }
}

pub fn runtime_path(file_key: &EditorFileKey, dynamic_layouts: &[DynamicLayoutFileSet]) -> String {
    match file_key {
        EditorFileKey::StaticJavaScript(file_id) => {
            generated::javascript::runtime_path(*file_id).to_string()
        }
        EditorFileKey::StaticLua(file_id) => generated::lua::runtime_path(*file_id).to_string(),
        EditorFileKey::StaticFennel(file_id) => {
            generated::fennel::runtime_path(*file_id).to_string()
        }
        EditorFileKey::Dynamic(key) => iter_dynamic_files(dynamic_layouts)
            .find(|file| matches!(&file.key, EditorFileKey::Dynamic(candidate) if candidate == key))
            .map(|file| workspace_path_to_runtime_path(&file.path))
            .unwrap_or_else(|| panic!("unknown dynamic editor file key `{key}`")),
    }
}

pub fn file_key_by_model_path(
    path: &str,
    language: AuthoringLanguage,
    dynamic_layouts: &[DynamicLayoutFileSet],
) -> Option<EditorFileKey> {
    static_files(language)
        .iter()
        .find(|file| model_path(&static_key(language, file.id()), dynamic_layouts) == path)
        .map(|file| static_key(language, file.id()))
        .or_else(|| {
            iter_dynamic_files(dynamic_layouts)
                .filter(|file| file_layout_language(&file.key, dynamic_layouts) == Some(language))
                .find(|file| model_path(&file.key, dynamic_layouts) == path)
                .map(|file| file.key.clone())
        })
}

pub fn file_badge(language: &str) -> &'static str {
    match language {
        "css" => "css",
        "typescriptreact" => "tsx",
        "typescript" => "ts",
        "lua" => "lua",
        "fennel" => "fnl",
        _ => "txt",
    }
}

pub fn file_icon(language: &str) -> &'static str {
    match language {
        "css" => "",
        "typescript" | "typescriptreact" => "󰛦",
        "lua" => "",
        "fennel" => "󱘎",
        _ => "󰈔",
    }
}

pub fn file_color_class(language: &str) -> &'static str {
    match language {
        "css" => "text-[var(--color-editor-file-css)]",
        "typescript" | "typescriptreact" => "text-[var(--color-editor-file-typescript)]",
        "lua" => "text-[var(--color-editor-file-lua)]",
        "fennel" => "text-[var(--color-editor-file-fennel)]",
        _ => "text-terminal-info",
    }
}

pub fn file_display_icon(language: &str, is_reference_only: bool) -> &'static str {
    if is_reference_only { "󰘦" } else { file_icon(language) }
}

pub fn file_display_color_class(language: &str, is_reference_only: bool) -> &'static str {
    if is_reference_only { "text-terminal-faint" } else { file_color_class(language) }
}

pub fn file_display_badge(language: &str, is_reference_only: bool) -> &'static str {
    if is_reference_only { "sdk" } else { file_badge(language) }
}

pub fn initial_editor_buffers(language: AuthoringLanguage) -> BTreeMap<EditorFileKey, String> {
    static_files(language)
        .iter()
        .map(|file| {
            (
                static_key(language, file.id()),
                initial_static_content(language, file.id()).to_string(),
            )
        })
        .collect()
}

pub fn file_by_key(
    file_key: &EditorFileKey,
    dynamic_layouts: &[DynamicLayoutFileSet],
) -> EditorFileMeta {
    match file_key {
        EditorFileKey::StaticJavaScript(file_id) => {
            let file = generated::javascript::EDITOR_FILES
                .iter()
                .find(|file| file.id == *file_id)
                .expect("static editor file should exist");
            EditorFileMeta {
                key: EditorFileKey::StaticJavaScript(*file_id),
                label: file.label.to_string(),
                path: file.path.to_string(),
                language: file.language.to_string(),
                initial_content: generated::javascript::initial_content(*file_id).to_string(),
                is_dynamic: false,
                is_reference_only: generated::javascript::is_reference_only(*file_id),
            }
        }
        EditorFileKey::StaticLua(file_id) => {
            let file = generated::lua::EDITOR_FILES
                .iter()
                .find(|file| file.id == *file_id)
                .expect("static editor file should exist");
            EditorFileMeta {
                key: EditorFileKey::StaticLua(*file_id),
                label: file.label.to_string(),
                path: file.path.to_string(),
                language: file.language.to_string(),
                initial_content: generated::lua::initial_content(*file_id).to_string(),
                is_dynamic: false,
                is_reference_only: generated::lua::is_reference_only(*file_id),
            }
        }
        EditorFileKey::StaticFennel(file_id) => {
            let file = generated::fennel::EDITOR_FILES
                .iter()
                .find(|file| file.id == *file_id)
                .expect("static editor file should exist");
            EditorFileMeta {
                key: EditorFileKey::StaticFennel(*file_id),
                label: file.label.to_string(),
                path: file.path.to_string(),
                language: file.language.to_string(),
                initial_content: generated::fennel::initial_content(*file_id).to_string(),
                is_dynamic: false,
                is_reference_only: generated::fennel::is_reference_only(*file_id),
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
                is_reference_only: file.is_reference_only,
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

pub fn initial_open_editor_files(language: AuthoringLanguage) -> Vec<EditorFileKey> {
    match language {
        AuthoringLanguage::JavaScript => generated::javascript::initial_open_editor_files()
            .into_iter()
            .map(EditorFileKey::StaticJavaScript)
            .collect(),
        AuthoringLanguage::Lua => generated::lua::initial_open_editor_files()
            .into_iter()
            .map(EditorFileKey::StaticLua)
            .collect(),
        AuthoringLanguage::Fennel => generated::fennel::initial_open_editor_files()
            .into_iter()
            .map(EditorFileKey::StaticFennel)
            .collect(),
    }
}

pub fn make_dynamic_layout(language: AuthoringLanguage, layout_name: &str) -> DynamicLayoutFileSet {
    let normalized = normalize_layout_name(layout_name);
    let directory_path = format!("{WORKSPACE_ROOT}/layouts/{normalized}");
    let base_key = format!(
        "layout:{}:{normalized}",
        match language {
            AuthoringLanguage::JavaScript => "javascript",
            AuthoringLanguage::Lua => "lua",
            AuthoringLanguage::Fennel => "fennel",
        }
    );
    let source_file = match language {
        AuthoringLanguage::JavaScript => DynamicEditorFile {
            key: EditorFileKey::Dynamic(format!("{base_key}:tsx")),
            label: "index.tsx".to_string(),
            path: format!("{directory_path}/index.tsx"),
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
            is_reference_only: false,
        },
        AuthoringLanguage::Lua => DynamicEditorFile {
            key: EditorFileKey::Dynamic(format!("{base_key}:lua")),
            label: "index.lua".to_string(),
            path: format!("{directory_path}/index.lua"),
            language: "lua".to_string(),
            initial_content: concat!(
                "local h = require(\"hypreact\")\n\n",
                "---@param ctx Hypreact.LayoutContext\n",
                "return function(ctx)\n",
                "  return h.workspace() {\n",
                "    h.slot(),\n",
                "  }\n",
                "end\n",
            )
            .to_string(),
            is_reference_only: false,
        },
        AuthoringLanguage::Fennel => DynamicEditorFile {
            key: EditorFileKey::Dynamic(format!("{base_key}:fnl")),
            label: "index.fnl".to_string(),
            path: format!("{directory_path}/index.fnl"),
            language: "fennel".to_string(),
            initial_content: concat!(
                "(local h (require \"hypreact\"))\n\n",
                "(fn [ctx]\n",
                "  ((h.workspace {})\n",
                "   [(h.slot {})]))\n",
            )
            .to_string(),
            is_reference_only: false,
        },
    };
    let css_file = DynamicEditorFile {
        key: EditorFileKey::Dynamic(format!("{base_key}:css")),
        label: "index.css".to_string(),
        path: format!("{directory_path}/index.css"),
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
        is_reference_only: false,
    };

    DynamicLayoutFileSet {
        language,
        name: normalized.clone(),
        directory_path,
        files: vec![source_file, css_file],
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
    pub is_reference_only: bool,
}

pub enum StaticEditorFileRef {
    JavaScript(&'static generated::EditorFile<JavaScriptStaticEditorFileId>),
    Lua(&'static generated::EditorFile<LuaStaticEditorFileId>),
    Fennel(&'static generated::EditorFile<FennelStaticEditorFileId>),
}

impl StaticEditorFileRef {
    pub const fn id(&self) -> StaticFileId {
        match self {
            Self::JavaScript(file) => StaticFileId::JavaScript(file.id),
            Self::Lua(file) => StaticFileId::Lua(file.id),
            Self::Fennel(file) => StaticFileId::Fennel(file.id),
        }
    }

    pub const fn key(&self) -> EditorFileKey {
        static_file_key(self.id())
    }

    pub const fn path(&self) -> &'static str {
        match self {
            Self::JavaScript(file) => file.path,
            Self::Lua(file) => file.path,
            Self::Fennel(file) => file.path,
        }
    }

    pub const fn language(&self) -> &'static str {
        match self {
            Self::JavaScript(file) => file.language,
            Self::Lua(file) => file.language,
            Self::Fennel(file) => file.language,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaticFileId {
    JavaScript(JavaScriptStaticEditorFileId),
    Lua(LuaStaticEditorFileId),
    Fennel(FennelStaticEditorFileId),
}

pub const fn static_file_key(file_id: StaticFileId) -> EditorFileKey {
    match file_id {
        StaticFileId::JavaScript(id) => EditorFileKey::StaticJavaScript(id),
        StaticFileId::Lua(id) => EditorFileKey::StaticLua(id),
        StaticFileId::Fennel(id) => EditorFileKey::StaticFennel(id),
    }
}

pub fn static_files(language: AuthoringLanguage) -> Vec<StaticEditorFileRef> {
    match language {
        AuthoringLanguage::JavaScript => generated::javascript::EDITOR_FILES
            .iter()
            .map(|file| StaticEditorFileRef::JavaScript(file))
            .collect(),
        AuthoringLanguage::Lua => {
            generated::lua::EDITOR_FILES.iter().map(|file| StaticEditorFileRef::Lua(file)).collect()
        }
        AuthoringLanguage::Fennel => generated::fennel::EDITOR_FILES
            .iter()
            .map(|file| StaticEditorFileRef::Fennel(file))
            .collect(),
    }
}

pub fn iter_dynamic_files(
    dynamic_layouts: &[DynamicLayoutFileSet],
) -> impl Iterator<Item = &DynamicEditorFile> {
    dynamic_layouts.iter().flat_map(|layout| layout.files.iter())
}

pub fn file_layout_language(
    file_key: &EditorFileKey,
    dynamic_layouts: &[DynamicLayoutFileSet],
) -> Option<AuthoringLanguage> {
    match file_key {
        EditorFileKey::StaticJavaScript(_) => Some(AuthoringLanguage::JavaScript),
        EditorFileKey::StaticLua(_) => Some(AuthoringLanguage::Lua),
        EditorFileKey::StaticFennel(_) => Some(AuthoringLanguage::Fennel),
        EditorFileKey::Dynamic(_) => dynamic_layouts.iter().find_map(|layout| {
            layout.files.iter().any(|file| &file.key == file_key).then_some(layout.language)
        }),
    }
}

fn initial_static_content(language: AuthoringLanguage, file_id: StaticFileId) -> &'static str {
    match (language, file_id) {
        (AuthoringLanguage::JavaScript, StaticFileId::JavaScript(file_id)) => {
            generated::javascript::initial_content(file_id)
        }
        (AuthoringLanguage::Lua, StaticFileId::Lua(file_id)) => {
            generated::lua::initial_content(file_id)
        }
        (AuthoringLanguage::Fennel, StaticFileId::Fennel(file_id)) => {
            generated::fennel::initial_content(file_id)
        }
        _ => unreachable!("static file id must match authoring language"),
    }
}

fn static_key(language: AuthoringLanguage, file_id: impl Into<StaticFileId>) -> EditorFileKey {
    match (language, file_id.into()) {
        (AuthoringLanguage::JavaScript, StaticFileId::JavaScript(file_id)) => {
            static_file_key(StaticFileId::JavaScript(file_id))
        }
        (AuthoringLanguage::Lua, StaticFileId::Lua(file_id)) => {
            static_file_key(StaticFileId::Lua(file_id))
        }
        (AuthoringLanguage::Fennel, StaticFileId::Fennel(file_id)) => {
            static_file_key(StaticFileId::Fennel(file_id))
        }
        _ => unreachable!("static file id must match authoring language"),
    }
}

impl From<JavaScriptStaticEditorFileId> for StaticFileId {
    fn from(value: JavaScriptStaticEditorFileId) -> Self {
        Self::JavaScript(value)
    }
}

impl From<LuaStaticEditorFileId> for StaticFileId {
    fn from(value: LuaStaticEditorFileId) -> Self {
        Self::Lua(value)
    }
}

impl From<FennelStaticEditorFileId> for StaticFileId {
    fn from(value: FennelStaticEditorFileId) -> Self {
        Self::Fennel(value)
    }
}
