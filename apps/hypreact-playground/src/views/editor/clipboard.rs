use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyFeedback {
    Idle,
    Copied,
    Failed,
}

impl CopyFeedback {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "copy",
            Self::Copied => "copied",
            Self::Failed => "copy failed",
        }
    }
}

pub fn copy_buffer_to_clipboard(contents: String, feedback: RwSignal<CopyFeedback>) {
    feedback.set(CopyFeedback::Idle);

    spawn_local(async move {
        let Some(window) = web_sys::window() else {
            feedback.set(CopyFeedback::Failed);
            return;
        };
        let clipboard = window.navigator().clipboard();

        let result = wasm_bindgen_futures::JsFuture::from(clipboard.write_text(&contents)).await;
        feedback.set(match result {
            Ok(_) => CopyFeedback::Copied,
            Err(_) => CopyFeedback::Failed,
        });
    });
}
