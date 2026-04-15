use wasm_bindgen::JsCast;
use web_sys::js_sys;

use crate::editor_files::{EDITOR_FILES, EditorFileId, file_by_id, initial_content};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Text,
    Json,
}

impl ExportFormat {
    pub fn label(self) -> &'static str {
        match self {
            Self::Text => "bundle text",
            Self::Json => "bundle json",
        }
    }
}

pub fn export_source_bundle(
    buffers: &std::collections::BTreeMap<EditorFileId, String>,
    format: ExportFormat,
) -> Result<(), String> {
    let (payload, filename, mime) = match format {
        ExportFormat::Text => {
            let mut archive = String::new();
            for file in EDITOR_FILES {
                let content = buffers
                    .get(&file.id)
                    .cloned()
                    .unwrap_or_else(|| initial_content(file.id).to_string());
                archive.push_str("===== ");
                archive.push_str(file.path);
                archive.push_str(" =====\n");
                archive.push_str(&content);
                if !content.ends_with('\n') {
                    archive.push('\n');
                }
                archive.push('\n');
            }
            (archive, "hypreact-playground-source-bundle.txt", "text/plain;charset=utf-8")
        }
        ExportFormat::Json => {
            let archive = EDITOR_FILES
                .iter()
                .map(|file| {
                    (
                        file.path,
                        buffers
                            .get(&file.id)
                            .cloned()
                            .unwrap_or_else(|| initial_content(file.id).to_string()),
                    )
                })
                .collect::<std::collections::BTreeMap<_, _>>();
            (
                serde_json::to_string_pretty(&archive)
                    .map_err(|_| "failed to serialize bundle json".to_string())?,
                "hypreact-playground-source-bundle.json",
                "application/json;charset=utf-8",
            )
        }
    };

    export_payload(&payload, filename, mime)
}

pub fn export_single_file(
    buffers: &std::collections::BTreeMap<EditorFileId, String>,
    file_id: EditorFileId,
) -> Result<(), String> {
    let file = file_by_id(file_id);
    let content =
        buffers.get(&file_id).cloned().unwrap_or_else(|| initial_content(file_id).to_string());
    let filename = file.path.rsplit('/').next().unwrap_or(file.label);
    export_payload(&content, filename, "text/plain;charset=utf-8")
}

fn export_payload(payload: &str, filename: &str, mime: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or_else(|| "missing window".to_string())?;
    let document = window.document().ok_or_else(|| "missing document".to_string())?;
    let array = js_sys::Array::new();
    array.push(&wasm_bindgen::JsValue::from_str(payload));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(mime);
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&array, &options)
        .map_err(|_| "failed to build blob".to_string())?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|_| "failed to create object url".to_string())?;

    let anchor = document
        .create_element("a")
        .map_err(|_| "failed to create anchor".to_string())?
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|_| "failed to cast anchor".to_string())?;
    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}
