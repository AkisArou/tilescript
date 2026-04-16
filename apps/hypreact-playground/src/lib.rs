use leptos::prelude::*;

mod app_state;
mod components;
mod editor_files;
mod editor_host;
mod layout_runtime;
mod session;
mod views;
mod workspace;

use app_state::AppState;
use views::preview::PreviewView;

#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let app_state = AppState::new();
    provide_context(app_state);

    install_config_loader(app_state);
    install_preview_renderer(app_state);

    view! {
        <AppShell />
    }
}

#[component]
fn AppShell() -> impl IntoView {
    view! {
        <main class="flex h-screen flex-col overflow-hidden bg-terminal-bg text-terminal-fg">
            <PreviewView />
        </main>
    }
}

fn install_config_loader(app_state: AppState) {
    Effect::new(move |_| {
        let request_id = app_state.latest_config_request_id.get();
        let buffers = app_state.editor_buffers.get();
        let dynamic_layouts = app_state.dynamic_layouts.get();

        wasm_bindgen_futures::spawn_local(async move {
            match layout_runtime::load_config_from_buffers(
                app_state.authoring_language.get_untracked(),
                &buffers,
                &dynamic_layouts,
            )
            .await
            {
                Ok(config) => {
                    if app_state.latest_config_request_id.get_untracked() != request_id {
                        return;
                    }
                    app_state.apply_loaded_config(config);
                }
                Err(error) => {
                    if app_state.latest_config_request_id.get_untracked() != request_id {
                        return;
                    }
                    app_state.apply_config_error(error);
                }
            }
        });
    });
}

fn install_preview_renderer(app_state: AppState) {
    Effect::new(move |_| {
        let request_id = app_state.preview_eval_request.get();
        let Some(config) = app_state.loaded_config.get() else {
            return;
        };
        let buffers = app_state.editor_buffers.get();
        let dynamic_layouts = app_state.dynamic_layouts.get();
        let session = app_state.session.get_untracked();
        let model = session.model.clone();
        let manual_layouts = session.manual_layout_by_workspace.clone();
        let preserve_last_scene_on_error = session.scene.is_some();

        wasm_bindgen_futures::spawn_local(async move {
            match layout_runtime::evaluate_preview_from_buffers(
                app_state.authoring_language.get_untracked(),
                &buffers,
                &dynamic_layouts,
                &config,
                &model,
                &manual_layouts,
                preserve_last_scene_on_error,
            )
            .await
            {
                Ok(preview) => {
                    if app_state.preview_eval_request.get_untracked() != request_id {
                        return;
                    }
                    app_state.apply_loaded_preview(preview);
                }
                Err(error) => {
                    if app_state.preview_eval_request.get_untracked() != request_id {
                        return;
                    }
                    app_state.apply_preview_failure(error);
                }
            }
        });
    });
}
