use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum AuthoringLanguage {
    JavaScript,
    Lua,
}

impl AuthoringLanguage {
    const ALL: [Self; 2] = [Self::JavaScript, Self::Lua];

    const fn manifest_name(self) -> &'static str {
        match self {
            Self::JavaScript => "JavaScript",
            Self::Lua => "Lua",
        }
    }

    const fn template_dir(self) -> &'static str {
        match self {
            Self::JavaScript => "examples/js",
            Self::Lua => "examples/lua",
        }
    }

    const fn config_candidates(self) -> &'static [&'static str] {
        match self {
            Self::JavaScript => &["config.ts", "config.tsx"],
            Self::Lua => &["config.lua"],
        }
    }
}

#[derive(Debug, Clone)]
struct FileSpec {
    variant: String,
    label: String,
    workspace_path: String,
    runtime_path: String,
    include_path: String,
    language: &'static str,
    is_layout_source: bool,
    is_reference_only: bool,
}

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("playground crate should be under workspace root");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("out dir"));

    let mut manifests = BTreeMap::new();
    for language in AuthoringLanguage::ALL {
        let template_root = workspace_root.join(language.template_dir());
        println!("cargo:rerun-if-changed={}", template_root.display());
        manifests.insert(language, collect_specs(language, &template_root));
    }

    let generated = render_manifest(&manifests);
    fs::write(out_dir.join("editor_files_manifest.rs"), generated)
        .expect("write editor files manifest");
}

fn collect_specs(language: AuthoringLanguage, template_root: &Path) -> Vec<FileSpec> {
    let mut specs = Vec::new();

    let config_relative = language
        .config_candidates()
        .iter()
        .find(|candidate| template_root.join(candidate).is_file())
        .unwrap_or_else(|| panic!("{} must contain a config entrypoint", template_root.display()));
    specs.push(build_spec(template_root, config_relative));

    if template_root.join("index.css").is_file() {
        specs.push(build_spec(template_root, "index.css"));
    }

    if language == AuthoringLanguage::Lua {
        specs.push(FileSpec {
            variant: "SdkLuaHypreact".to_string(),
            label: "hypreact.lua".to_string(),
            workspace_path: "~/.config/hypreact/.sdk/lua/hypreact.lua".to_string(),
            runtime_path: "/playground/.sdk/lua/hypreact.lua".to_string(),
            include_path: workspace_root_path(template_root)
                .join("packages/sdk/lua/hypreact.lua")
                .to_string_lossy()
                .replace('\\', "/"),
            language: "lua",
            is_layout_source: false,
            is_reference_only: true,
        });
    }

    let layouts_root = template_root.join("layouts");
    if layouts_root.is_dir() {
        let mut layout_files = Vec::new();
        collect_layout_files(template_root, &layouts_root, &mut layout_files);
        layout_files.sort_by(layout_sort_key);
        specs.extend(layout_files);
    }

    specs
}

fn collect_layout_files(template_root: &Path, directory: &Path, out: &mut Vec<FileSpec>) {
    let mut entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read_dir {}: {error}", directory.display()))
        .map(|entry| entry.expect("dir entry").path())
        .collect::<Vec<_>>();
    entries.sort();

    for path in entries {
        if path.is_dir() {
            collect_layout_files(template_root, &path, out);
            continue;
        }

        let relative = path
            .strip_prefix(template_root)
            .expect("layout file should be under template root")
            .to_string_lossy()
            .replace('\\', "/");

        let Some(language) = language_for_path(&relative) else {
            continue;
        };

        if !relative.starts_with("layouts/") {
            continue;
        }

        out.push(build_spec_with_language(template_root, &relative, language));
    }
}

fn build_spec(template_root: &Path, relative: &str) -> FileSpec {
    let language = language_for_path(relative)
        .unwrap_or_else(|| panic!("unsupported template file type: {relative}"));
    build_spec_with_language(template_root, relative, language)
}

fn build_spec_with_language(
    template_root: &Path,
    relative: &str,
    language: &'static str,
) -> FileSpec {
    let relative = relative.replace('\\', "/");
    let include_path = template_root.join(&relative).to_string_lossy().replace('\\', "/");
    let workspace_path = format!("~/.config/hypreact/{relative}");
    let runtime_path = format!("/playground/{relative}");
    let label = Path::new(&relative).file_name().expect("file name").to_string_lossy().to_string();

    FileSpec {
        variant: variant_name_for_relative(&relative),
        label,
        workspace_path,
        runtime_path,
        include_path,
        language,
        is_layout_source: relative.starts_with("layouts/")
            && matches!(language, "typescript" | "typescriptreact" | "lua"),
        is_reference_only: false,
    }
}

fn workspace_root_path(template_root: &Path) -> PathBuf {
    template_root
        .ancestors()
        .nth(2)
        .expect("template root should be under workspace/examples")
        .to_path_buf()
}

fn layout_sort_key(left: &FileSpec, right: &FileSpec) -> std::cmp::Ordering {
    let left_parent = Path::new(&left.runtime_path)
        .parent()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default();
    let right_parent = Path::new(&right.runtime_path)
        .parent()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default();

    left_parent
        .cmp(&right_parent)
        .then_with(|| file_name_priority(&left.label).cmp(&file_name_priority(&right.label)))
        .then_with(|| left.label.cmp(&right.label))
}

fn file_name_priority(label: &str) -> u8 {
    match label {
        "index.tsx" | "index.ts" | "index.lua" => 0,
        "index.css" => 1,
        _ => 2,
    }
}

fn language_for_path(relative: &str) -> Option<&'static str> {
    if relative.ends_with(".tsx") {
        Some("typescriptreact")
    } else if relative.ends_with(".ts") {
        Some("typescript")
    } else if relative.ends_with(".css") {
        Some("css")
    } else if relative.ends_with(".lua") {
        Some("lua")
    } else {
        None
    }
}

fn variant_name_for_relative(relative: &str) -> String {
    relative
        .split('/')
        .flat_map(|segment| segment.split(['-', '_', '.']))
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            let first = chars.next().expect("segment char");
            first.to_ascii_uppercase().to_string() + chars.as_str()
        })
        .collect::<String>()
}

fn render_language_manifest(language: AuthoringLanguage, specs: &[FileSpec]) -> String {
    let default_open = specs
        .iter()
        .find(|spec| spec.is_layout_source)
        .unwrap_or_else(|| specs.first().expect("at least one editor file"));
    let entry = specs.first().expect("config file should be first");

    let enum_variants = specs
        .iter()
        .map(|spec| format!("        {},", spec.variant))
        .collect::<Vec<_>>()
        .join("\n");
    let file_entries = specs
        .iter()
        .map(|spec| {
            format!(
                "        EditorFile {{ id: EditorFileId::{}, label: {:?}, path: {:?}, language: {:?} }},",
                spec.variant, spec.label, spec.workspace_path, spec.language
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let reference_only_arms = specs
        .iter()
        .map(|spec| {
            format!("            EditorFileId::{} => {},", spec.variant, spec.is_reference_only)
        })
        .collect::<Vec<_>>()
        .join("\n");
    let initial_content_arms = specs
        .iter()
        .map(|spec| {
            format!(
                "            EditorFileId::{} => include_str!({:?}),",
                spec.variant, spec.include_path
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let runtime_path_arms = specs
        .iter()
        .map(|spec| {
            format!("            EditorFileId::{} => {:?},", spec.variant, spec.runtime_path)
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "pub mod {} {{\n    use super::EditorFile;\n    use serde::{{Deserialize, Serialize}};\n\n    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]\n    pub enum EditorFileId {{\n{enum_variants}\n    }}\n\n    pub const ENTRY_RUNTIME_PATH: &str = {:?};\n    pub const DEFAULT_OPEN_FILE_ID: EditorFileId = EditorFileId::{};\n    pub const EDITOR_FILES: [EditorFile<EditorFileId>; {}] = [\n{file_entries}\n    ];\n\n    pub fn initial_open_editor_files() -> Vec<EditorFileId> {{\n        vec![DEFAULT_OPEN_FILE_ID]\n    }}\n\n    pub fn initial_content(file_id: EditorFileId) -> &'static str {{\n        match file_id {{\n{initial_content_arms}\n        }}\n    }}\n\n    pub fn runtime_path(file_id: EditorFileId) -> &'static str {{\n        match file_id {{\n{runtime_path_arms}\n        }}\n    }}\n\n    pub fn is_reference_only(file_id: EditorFileId) -> bool {{\n        match file_id {{\n{reference_only_arms}\n        }}\n    }}\n}}\n",
        language.manifest_name().to_ascii_lowercase(),
        entry.runtime_path,
        default_open.variant,
        specs.len(),
    )
}

fn render_manifest(manifests: &BTreeMap<AuthoringLanguage, Vec<FileSpec>>) -> String {
    let modules = AuthoringLanguage::ALL
        .into_iter()
        .map(|language| {
            render_language_manifest(language, manifests.get(&language).expect("manifest"))
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct EditorFile<Id> {{\n    pub id: Id,\n    pub label: &'static str,\n    pub path: &'static str,\n    pub language: &'static str,\n}}\n\n{modules}"
    )
}
