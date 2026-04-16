use std::collections::HashMap;
use std::path::PathBuf;

use tilescript_css_lsp_core::{Session, protocol};
use lsp_server::{Message, Request};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmServer {
    session: Session,
}

#[wasm_bindgen]
impl WasmServer {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { session: Session::new() }
    }

    #[wasm_bindgen(js_name = withFilesJson)]
    pub fn with_files_json(files_json: &str) -> Result<WasmServer, JsValue> {
        let files: HashMap<String, String> =
            serde_json::from_str(files_json).map_err(into_js_error)?;
        let files = files.into_iter().map(|(path, source)| (PathBuf::from(path), source)).collect();

        Ok(WasmServer { session: Session::with_in_memory_sources(files) })
    }

    #[wasm_bindgen(js_name = handleInitializeJson)]
    pub fn handle_initialize_json(&self, request_json: &str) -> Result<String, JsValue> {
        let request: Request = serde_json::from_str(request_json).map_err(into_js_error)?;
        let (response, events) = protocol::handle_initialize(request);
        serde_json::to_string(&ProtocolOutput {
            response: Some(Message::Response(response)),
            events,
        })
        .map_err(into_js_error)
    }

    #[wasm_bindgen(js_name = handleMessageJson)]
    pub fn handle_message_json(&mut self, message_json: &str) -> Result<String, JsValue> {
        let message: Message = serde_json::from_str(message_json).map_err(into_js_error)?;

        let output = match message {
            Message::Request(request) => {
                let (response, events) = protocol::handle_request(&self.session, request);
                ProtocolOutput { response: Some(Message::Response(response)), events }
            }
            Message::Notification(notification) => ProtocolOutput {
                response: None,
                events: protocol::handle_notification(&mut self.session, notification),
            },
            Message::Response(_) => ProtocolOutput { response: None, events: Vec::new() },
        };

        serde_json::to_string(&output).map_err(into_js_error)
    }
}

#[derive(serde::Serialize)]
struct ProtocolOutput {
    response: Option<Message>,
    events: Vec<protocol::ServerEvent>,
}

fn into_js_error(error: impl ToString) -> JsValue {
    JsValue::from_str(&error.to_string())
}
