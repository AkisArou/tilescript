use js_sys::{Array, Function, Object, Promise, Reflect};
use serde::Serialize;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url, window};

#[derive(Debug, Clone, Serialize)]
pub struct DirectoryDownloadItem {
    pub relative_path: String,
    pub content: String,
}

pub async fn download_directory(
    directory_name: &str,
    items: &[DirectoryDownloadItem],
) -> Result<(), String> {
    if items.is_empty() {
        return Ok(());
    }

    let window = window().ok_or_else(|| "window is unavailable".to_string())?;
    let window_value = JsValue::from(window.clone());

    if has_method(&window_value, "showDirectoryPicker") {
        match try_write_directory(&window_value, directory_name, items).await {
            Ok(()) => return Ok(()),
            Err(error) if is_abort_error(&error) => return Ok(()),
            Err(_) => {}
        }
    }

    for item in items {
        download_text_file(
            &item.content,
            &format_fallback_download_name(directory_name, &item.relative_path),
        )?;
    }

    Ok(())
}

async fn try_write_directory(
    window: &JsValue,
    directory_name: &str,
    items: &[DirectoryDownloadItem],
) -> Result<(), JsValue> {
    let parent_directory = await_js_value(call_method0(window, "showDirectoryPicker")?).await?;
    let directory = await_js_value(call_method2(
        &parent_directory,
        "getDirectoryHandle",
        &JsValue::from_str(directory_name),
        &create_true_option(),
    )?)
    .await?;

    for item in items {
        write_directory_file(&directory, &item.relative_path, &item.content).await?;
    }

    Ok(())
}

async fn write_directory_file(
    root_directory: &JsValue,
    relative_path: &str,
    content: &str,
) -> Result<(), JsValue> {
    let mut segments =
        relative_path.split('/').filter(|segment| !segment.is_empty()).collect::<Vec<_>>();
    let Some(file_name) = segments.pop() else {
        return Ok(());
    };

    let mut directory = root_directory.clone();
    for segment in segments {
        directory = await_js_value(call_method2(
            &directory,
            "getDirectoryHandle",
            &JsValue::from_str(segment),
            &create_true_option(),
        )?)
        .await?;
    }

    let file_handle = await_js_value(call_method2(
        &directory,
        "getFileHandle",
        &JsValue::from_str(file_name),
        &create_true_option(),
    )?)
    .await?;
    let writable = await_js_value(call_method0(&file_handle, "createWritable")?).await?;
    await_js_value(call_method1(&writable, "write", &JsValue::from_str(content))?).await?;
    await_js_value(call_method0(&writable, "close")?).await?;
    Ok(())
}

fn download_text_file(content: &str, file_name: &str) -> Result<(), String> {
    let window = window().ok_or_else(|| "window is unavailable".to_string())?;
    let document = window.document().ok_or_else(|| "document is unavailable".to_string())?;
    let url = Url::create_object_url_with_blob(&text_blob(content)?).map_err(js_error_message)?;
    let anchor = document
        .create_element("a")
        .map_err(js_error_message)?
        .dyn_into::<HtmlAnchorElement>()
        .map_err(|error| js_error_message(error.into()))?;
    anchor.set_href(&url);
    anchor.set_download(file_name);
    anchor.click();
    Url::revoke_object_url(&url).map_err(js_error_message)
}

fn text_blob(text: &str) -> Result<Blob, String> {
    let parts = Array::new();
    parts.push(&JsValue::from_str(text));
    let bag = BlobPropertyBag::new();
    bag.set_type("text/plain;charset=utf-8");
    Blob::new_with_str_sequence_and_options(&parts, &bag).map_err(js_error_message)
}

fn format_fallback_download_name(directory_name: &str, relative_path: &str) -> String {
    format!("{directory_name}__{}", relative_path.replace('/', "__"))
}

fn create_true_option() -> JsValue {
    let options = Object::new();
    let _ = Reflect::set(&options, &JsValue::from_str("create"), &JsValue::TRUE);
    options.into()
}

fn has_method(target: &JsValue, method: &str) -> bool {
    Reflect::get(target, &JsValue::from_str(method))
        .ok()
        .and_then(|value| value.dyn_into::<Function>().ok())
        .is_some()
}

fn call_method0(target: &JsValue, method: &str) -> Result<JsValue, JsValue> {
    let function = Reflect::get(target, &JsValue::from_str(method))?
        .dyn_into::<Function>()
        .map_err(|error| error)?;
    function.call0(target)
}

fn call_method1(target: &JsValue, method: &str, arg: &JsValue) -> Result<JsValue, JsValue> {
    let function = Reflect::get(target, &JsValue::from_str(method))?
        .dyn_into::<Function>()
        .map_err(|error| error)?;
    function.call1(target, arg)
}

fn call_method2(
    target: &JsValue,
    method: &str,
    first: &JsValue,
    second: &JsValue,
) -> Result<JsValue, JsValue> {
    let function = Reflect::get(target, &JsValue::from_str(method))?
        .dyn_into::<Function>()
        .map_err(|error| error)?;
    function.call2(target, first, second)
}

async fn await_js_value(value: JsValue) -> Result<JsValue, String> {
    JsFuture::from(value.dyn_into::<Promise>().map_err(js_error_message)?)
        .await
        .map_err(js_error_message)
}

fn is_abort_error(error: &JsValue) -> bool {
    Reflect::get(error, &JsValue::from_str("name"))
        .ok()
        .and_then(|value| value.as_string())
        .as_deref()
        == Some("AbortError")
}

fn js_error_message(error: JsValue) -> String {
    error
        .as_string()
        .or_else(|| js_sys::Error::from(error).message().as_string())
        .unwrap_or_else(|| "browser host operation failed".to_string())
}
