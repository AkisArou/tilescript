use leptos::ev::MouseEvent;
use leptos::prelude::*;

use crate::app_state::AppState;
use crate::components::context_menu::{ContextMenu, ContextMenuItem, ContextMenuPosition};
use crate::editor_files::{EditorFileKey, WORKSPACE_ROOT, file_by_key};
use crate::workspace::workspace_file_tree;

use super::buffers::active_file_path;
use super::clipboard::{CopyFeedback, copy_buffer_to_clipboard};
use super::file_tree::FileTreeDirectoryView;
use super::monaco::MonacoEditorPane;

const ACTION_BUTTON_CLASS: &str = "border-terminal-border bg-terminal-bg-panel text-terminal-dim hover:text-terminal-fg border px-2 py-0.5 text-xs disabled:cursor-not-allowed disabled:opacity-40";

#[derive(Clone, Debug, PartialEq, Eq)]
struct TabContextMenuState {
    file_id: EditorFileKey,
    position: ContextMenuPosition,
}

#[component]
pub fn EditorView() -> impl IntoView {
    let app_state = expect_context::<AppState>();
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
        <section class="grid h-full min-h-0 w-full min-w-0 grid-cols-1 gap-2">
            <section class="border-terminal-border bg-terminal-bg-subtle relative grid min-h-0 overflow-hidden border lg:grid-cols-[16rem_minmax(0,1fr)]">
                <aside class="border-terminal-border bg-terminal-bg-subtle min-h-0 overflow-auto border-b lg:border-r lg:border-b-0">
                    <div class="border-terminal-border bg-terminal-bg-bar text-terminal-dim border-b px-2 py-1 text-xs">
                        {WORKSPACE_ROOT}
                    </div>
                    <div class="py-1">
                        <FileTreeDirectoryView
                            directory=Signal::derive(move || {
                                workspace_file_tree(&app_state.dynamic_layouts.get())
                            })
                            is_root=true
                            depth=0
                        />
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
                                                let file =
                                                    file_by_key(&file_id, &app_state.dynamic_layouts.get());
                                                let tab_file_id_active = file_id.clone();
                                                let tab_file_id_context = file_id.clone();
                                                let tab_file_id_select = file_id.clone();
                                                let tab_file_id_close = file_id.clone();
                                                let badge =
                                                    super::buffers::editor_file_badge(&file.language).to_string();
                                                let label = file.label.clone();

                                                view! {
                                                    <div
                                                        class=move || {
                                                            if app_state.active_file_id.get()
                                                                == Some(tab_file_id_active.clone())
                                                            {
                                                                "flex min-w-0 items-center border border-b-0 border-terminal-border-strong bg-terminal-bg-subtle text-terminal-fg-strong text-[12px]"
                                                            } else {
                                                                "flex min-w-0 items-center border border-b-0 border-terminal-border bg-terminal-bg-panel text-terminal-dim text-[12px] hover:text-terminal-fg"
                                                            }
                                                        }
                                                        on:contextmenu=move |event: MouseEvent| {
                                                            event.prevent_default();
                                                            app_state.select_editor_file(tab_file_id_context.clone());
                                                            tab_context_menu.set(Some(TabContextMenuState {
                                                                file_id: tab_file_id_context.clone(),
                                                                position: ContextMenuPosition {
                                                                    x: event.client_x(),
                                                                    y: event.client_y(),
                                                                },
                                                            }));
                                                        }
                                                    >
                                                        <button
                                                            class="flex min-w-0 items-center gap-1 px-2 py-0.5"
                                                            on:click=move |_| {
                                                                tab_context_menu.set(None);
                                                                app_state.select_editor_file(tab_file_id_select.clone());
                                                            }
                                                        >
                                                            <span
                                                                class=move || {
                                                                    if file.language == "css" {
                                                                        "text-[#7b4fc9]"
                                                                    } else if file.language == "typescript"
                                                                        || file.language == "typescriptreact"
                                                                    {
                                                                        "text-[#519aba]"
                                                                    } else {
                                                                        "text-terminal-info"
                                                                    }
                                                                }
                                                            >
                                                                {badge.clone()}
                                                            </span>
                                                            <span class="truncate">{label.clone()}</span>

                                                        </button>

                                                        <button
                                                            class="text-terminal-faint hover:text-terminal-fg px-1.5 py-1"
                                                            on:click=move |event| {
                                                                event.stop_propagation();
                                                                tab_context_menu.set(None);
                                                                app_state.close_editor_file(tab_file_id_close.clone());
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
        </section>
    }
}
