use leptos::ev::MouseEvent;
use leptos::prelude::*;

use crate::app_state::AppState;
use crate::components::context_menu::{ContextMenu, ContextMenuItem, ContextMenuPosition};
use crate::editor_files::{EditorFileId, WORKSPACE_ROOT, file_by_id};
use crate::workspace::workspace_file_tree;

use super::buffers::{
    active_file_badge, active_file_is_dirty, active_file_language, active_file_path, is_file_dirty,
};
use super::clipboard::{CopyFeedback, copy_buffer_to_clipboard};
use super::file_tree::FileTreeDirectoryView;
use super::monaco::MonacoEditorPane;

const PANEL_CLASS: &str =
    "border-terminal-border bg-terminal-bg-subtle flex min-h-0 flex-col overflow-hidden border";
const BAR_CLASS: &str = "border-terminal-border bg-terminal-bg-bar text-terminal-dim flex items-center justify-between border-b px-2 py-1 text-xs";
const ACTION_BUTTON_CLASS: &str = "border-terminal-border bg-terminal-bg-panel text-terminal-dim hover:text-terminal-fg border px-2 py-0.5 text-xs disabled:cursor-not-allowed disabled:opacity-40";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TabContextMenuState {
    file_id: EditorFileId,
    position: ContextMenuPosition,
}

#[component]
pub fn EditorView() -> impl IntoView {
    let app_state = expect_context::<AppState>();
    let file_tree = workspace_file_tree();
    let copy_feedback = RwSignal::new(CopyFeedback::Idle);
    let tab_context_menu = RwSignal::new(None::<TabContextMenuState>);

    let close_tab_context_menu = Callback::new(move |_| tab_context_menu.set(None));
    let tab_context_menu_open = Signal::derive(move || tab_context_menu.get().is_some());
    let tab_context_menu_position = Signal::derive(move || {
        tab_context_menu.get().map(|state| state.position).unwrap_or_default()
    });
    let tab_context_menu_file_id =
        Signal::derive(move || tab_context_menu.get().map(|state| state.file_id));

    view! {
        <section class="grid h-full min-h-0 w-full min-w-0 grid-cols-1 gap-2 xl:grid-cols-[minmax(0,1fr)_20rem]">
            <section class="border-terminal-border bg-terminal-bg-subtle relative grid min-h-0 overflow-hidden border lg:grid-cols-[16rem_minmax(0,1fr)]">
                <aside class="border-terminal-border bg-terminal-bg-subtle min-h-0 overflow-auto border-b lg:border-r lg:border-b-0">
                    <div class="border-terminal-border bg-terminal-bg-bar text-terminal-dim border-b px-2 py-1 text-xs">
                        {WORKSPACE_ROOT}
                    </div>
                    <div class="py-1">
                        <FileTreeDirectoryView directory=file_tree is_root=true depth=0/>
                    </div>
                </aside>

                <div class="flex min-h-0 min-w-0 flex-col overflow-hidden">
                    <div class="border-terminal-border bg-terminal-bg-bar flex items-center gap-px border-b px-1 pt-1 text-xs">
                        <div class="flex min-w-0 flex-1 gap-px overflow-x-auto">
                            <Show
                                when=move || !app_state.open_file_ids.get().is_empty()
                                fallback=move || {
                                    view! {
                                        <span class="text-terminal-faint px-2 py-1 text-sm">
                                            "no files open"
                                        </span>
                                    }
                                }
                            >
                                <>
                                    {move || {
                                        app_state
                                            .open_file_ids
                                            .get()
                                            .into_iter()
                                            .map(|file_id| {
                                                let file = file_by_id(file_id);
                                                let badge = super::buffers::editor_file_badge(file.language).to_string();
                                                let label = file.label.to_string();

                                                view! {
                                                    <div
                                                        class=move || {
                                                            if app_state.active_file_id.get() == Some(file_id) {
                                                                "flex min-w-0 items-center border border-b-0 border-terminal-border-strong bg-terminal-bg-subtle text-terminal-fg-strong text-sm"
                                                            } else {
                                                                "flex min-w-0 items-center border border-b-0 border-terminal-border bg-terminal-bg-panel text-terminal-dim text-sm hover:text-terminal-fg"
                                                            }
                                                        }
                                                        on:contextmenu=move |event: MouseEvent| {
                                                            event.prevent_default();
                                                            app_state.select_editor_file(file_id);
                                                            tab_context_menu.set(Some(TabContextMenuState {
                                                                file_id,
                                                                position: ContextMenuPosition {
                                                                    x: event.client_x(),
                                                                    y: event.client_y(),
                                                                },
                                                            }));
                                                        }
                                                    >
                                                        <button
                                                            class="flex min-w-0 items-center gap-1 px-2 py-1"
                                                            on:click=move |_| {
                                                                tab_context_menu.set(None);
                                                                app_state.select_editor_file(file_id);
                                                            }
                                                        >
                                                            <span
                                                                class=move || {
                                                                    if file.language == "css" {
                                                                        "text-[#7b4fc9]"
                                                                    } else {
                                                                        "text-terminal-info"
                                                                    }
                                                                }
                                                            >
                                                                {badge.clone()}
                                                            </span>
                                                            <span class="truncate">{label.clone()}</span>

                                                            <Show when=move || is_file_dirty(&app_state.editor_buffers.get(), file_id)>
                                                                <span class="text-terminal-warn">"+"</span>
                                                            </Show>
                                                        </button>

                                                        <button
                                                            class="text-terminal-faint hover:text-terminal-fg px-1.5 py-1"
                                                            on:click=move |event| {
                                                                event.stop_propagation();
                                                                tab_context_menu.set(None);
                                                                app_state.close_editor_file(file_id);
                                                            }
                                                        >
                                                            "x"
                                                        </button>
                                                    </div>
                                                }
                                            })
                                            .collect_view()
                                    }}
                                </>
                            </Show>
                        </div>

                        <div class="mr-1 ml-auto flex items-center gap-1">
                            <button
                                class=ACTION_BUTTON_CLASS
                                disabled=move || app_state.active_file_id.get().is_none()
                                on:click=move |_| {
                                    let Some(_) = app_state.active_file_id.get_untracked() else {
                                        return;
                                    };

                                    let contents = super::buffers::active_buffer_text(app_state);
                                    copy_buffer_to_clipboard(contents, copy_feedback);
                                }
                            >
                                {move || copy_feedback.get().label()}
                            </button>
                        </div>
                    </div>

                    <ContextMenu
                        open=tab_context_menu_open
                        position=tab_context_menu_position
                        on_close=close_tab_context_menu
                    >
                        <ContextMenuItem
                            label="Close"
                            disabled=Signal::derive(move || tab_context_menu_file_id.get().is_none())
                            on_select=Callback::new(move |_| {
                                let Some(file_id) = tab_context_menu_file_id.get_untracked() else {
                                    return;
                                };

                                app_state.close_editor_file(file_id);
                            })
                            on_close=close_tab_context_menu
                        />
                        <ContextMenuItem
                            label="Close others"
                            disabled=Signal::derive(move || {
                                tab_context_menu_file_id.get().is_none()
                                    || app_state.open_file_ids.get().len() <= 1
                            })
                            on_select=Callback::new(move |_| {
                                let Some(file_id) = tab_context_menu_file_id.get_untracked() else {
                                    return;
                                };

                                app_state.close_other_editor_files(file_id);
                                copy_feedback.set(CopyFeedback::Idle);
                            })
                            on_close=close_tab_context_menu
                        />
                        <ContextMenuItem
                            label="Close all"
                            disabled=Signal::derive(move || app_state.open_file_ids.get().is_empty())
                            on_select=Callback::new(move |_| {
                                app_state.close_all_editor_files();
                                copy_feedback.set(CopyFeedback::Idle);
                            })
                            on_close=close_tab_context_menu
                        />
                    </ContextMenu>

                    <div class="border-terminal-border bg-terminal-bg-panel text-terminal-faint flex items-center justify-between gap-2 border-b px-2 py-1 text-xs">
                        <span class="min-w-0 flex-1 truncate">{move || active_file_path(app_state)}</span>
                        <div class="flex shrink-0 items-center gap-2">
                            <span>{move || active_file_badge(app_state)}</span>
                            <span>{move || active_file_language(app_state)}</span>
                            <Show when=move || active_file_is_dirty(app_state)>
                                <span class="text-terminal-warn">"modified"</span>
                            </Show>
                        </div>
                    </div>

                    <div class="min-h-0 flex-1 overflow-hidden">
                        <Show
                            when=move || app_state.active_file_id.get().is_some()
                            fallback=move || {
                                view! {
                                    <div class="text-terminal-faint grid h-full place-items-center text-sm">
                                        "no file open"
                                    </div>
                                }
                            }
                        >
                            <MonacoEditorPane/>
                        </Show>
                    </div>
                </div>
            </section>

            <section class="grid min-h-0 gap-2 xl:grid-rows-[auto_auto_1fr]">
                <div class=PANEL_CLASS>
                    <div class=BAR_CLASS>"editor://state"</div>
                    <div class="text-terminal-muted grid gap-1 p-2 text-sm">
                        <div class="border-terminal-border bg-terminal-bg-panel flex justify-between border px-2 py-1">
                            <span>"workspace"</span>
                            <span class="text-terminal-fg-strong">{WORKSPACE_ROOT}</span>
                        </div>
                        <div class="border-terminal-border bg-terminal-bg-panel flex justify-between border px-2 py-1">
                            <span>"language"</span>
                            <span class="text-terminal-fg-strong">{move || active_file_language(app_state)}</span>
                        </div>
                        <div class="border-terminal-border bg-terminal-bg-panel flex justify-between border px-2 py-1">
                            <span>"layout"</span>
                            <span class="text-terminal-fg-strong">{move || app_state.session.get().active_layout_name()}</span>
                        </div>
                    </div>
                </div>

                <div class=PANEL_CLASS>
                    <div class=BAR_CLASS>"editor://bindings"</div>
                    <div class="text-terminal-muted min-h-0 overflow-auto p-2 text-sm">
                        <div class="grid gap-1">
                            <div class="border-terminal-border bg-terminal-bg-panel grid gap-1 border px-2 py-1">
                                <div class="text-terminal-fg-strong">"Ctrl/Meta + S"</div>
                                <div class="text-terminal-dim">"saved implicitly to the in-memory source bundle"</div>
                            </div>
                            <div class="border-terminal-border bg-terminal-bg-panel grid gap-1 border px-2 py-1">
                                <div class="text-terminal-fg-strong">"Preview keys"</div>
                                <div class="text-terminal-dim">"j/k focus, shift+j/k workspace, [/] layout, f floating, enter fullscreen"</div>
                            </div>
                        </div>
                    </div>
                </div>

                <div class=PANEL_CLASS>
                    <div class=BAR_CLASS>"editor://runtime"</div>
                    <div class="text-terminal-muted grid gap-1 p-2 text-sm">
                        <div class="border-terminal-border bg-terminal-bg-panel border px-2 py-1">
                            <div class="text-terminal-info text-xs">"applied live"</div>
                            <div class="mt-1">"config.ts, root css, layout css, and active layout TSX stay in sync with the browser runtime preview source bundle"</div>
                        </div>
                        <div class="border-terminal-border bg-terminal-bg-panel border px-2 py-1">
                            <div class="text-terminal-info text-xs">"editor host"</div>
                            <div class="mt-1">"Monaco models are hydrated from the local template source bundle in this crate and receive @hypreact/sdk type libraries"</div>
                        </div>
                    </div>
                </div>
            </section>
        </section>
    }
}
