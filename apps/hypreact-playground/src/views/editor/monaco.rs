use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use leptos::html;
use leptos::prelude::*;
use leptos::serde_json;
use wasm_bindgen_futures::spawn_local;

use crate::app_state::AppState;
use crate::editor_files::{
    AuthoringLanguage, EditorFileKey, WORKSPACE_FS_ROOT, file_by_key, file_key_by_model_path,
    file_layout_language, iter_dynamic_files, model_path, static_files,
};

use super::buffers::active_buffer_text;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MonacoModel {
    path: String,
    language: String,
    value: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MonacoExtraLib {
    file_path: String,
    content: &'static str,
}

fn monaco_models(
    language: AuthoringLanguage,
    buffers: &BTreeMap<EditorFileKey, String>,
    dynamic_layouts: &[crate::editor_files::DynamicLayoutFileSet],
) -> Vec<MonacoModel> {
    static_files(language)
        .iter()
        .map(|file| MonacoModel {
            path: model_path(&file.key(), dynamic_layouts).to_string(),
            language: monaco_language(file.language()).to_string(),
            value: buffers
                .get(&file.key())
                .cloned()
                .unwrap_or_else(|| file_by_key(&file.key(), dynamic_layouts).initial_content),
        })
        .chain(
            iter_dynamic_files(dynamic_layouts)
                .filter(move |file| {
                    file_layout_language(&file.key, dynamic_layouts) == Some(language)
                })
                .map(|file| MonacoModel {
                    path: model_path(&file.key, dynamic_layouts).to_string(),
                    language: monaco_language(&file.language).to_string(),
                    value: buffers
                        .get(&file.key)
                        .cloned()
                        .unwrap_or_else(|| file.initial_content.clone()),
                }),
        )
        .collect()
}

fn monaco_language(language: &str) -> &'static str {
    match language {
        "css" => "css",
        "lua" => "lua",
        "fennel" => "clojure",
        _ => "typescript",
    }
}

fn sdk_type_libs() -> Vec<MonacoExtraLib> {
    let workspace_node_modules = format!("file://{WORKSPACE_FS_ROOT}/node_modules/@hypreact/sdk");

    vec![
        MonacoExtraLib {
            file_path: format!("{workspace_node_modules}/index.d.ts"),
            content: concat!(
                "export * from \"./api\";\n",
                "export * from \"./commands\";\n",
                "export * from \"./config\";\n",
                "export * from \"./css\";\n",
                "export * from \"./jsx-dev-runtime\";\n",
                "export * from \"./jsx-runtime\";\n",
                "export * from \"./layout\";\n",
            ),
        },
        MonacoExtraLib {
            file_path: format!("{workspace_node_modules}/api.d.ts"),
            content: include_str!("../../../../../packages/sdk/js/src/api.d.ts"),
        },
        MonacoExtraLib {
            file_path: format!("{workspace_node_modules}/commands.d.ts"),
            content: include_str!("../../../../../packages/sdk/js/src/commands.d.ts"),
        },
        MonacoExtraLib {
            file_path: format!("{workspace_node_modules}/config.d.ts"),
            content: include_str!("../../../../../packages/sdk/js/src/config.d.ts"),
        },
        MonacoExtraLib {
            file_path: format!("{workspace_node_modules}/css.d.ts"),
            content: include_str!("../../../../../packages/sdk/js/src/css.d.ts"),
        },
        MonacoExtraLib {
            file_path: format!("{workspace_node_modules}/jsx-dev-runtime.d.ts"),
            content: include_str!("../../../../../packages/sdk/js/src/jsx-dev-runtime.d.ts"),
        },
        MonacoExtraLib {
            file_path: format!("{workspace_node_modules}/jsx-runtime.d.ts"),
            content: include_str!("../../../../../packages/sdk/js/src/jsx-runtime.d.ts"),
        },
        MonacoExtraLib {
            file_path: format!("{workspace_node_modules}/layout.d.ts"),
            content: include_str!("../../../../../packages/sdk/js/src/layout.d.ts"),
        },
    ]
}

mod wasm {
    use js_sys::{Function, Promise};
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::JsFuture;

    use super::{MonacoModel, sdk_type_libs};

    #[wasm_bindgen(module = "/src/monaco_host_bundle.js")]
    extern "C" {
        #[wasm_bindgen(catch, js_name = createMonacoEditor)]
        fn create_monaco_editor_js(
            host: &web_sys::HtmlElement,
            active_path: &str,
            models: JsValue,
            extra_libs: JsValue,
            on_change: &Function,
            on_open: &Function,
        ) -> Result<Promise, JsValue>;

        #[wasm_bindgen(catch, js_name = updateMonacoEditor)]
        fn update_monaco_editor_js(
            handle: &JsValue,
            active_path: &str,
            models: JsValue,
        ) -> Result<(), JsValue>;

        #[wasm_bindgen(catch, js_name = revealMonacoPosition)]
        fn reveal_monaco_position_js(
            handle: &JsValue,
            line_number: u32,
            column: u32,
        ) -> Result<(), JsValue>;

        #[wasm_bindgen(catch, js_name = monacoMarkerCount)]
        fn monaco_marker_count_js(handle: &JsValue) -> Result<u32, JsValue>;

        #[wasm_bindgen(catch, js_name = disposeMonacoEditor)]
        fn dispose_monaco_editor_js(handle: &JsValue) -> Result<(), JsValue>;
    }

    pub struct MonacoEditorHandle {
        handle: JsValue,
        _change_callback: Closure<dyn Fn(String, String)>,
        _open_callback: Closure<dyn Fn(String)>,
    }

    impl MonacoEditorHandle {
        pub(super) fn sync(
            &self,
            active_path: Option<&str>,
            models: &[MonacoModel],
        ) -> Result<(), String> {
            let models = serde_wasm_bindgen::to_value(models).map_err(|error| error.to_string())?;
            update_monaco_editor_js(&self.handle, active_path.unwrap_or_default(), models)
                .map_err(js_error_message)
        }

        pub(super) fn reveal_position(&self, line_number: u32, column: u32) -> Result<(), String> {
            reveal_monaco_position_js(&self.handle, line_number, column).map_err(js_error_message)
        }

        #[allow(dead_code)]
        pub(super) fn marker_count(&self) -> Result<u32, String> {
            monaco_marker_count_js(&self.handle).map_err(js_error_message)
        }
    }

    impl Drop for MonacoEditorHandle {
        fn drop(&mut self) {
            let _ = dispose_monaco_editor_js(&self.handle);
        }
    }

    pub async fn mount_monaco_editor(
        host: web_sys::HtmlElement,
        active_path: Option<&str>,
        models: &[MonacoModel],
        on_change: impl Fn(String, String) + 'static,
        on_open: impl Fn(String) + 'static,
    ) -> Result<MonacoEditorHandle, String> {
        let models = serde_wasm_bindgen::to_value(models).map_err(|error| error.to_string())?;
        let extra_libs =
            serde_wasm_bindgen::to_value(&sdk_type_libs()).map_err(|error| error.to_string())?;
        let change_callback = Closure::wrap(Box::new(on_change) as Box<dyn Fn(String, String)>);
        let open_callback = Closure::wrap(Box::new(on_open) as Box<dyn Fn(String)>);
        let promise = create_monaco_editor_js(
            &host,
            active_path.unwrap_or_default(),
            models,
            extra_libs,
            change_callback.as_ref().unchecked_ref(),
            open_callback.as_ref().unchecked_ref(),
        )
        .map_err(js_error_message)?;
        let handle = JsFuture::from(promise).await.map_err(js_error_message)?;

        Ok(MonacoEditorHandle {
            handle,
            _change_callback: change_callback,
            _open_callback: open_callback,
        })
    }

    fn js_error_message(error: JsValue) -> String {
        error.as_string().unwrap_or_else(|| "monaco editor bridge failed".to_string())
    }
}

pub use wasm::MonacoEditorHandle;
use wasm::mount_monaco_editor;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct MonacoOpenPayload {
    path: String,
    #[serde(default)]
    selection_or_position: Option<MonacoSelection>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct MonacoSelection {
    start_line_number: u32,
    start_column: u32,
}

#[derive(Debug, Clone)]
struct PendingNavigation {
    path: String,
    selection: MonacoSelection,
}

#[component]
pub fn MonacoEditorPane() -> impl IntoView {
    let app_state = expect_context::<AppState>();
    let editor_mount = NodeRef::<html::Div>::new();
    let monaco_error = RwSignal::new(None::<String>);
    let monaco_loading = RwSignal::new(false);
    let _monaco_handle = Rc::new(RefCell::new(None::<MonacoEditorHandle>));
    let pending_navigation = RwSignal::new(None::<PendingNavigation>);

    {
        let editor_mount = editor_mount.clone();
        let monaco_handle = Rc::clone(&_monaco_handle);
        let app_state_for_mount = app_state;
        Effect::new(move |_| {
            let Some(host) = editor_mount.get() else {
                return;
            };

            let active_file_id = app_state_for_mount.active_file_id.get();

            if monaco_handle.borrow().is_some() || monaco_loading.get() {
                return;
            }

            let dynamic_layouts = app_state_for_mount.dynamic_layouts.get_untracked();
            let authoring_language = app_state_for_mount.authoring_language.get_untracked();
            let models = monaco_models(
                authoring_language,
                &app_state_for_mount.editor_buffers.get_untracked(),
                &dynamic_layouts,
            );
            let active_path = active_file_id
                .as_ref()
                .map(|file_id| model_path(file_id, &dynamic_layouts).to_string());
            let monaco_handle = Rc::clone(&monaco_handle);
            let callback_state = app_state_for_mount;
            let pending_navigation = pending_navigation;

            monaco_loading.set(true);
            monaco_error.set(None);

            spawn_local(async move {
                let result = mount_monaco_editor(
                    host.into(),
                    active_path.as_deref(),
                    &models,
                    move |path, value| {
                        let dynamic_layouts = callback_state.dynamic_layouts.get_untracked();
                        let Some(file_id) = file_key_by_model_path(
                            &path,
                            callback_state.authoring_language.get_untracked(),
                            &dynamic_layouts,
                        ) else {
                            return;
                        };

                        let file_meta = file_by_key(&file_id, &dynamic_layouts);
                        let current_value = callback_state
                            .editor_buffers
                            .get_untracked()
                            .get(&file_id)
                            .cloned()
                            .unwrap_or(file_meta.initial_content);

                        if current_value != value {
                            callback_state.update_buffer(file_id, value);
                        }
                    },
                    move |path| {
                        let payload = serde_json::from_str::<MonacoOpenPayload>(&path).ok();
                        let target_path =
                            payload.as_ref().map(|payload| payload.path.clone()).unwrap_or(path);
                        let selection = payload.and_then(|payload| payload.selection_or_position);

                        let dynamic_layouts = callback_state.dynamic_layouts.get_untracked();
                        if let Some(file_id) = file_key_by_model_path(
                            &target_path,
                            callback_state.authoring_language.get_untracked(),
                            &dynamic_layouts,
                        ) {
                            if let Some(selection) = selection {
                                pending_navigation.set(Some(PendingNavigation {
                                    path: target_path.clone(),
                                    selection,
                                }));
                            }
                            callback_state.select_editor_file(file_id);
                        }
                    },
                )
                .await;

                match result {
                    Ok(handle) => {
                        *monaco_handle.borrow_mut() = Some(handle);
                        monaco_error.set(None);
                    }
                    Err(error) => {
                        monaco_error.set(Some(error));
                    }
                }

                monaco_loading.set(false);
            });
        });

        let monaco_handle = Rc::clone(&_monaco_handle);
        Effect::new(move |_| {
            let authoring_language = app_state.authoring_language.get();
            let dynamic_layouts = app_state.dynamic_layouts.get();
            let active_path = app_state
                .active_file_id
                .get()
                .as_ref()
                .map(|file_id| model_path(file_id, &dynamic_layouts).to_string());
            let models = monaco_models(
                authoring_language,
                &app_state.editor_buffers.get(),
                &dynamic_layouts,
            );

            if let Some(handle) = monaco_handle.borrow().as_ref() {
                if let Err(error) = handle.sync(active_path.as_deref(), &models) {
                    monaco_error.set(Some(error));
                }
            }
        });

        let monaco_handle = Rc::clone(&_monaco_handle);
        Effect::new(move |_| {
            let Some(navigation) = pending_navigation.get() else {
                return;
            };
            let Some(active_file_id) = app_state.active_file_id.get() else {
                return;
            };
            let dynamic_layouts = app_state.dynamic_layouts.get();
            if model_path(&active_file_id, &dynamic_layouts) != navigation.path {
                return;
            }

            let monaco_handle = monaco_handle.borrow();
            let Some(handle) = monaco_handle.as_ref() else {
                return;
            };

            if let Err(error) = handle.reveal_position(
                navigation.selection.start_line_number,
                navigation.selection.start_column,
            ) {
                monaco_error.set(Some(error));
            }
            pending_navigation.set(None);
        });
    }

    if cfg!(target_arch = "wasm32") {
        view! {
            <div class="relative flex h-full min-h-0 flex-1 bg-[linear-gradient(180deg,rgba(255,255,255,0.015),transparent)]">
                <Show
                    when=move || monaco_error.get().is_none()
                    fallback=move || {
                        view! {
                            <textarea
                                class="flex h-full w-full min-h-0 px-4 py-4 font-mono text-[0.94rem] leading-[1.55] text-white bg-transparent outline-none resize-none"
                                prop:value=move || active_buffer_text(app_state)
                                prop:spellcheck=false
                                on:input=move |event| {
                                    let Some(file_id) = app_state.active_file_id.get_untracked() else {
                                        return;
                                    };
                                    app_state.update_buffer(file_id, event_target_value(&event));
                                }
                            />
                        }
                    }
                >
                    <div
                        node_ref=editor_mount
                        class="h-full w-full min-h-0 text-[14px] leading-[20px]"
                    />
                    <Show when=move || monaco_loading.get()>
                        <div class="grid absolute inset-0 place-items-center uppercase pointer-events-none bg-[linear-gradient(180deg,rgba(4,8,12,0.82),rgba(9,17,26,0.78))] text-[0.72rem] tracking-[0.18em] text-slate-400">
                            "loading monaco..."
                        </div>
                    </Show>
                </Show>
            </div>
        }
            .into_any()
    } else {
        view! {
            <textarea
                class="flex-1 h-full min-h-0 px-4 py-4 font-mono text-[0.94rem] leading-[1.55] text-white outline-none resize-none bg-[linear-gradient(180deg,rgba(255,255,255,0.015),transparent)]"
                prop:value=move || active_buffer_text(app_state)
                prop:spellcheck=false
                on:input=move |event| {
                    let Some(file_id) = app_state.active_file_id.get_untracked() else {
                        return;
                    };
                    app_state.update_buffer(file_id, event_target_value(&event));
                }
            />
        }
            .into_any()
    }
}
