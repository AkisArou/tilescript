use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::app_state::AppState;
use crate::components::tooltip::Tooltip;
use crate::editor_files::{file_by_key, make_dynamic_layout};
use crate::editor_host::download_directory;
use crate::workspace::{EditorFileTreeDirectory, EditorFileTreeNode};

use super::download::{collect_directory_download_items, download_directory_title};

fn branch_indent(depth: usize, is_root: bool) -> String {
    if is_root {
        "padding-left: 6px".to_string()
    } else {
        format!("padding-left: {}px", depth * 14 + 4)
    }
}

fn branch_guide(depth: usize, elbow: bool) -> String {
    if depth == 0 {
        return String::new();
    }

    let offset = depth * 14 - 6;
    if elbow {
        format!(
            "background-image: linear-gradient(rgba(120,120,120,0.26), rgba(120,120,120,0.26)), linear-gradient(rgba(120,120,120,0.26), rgba(120,120,120,0.26)); background-size: 1px 100%, 9px 1px; background-position: {offset}px 0, {offset}px 50%; background-repeat: no-repeat;"
        )
    } else {
        format!(
            "background-image: linear-gradient(rgba(120,120,120,0.26), rgba(120,120,120,0.26)); background-size: 1px 100%; background-position: {offset}px 0; background-repeat: no-repeat;"
        )
    }
}

fn tree_row_style(depth: usize, elbow: bool, is_root: bool) -> String {
    format!("{}; {}", branch_indent(depth, is_root), branch_guide(depth, elbow))
}

fn is_directory_open(app_state: AppState, path: &str, default_open: bool, is_root: bool) -> bool {
    if is_root {
        true
    } else {
        app_state.directory_open_state.get().get(path).copied().unwrap_or(default_open)
    }
}

#[component]
pub fn FileTreeDirectoryView(
    directory: Signal<EditorFileTreeDirectory>,
    #[prop(optional)] is_root: bool,
    #[prop(optional)] depth: usize,
) -> impl IntoView {
    let app_state = expect_context::<AppState>();
    let default_open = Signal::derive(move || directory.get().default_open);
    let directory_path = Signal::derive(move || directory.get().path.to_string());

    view! {
        <div class="grid">
            {if !is_root {
                    view! {
                        <div class="group/layout-subtree flex items-center gap-1">
                            <button
                                class="text-terminal-dim flex flex-1 items-center gap-1 py-0.5 text-left text-[13px] leading-[1.15rem] hover:text-terminal-fg"
                                style=tree_row_style(depth, true, is_root)
                                on:click=move |_| {
                                    let current = directory.get();
                                    app_state.toggle_directory(
                                        current.path.to_string(),
                                        current.default_open,
                                    )
                                }
                            >
                                <span class="w-2 shrink-0 text-terminal-faint">"╰"</span>
                                <span class="w-2 text-terminal-faint">
                                    {move || {
                                        let current = directory.get();
                                        if app_state
                                            .directory_open_state
                                            .get()
                                            .get(current.path)
                                            .copied()
                                            .unwrap_or(current.default_open)
                                        {
                                            "v"
                                        } else {
                                            ">"
                                        }
                                    }}
                                </span>
                                <span class="text-terminal-info shrink-0 text-[12px]">""</span>
                                <span class="min-w-0 flex-1 truncate text-terminal-fg">
                                    {move || directory.get().name.to_string()}
                                </span>
                            </button>

                            <Show when=move || directory.get().can_create_layout>
                                <button
                                    class="px-1 text-[11px] text-terminal-faint hover:text-terminal-fg"
                                    on:click=move |event| {
                                        event.stop_propagation();
                                        let Some(window) = web_sys::window() else {
                                            return;
                                        };
                                        let Ok(result) = window.prompt_with_message("Layout name") else {
                                            return;
                                        };
                                        let Some(name) = result else {
                                            return;
                                        };
                                        let layout = make_dynamic_layout(&name);
                                        app_state.create_layout(layout);
                                    }
                                >
                                    "+ Layout"
                                </button>
                            </Show>

                            <Show when=move || directory.get().download_root_path.is_some()>
                                <Tooltip
                                    content=Signal::derive({ move || download_directory_title(&directory.get()) })
                                    class="ml-auto"
                                >
                                    <button
                                        class="py-0 px-1 text-terminal-faint text-[9px] uppercase tracking-[0.16em] hover:text-terminal-fg"
                                        aria-label=move || download_directory_title(&directory.get())
                                        on:click=move |event| {
                                            event.stop_propagation();
                                            let directory_for_download = directory.get();
                                            let directory_label = directory_for_download.name;
                                            let items = collect_directory_download_items(
                                                &directory_for_download,
                                                &app_state.editor_buffers.get_untracked(),
                                                &app_state.dynamic_layouts.get_untracked(),
                                            );
                                            if items.is_empty() {
                                                return;
                                            }
                                            spawn_local(async move {
                                                let _ = download_directory(directory_label, &items).await;
                                            });
                                        }
                                    >
                                        "dl"
                                    </button>
                                </Tooltip>
                            </Show>
                </div>
                    }
                        .into_any()
                } else {
                    view! { <></> }.into_any()
                }}
            <Show when=move || is_directory_open(app_state, &directory_path.get(), default_open.get(), is_root)>
                <div class="grid">
                    {move || {
                        let current = directory.get();
                        current
                            .children
                            .into_iter()
                            .map(|child| view! { <FileTreeNodeView node=child depth=depth + 1 /> })
                            .collect_view()
                    }}
                </div>
            </Show>
        </div>
    }
}

#[component]
pub fn FileTreeNodeView(node: EditorFileTreeNode, #[prop(optional)] depth: usize) -> impl IntoView {
    let app_state = expect_context::<AppState>();

    view! {
        {match node {
            EditorFileTreeNode::Directory(directory) => {
                let child_directory = directory.clone();
                view! {
                    <FileTreeDirectoryView
                        directory=Signal::derive(move || child_directory.clone())
                        depth=depth
                    />
                }
                    .into_any()
            }
            EditorFileTreeNode::File(file_id) => {
                let file = file_by_key(&file_id, &app_state.dynamic_layouts.get());
                let label = file.label.clone();
                let file_key_active = file_id.clone();
                let file_key_open = file_id.clone();

                view! {
                    <button
                        class=move || {
                            if app_state.active_file_id.get() == Some(file_key_active.clone()) {
                                "flex w-full items-center gap-1 py-0.5 text-left text-[13px] leading-[1.15rem] bg-terminal-bg-hover text-terminal-fg-strong"
                            } else {
                                "flex w-full items-center gap-1 py-0.5 text-left text-[13px] leading-[1.15rem] text-terminal-muted hover:bg-terminal-bg-hover hover:text-terminal-fg"
                            }
                        }
                        style=tree_row_style(depth, true, false)
                        on:click=move |_| app_state.select_editor_file(file_key_open.clone())
                    >
                        <span class="w-2 shrink-0 text-terminal-faint">"╰"</span>
                        <span
                            class=move || match file.language.as_str() {
                                "css" => "shrink-0 text-[#7b4fc9]",
                                "typescript" | "typescriptreact" => "shrink-0 text-[#519aba]",
                                _ => "shrink-0 text-terminal-info",
                            }
                        >
                            {match file.language.as_str() {
                                "css" => "",
                                "typescript" | "typescriptreact" => "󰛦",
                                _ => "󰈔",
                            }}
                        </span>
                        <span class="min-w-0 flex-1 truncate">{label}</span>

                    </button>
                }
                    .into_any()
            }
        }}
    }
}
