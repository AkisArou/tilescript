use leptos::prelude::*;
use leptos_router::components::{A, Route, Router, Routes};
use leptos_router::hooks::use_location;
use leptos_router::path;

mod app_state;
mod components;
mod editor_files;
mod layout_runtime;
mod session;
mod views;
mod workspace;

use app_state::AppState;
use views::editor::EditorView;
use views::preview::PreviewView;
use views::system::SystemView;

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
        <Router>
            <AppShell />
        </Router>
    }
}

#[component]
fn AppShell() -> impl IntoView {
    let location = use_location();

    let tab_class = move |route: &'static str| {
        let current = location.pathname.get();
        let is_active = match route {
            "/" => current == "/" || current == "/preview",
            _ => route == current,
        };
        if is_active {
            "inline-flex items-center border border-b-0 px-3 py-1.5 text-sm transition duration-150 border-terminal-border-strong bg-terminal-bg text-terminal-fg-strong"
        } else {
            "inline-flex items-center border border-b-0 px-3 py-1.5 text-sm transition duration-150 border-terminal-border bg-terminal-bg-bar text-terminal-dim opacity-70 hover:text-terminal-fg hover:opacity-100"
        }
    };

    view! {
        <main class="flex h-screen flex-col overflow-hidden bg-terminal-bg text-terminal-fg">
            <div class="min-h-0 flex-1 overflow-hidden">
                <Routes fallback=NotFoundRoute>
                    <Route path=path!("/") view=PreviewRoute />
                    <Route path=path!("/preview") view=PreviewRoute />
                    <Route path=path!("/editor") view=EditorRoute />
                    <Route path=path!("/system") view=SystemRoute />
                </Routes>
            </div>

            <div class="border-terminal-border bg-terminal-bg-subtle px-2 pb-1 border-t">
                <nav class="flex flex-wrap gap-1 overflow-x-auto">
                    <A href="/" attr:class=move || tab_class("/")>
                    "1:preview"
                    </A>
                    <A href="/editor" attr:class=move || tab_class("/editor")>
                    "2:editor"
                    </A>
                    <A href="/system" attr:class=move || tab_class("/system")>
                    "3:system"
                    </A>
                </nav>
            </div>
        </main>
    }
}

#[component]
fn PreviewRoute() -> impl IntoView {
    view! { <PreviewView /> }
}

#[component]
fn EditorRoute() -> impl IntoView {
    view! { <EditorView /> }
}

#[component]
fn SystemRoute() -> impl IntoView {
    view! { <SystemView /> }
}

#[component]
fn NotFoundRoute() -> impl IntoView {
    view! {
        <section class="empty-state">
            <div class="eyebrow">"route://missing"</div>
            <div class="title">"Not found"</div>
        </section>
    }
}

fn install_config_loader(app_state: AppState) {
    Effect::new(move |_| {
        let buffers = app_state.editor_buffers.get();
        let request_key = format!("{buffers:?}");

        if app_state.latest_config_request_key.get_untracked() == request_key {
            return;
        }

        app_state.latest_config_request_key.set(request_key.clone());

        wasm_bindgen_futures::spawn_local(async move {
            match layout_runtime::load_config_from_buffers(&buffers).await {
                Ok(config) => {
                    if app_state.latest_config_request_key.get_untracked() != request_key {
                        return;
                    }
                    app_state.apply_loaded_config(config);
                }
                Err(error) => {
                    if app_state.latest_config_request_key.get_untracked() != request_key {
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
        let buffers = app_state.editor_buffers.get();
        let model = app_state.session.get_untracked().model.clone();

        wasm_bindgen_futures::spawn_local(async move {
            match layout_runtime::evaluate_preview_from_buffers(&buffers, &model).await {
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
