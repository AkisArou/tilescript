use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::app_state::AppState;
use crate::components::tooltip::Tooltip;
use crate::editor_files::file_by_id;
use crate::editor_host::download_directory;
use crate::workspace::{EditorFileTreeDirectory, EditorFileTreeNode};

use super::buffers::is_file_dirty;
use super::download::{collect_directory_download_items, download_directory_title};

fn branch_indent(depth: usize, is_root: bool) -> String {
    if is_root {
        "padding-left: 6px".to_string()
    } else {
        format!("padding-left: {}px", depth * 16 + 6)
    }
}

fn is_directory_open(
    app_state: AppState,
    path: &'static str,
    default_open: bool,
    is_root: bool,
) -> bool {
    if is_root {
        true
    } else {
        app_state.directory_open_state.get().get(path).copied().unwrap_or(default_open)
    }
}

#[component]
pub fn FileTreeDirectoryView(
    directory: EditorFileTreeDirectory,
    #[prop(optional)] is_root: bool,
    #[prop(optional)] depth: usize,
) -> impl IntoView {
    let app_state = expect_context::<AppState>();
    let directory_path = directory.path.to_string();
    let directory_name = directory.name.to_string();
    let default_open = directory.default_open;
    let download_title = download_directory_title(&directory);
    let can_download = directory.download_root_path.is_some();
    let download_directory_node = directory.clone();
    let child_nodes = directory.children.clone();
    let branch_style = branch_indent(depth, is_root);

    view! {
        <div class="grid">
            {(!is_root)
                .then(|| {
                    let directory_name_text = directory_name.clone();
                    let row_padding = branch_style.clone();

                    view! {
                        <div class="group/layout-subtree flex items-center gap-1">
                            <button
                                class="text-terminal-dim flex flex-1 items-center gap-1.5 py-0 text-left text-[13px] leading-5 hover:text-terminal-fg"
                                style=row_padding
                                on:click=move |_| {
                                    app_state.toggle_directory(directory_path.clone(), default_open)
                                }
                            >
                                <span class="w-3 text-terminal-faint">"│"</span>
                                <span class="w-3 text-terminal-faint">
                                    {move || {
                                        if app_state
                                            .directory_open_state
                                            .get()
                                            .get(directory.path)
                                            .copied()
                                            .unwrap_or(default_open)
                                        {
                                            "v"
                                        } else {
                                            ">"
                                        }
                                    }}
                                </span>
                                <span class="text-terminal-info shrink-0 text-[12px]">""</span>
                                <span class="min-w-0 flex-1 truncate text-terminal-fg">{directory_name_text}</span>
                            </button>

                            {can_download
                                .then(|| {
                                    let directory_for_download = download_directory_node.clone();
                                    let directory_label = directory_for_download.name;

                                    view! {
                                        <Tooltip
                                            content=Signal::derive({
                                                let download_title = download_title.clone();
                                                move || download_title.clone()
                                            })
                                            class="ml-auto"
                                        >
                                            <button
                                                class="py-0 px-1 text-terminal-faint text-[9px] uppercase tracking-[0.16em] hover:text-terminal-fg"
                                                aria-label=download_title.clone()
                                                on:click=move |event| {
                                                    event.stop_propagation();
                                                    let items = collect_directory_download_items(
                                                        &directory_for_download,
                                                        &app_state.editor_buffers.get_untracked(),
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
                                    }
                                })}
                        </div>
                    }
                })}
            <Show when=move || is_directory_open(app_state, directory.path, default_open, is_root)>
                <div class="grid">
                    {child_nodes
                        .clone()
                        .into_iter()
                        .map(|child| view! { <FileTreeNodeView node=child depth=depth + 1 /> })
                        .collect_view()}
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
                view! { <FileTreeDirectoryView directory=directory depth=depth /> }.into_any()
            }
            EditorFileTreeNode::File(file_id) => {
                let file = file_by_id(file_id);
                let label = file.label.to_string();

                view! {
                    <button
                        class=move || {
                            if app_state.active_file_id.get() == Some(file_id) {
                                "flex w-full items-center gap-1.5 py-0 text-left text-[13px] leading-5 bg-terminal-bg-hover text-terminal-fg-strong"
                            } else {
                                "flex w-full items-center gap-1.5 py-0 text-left text-[13px] leading-5 text-terminal-muted hover:bg-terminal-bg-hover hover:text-terminal-fg"
                            }
                        }
                        style=branch_indent(depth, false)
                        on:click=move |_| app_state.select_editor_file(file_id)
                    >
                        <span class="w-3 text-terminal-faint">"│"</span>
                        <span class="w-3 text-terminal-faint">" "</span>
                        <span
                            class=move || {
                                if file.language == "css" {
                                    "shrink-0 text-[#7b4fc9]"
                                } else {
                                    "shrink-0 text-terminal-info"
                                }
                            }
                        >
                            {if file.language == "css" { "" } else { "󰈔" }}
                        </span>
                        <span class="min-w-0 flex-1 truncate">{label}</span>

                        <Show when=move || is_file_dirty(&app_state.editor_buffers.get(), file_id)>
                            <span class="text-terminal-warn ml-auto shrink-0">"●"</span>
                        </Show>
                    </button>
                }
                    .into_any()
            }
        }}
    }
}
