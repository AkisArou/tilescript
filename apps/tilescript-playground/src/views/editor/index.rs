use tilescript_core::window_id;
use leptos::ev::MouseEvent;
use leptos::prelude::*;

use crate::app_state::AppState;
use crate::components::context_menu::{ContextMenu, ContextMenuItem, ContextMenuPosition};
use crate::editor_files::{
    AuthoringLanguage, EditorFileKey, WORKSPACE_ROOT, file_by_key, file_display_badge,
    file_display_color_class, file_display_icon,
};
use crate::workspace_tree::workspace_file_tree;

use super::file_tree::FileTreeDirectoryView;
use super::monaco::MonacoEditorPane;

#[derive(Clone, Debug, PartialEq, Eq)]
struct TabContextMenuState {
    file_id: EditorFileKey,
    position: ContextMenuPosition,
}

#[component]
pub fn EditorView() -> impl IntoView {
    let app_state = expect_context::<AppState>();
    let tab_context_menu = RwSignal::new(None::<TabContextMenuState>);

    let close_tab_context_menu = Callback::new(move |_| tab_context_menu.set(None));
    let tab_context_menu_open = Signal::derive(move || tab_context_menu.get().is_some());
    let tab_context_menu_position = Signal::derive(move || {
        tab_context_menu.get().map(|state| state.position).unwrap_or_default()
    });
    let tab_context_menu_file_id =
        Signal::derive(move || tab_context_menu.get().map(|state| state.file_id));
    let focus_preview_editor = Callback::new(move |_| {
        app_state.session.update(|state| state.set_focus(window_id("win-preview-editor")));
        app_state.request_preview_reevaluation();
    });

    view! {
        <section class="grid h-full min-h-0 w-full min-w-0 grid-cols-1 gap-2">
            <section class="border-terminal-border bg-terminal-bg-subtle relative grid min-h-0 overflow-hidden border lg:grid-cols-[16rem_minmax(0,1fr)]">
                <aside class="border-terminal-border bg-terminal-bg-subtle min-h-0 overflow-auto border-b lg:border-r lg:border-b-0">
                    <div class="border-terminal-border bg-terminal-bg-bar text-terminal-dim flex items-center justify-between gap-2 border-b px-2 py-1 text-xs">
                        <span class="min-w-0 truncate">{WORKSPACE_ROOT}</span>
                        <button
                            class=move || {
                                if app_state.vim_mode_enabled.get() {
                                    "border-[var(--color-preview-accent-green)] bg-terminal-bg-hover text-[var(--color-preview-accent-green)] shrink-0 rounded-full border px-2 py-px text-[10px]"
                                } else {
                                    "border-terminal-border bg-terminal-bg-subtle text-terminal-dim hover:text-terminal-fg shrink-0 rounded-full border px-2 py-px text-[10px]"
                                }
                            }
                            on:click=move |_| {
                                app_state.set_vim_mode_enabled(!app_state.vim_mode_enabled.get_untracked());
                            }
                        >
                            "VIM mode"
                        </button>
                    </div>
                    <div class="border-terminal-border border-b px-2 py-2">
                        <div class="grid grid-cols-3 gap-2 rounded-lg border border-terminal-border bg-[linear-gradient(180deg,var(--color-editor-language-panel-top),var(--color-editor-language-panel-bottom))] p-1.5 shadow-[inset_0_1px_0_var(--color-editor-language-panel-inset)]">
                            <button
                                class=move || {
                                    if app_state.authoring_language.get() == AuthoringLanguage::JavaScript {
                                        "border-terminal-info bg-terminal-bg-hover text-terminal-fg flex min-h-20 flex-col items-center justify-center gap-2 rounded-md border px-2 py-2 text-sm font-medium shadow-[0_0_0_1px_var(--color-editor-language-glow)]"
                                    } else {
                                        "border-terminal-border bg-terminal-bg-subtle text-terminal-dim hover:text-terminal-fg flex min-h-20 flex-col items-center justify-center gap-2 rounded-md border px-2 py-2 text-sm"
                                    }
                                }
                                on:click=move |_| {
                                    app_state.set_authoring_language(AuthoringLanguage::JavaScript);
                                }
                            >
                                <span class="text-[var(--color-editor-file-typescript)] text-xl leading-none">"󰛦"</span>
                                <span class="text-center text-xs leading-tight">"JS/TS"</span>
                            </button>
                            <button
                                class=move || {
                                    if app_state.authoring_language.get() == AuthoringLanguage::Lua {
                                        "border-terminal-info bg-terminal-bg-hover text-terminal-fg flex min-h-20 flex-col items-center justify-center gap-2 rounded-md border px-2 py-2 text-sm font-medium shadow-[0_0_0_1px_var(--color-editor-language-glow)]"
                                    } else {
                                        "border-terminal-border bg-terminal-bg-subtle text-terminal-dim hover:text-terminal-fg flex min-h-20 flex-col items-center justify-center gap-2 rounded-md border px-2 py-2 text-sm"
                                    }
                                }
                                on:click=move |_| {
                                    app_state.set_authoring_language(AuthoringLanguage::Lua);
                                }
                            >
                                <span class="text-[var(--color-editor-file-lua)] text-xl leading-none">""</span>
                                <span class="text-center text-xs leading-tight">"Lua"</span>
                            </button>
                            <button
                                class=move || {
                                    if app_state.authoring_language.get() == AuthoringLanguage::Fennel {
                                        "border-terminal-info bg-terminal-bg-hover text-terminal-fg flex min-h-20 flex-col items-center justify-center gap-2 rounded-md border px-2 py-2 text-sm font-medium shadow-[0_0_0_1px_var(--color-editor-language-glow)]"
                                    } else {
                                        "border-terminal-border bg-terminal-bg-subtle text-terminal-dim hover:text-terminal-fg flex min-h-20 flex-col items-center justify-center gap-2 rounded-md border px-2 py-2 text-sm"
                                    }
                                }
                                on:click=move |_| {
                                    app_state.set_authoring_language(AuthoringLanguage::Fennel);
                                }
                            >
                                <span class="text-[var(--color-editor-file-fennel)] text-xl leading-none">"󱘎"</span>
                                <span class="text-center text-xs leading-tight">"Fennel"</span>
                            </button>
                        </div>
                    </div>
                    <div class="py-1">
                        <FileTreeDirectoryView
                            directory=Signal::derive(move || {
                                workspace_file_tree(
                                    app_state.authoring_language.get(),
                                    &app_state.dynamic_layouts.get(),
                                )
                            })
                            is_root=true
                            depth=0
                        />
                    </div>
                </aside>

                <div
                    class="flex min-h-0 min-w-0 flex-col overflow-hidden"
                    on:mousedown=move |_| focus_preview_editor.run(())
                    on:click=move |_| focus_preview_editor.run(())
                >
                    <div class="border-terminal-border bg-terminal-bg-bar flex items-center border-b text-xs">
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
                                                let file = file_by_key(&file_id, &app_state.dynamic_layouts.get_untracked());
                                                let tab_file_id_active = file_id.clone();
                                                let tab_file_id_context = file_id.clone();
                                                let tab_file_id_select = file_id.clone();
                                                let tab_file_id_close = file_id.clone();
                                                let label = file.label.clone();
                                                let icon = file_display_icon(&file.language, file.is_reference_only).to_string();
                                                let color_class = file_display_color_class(
                                                    &file.language,
                                                    file.is_reference_only,
                                                )
                                                .to_string();
                                                let badge = file_display_badge(
                                                    &file.language,
                                                    file.is_reference_only,
                                                )
                                                .to_string();

                                                view! {
                                                    <div
                                                        class=move || {
                                                            if app_state.active_file_id.get()
                                                                == Some(tab_file_id_active.clone())
                                                            {
                                                                "flex min-w-0 items-center border-r border-terminal-border-strong bg-terminal-bg-subtle text-terminal-fg-strong text-xs"
                                                            } else {
                                                                "flex min-w-0 items-center border-r border-terminal-border bg-terminal-bg-panel/75 text-terminal-dim text-xs opacity-70 hover:bg-terminal-bg-hover hover:text-terminal-fg hover:opacity-100"
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
                                                            class="flex min-w-0 items-center gap-2 px-3 py-1 font-mono"
                                                            on:click=move |_| {
                                                                tab_context_menu.set(None);
                                                                app_state.select_editor_file(tab_file_id_select.clone());
                                                            }
                                                        >
                                                            <span
                                                                class=format!("text-xs {}", color_class)
                                                            >
                                                                {icon.clone()}
                                                            </span>
                                                            <span class="truncate">{label.clone()}</span>
                                                            <Show when=move || file.is_reference_only>
                                                                <span class="rounded border border-terminal-border px-1 text-xs uppercase tracking-[0.14em] text-terminal-faint">
                                                                    {badge.clone()}
                                                                </span>
                                                            </Show>

                                                        </button>

                                                        <button
                                                            class="px-2 py-1 text-xs text-terminal-faint hover:text-terminal-fg"
                                                            on:click=move |event| {
                                                                event.stop_propagation();
                                                                tab_context_menu.set(None);
                                                                app_state.close_editor_file(tab_file_id_close.clone());
                                                            }
                                                        >
                                                            <span class="inline-block scale-75">"󰅖"</span>
                                                        </button>
                                                    </div>
                                                }
                                            })
                                            .collect_view()
                                    }}
                                </>
                            </Show>
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
                            })
                            on_close=close_tab_context_menu
                        />
                        <ContextMenuItem
                            label="Close all"
                            disabled=Signal::derive(move || app_state.open_file_ids.get().is_empty())
                            on_select=Callback::new(move |_| {
                                app_state.close_all_editor_files();
                            })
                            on_close=close_tab_context_menu
                        />
                    </ContextMenu>

                    <div class="border-terminal-border bg-terminal-bg-panel text-terminal-faint flex items-center justify-between gap-2 border-b px-2 py-1 text-xs">
                        <span class="min-w-0 flex-1 truncate">
                            {move || {
                                app_state
                                    .active_file_id
                                    .get()
                                    .map(|file_id| file_by_key(&file_id, &app_state.dynamic_layouts.get_untracked()).path)
                                    .unwrap_or_default()
                            }}
                        </span>
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
