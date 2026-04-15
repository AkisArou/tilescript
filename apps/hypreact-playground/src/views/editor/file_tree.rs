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

    view! {
        <div class="grid">
            {(!is_root)
                .then(|| {
                    let directory_name_text = directory_name.clone();

                    view! {
                        <div class="group/layout-subtree flex items-center gap-1">
                            <button
                                class="text-terminal-dim flex flex-1 items-center gap-1 py-0.5 text-left text-[13px] leading-[1.15rem] hover:text-terminal-fg"
                                style=tree_row_style(depth, true, is_root)
                                on:click=move |_| {
                                    app_state.toggle_directory(directory_path.clone(), default_open)
                                }
                            >
                                <span class="w-2 shrink-0 text-terminal-faint">"╰"</span>
                                <span class="w-2 text-terminal-faint">
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
                                "flex w-full items-center gap-1 py-0.5 text-left text-[13px] leading-[1.15rem] bg-terminal-bg-hover text-terminal-fg-strong"
                            } else {
                                "flex w-full items-center gap-1 py-0.5 text-left text-[13px] leading-[1.15rem] text-terminal-muted hover:bg-terminal-bg-hover hover:text-terminal-fg"
                            }
                        }
                        style=tree_row_style(depth, true, false)
                        on:click=move |_| app_state.select_editor_file(file_id)
                    >
                        <span class="w-2 shrink-0 text-terminal-faint">"╰"</span>
                        <span
                            class=move || match file.language {
                                "css" => "shrink-0 text-[#7b4fc9]",
                                "typescript" | "typescriptreact" => "shrink-0 text-[#519aba]",
                                _ => "shrink-0 text-terminal-info",
                            }
                        >
                            {match file.language {
                                "css" => "",
                                "typescript" | "typescriptreact" => "󰛦",
                                _ => "󰈔",
                            }}
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
