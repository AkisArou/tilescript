use leptos::prelude::*;

use crate::app_state::AppState;
use crate::editor_files::{file_badge, file_by_id};
use crate::workspace::{EditorFileTreeDirectory, EditorFileTreeNode};

use super::buffers::is_file_dirty;
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
    let child_nodes = directory.children.clone();
    let padding_left = format!("padding-left: {}px", depth * 14 + 8);

    view! {
        <div class="grid">
            {(!is_root)
                .then(|| {
                    let directory_name_text = directory_name.clone();
                    let row_padding = padding_left.clone();

                    view! {
                        <div class="group/layout-subtree flex items-center gap-1">
                            <button
                                class="text-terminal-dim flex flex-1 items-center gap-1 px-2 py-1 text-left text-sm leading-5"
                                style=row_padding
                                on:click=move |_| {
                                    app_state.toggle_directory(directory_path.clone(), default_open)
                                }
                            >
                                <span>
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
                                <span class="min-w-0 flex-1 truncate">{directory_name_text}</span>
                            </button>
                        </div>
                    }
                })}
            <Show when=move || is_root || app_state.directory_open_state.get().get(directory.path).copied().unwrap_or(default_open)>
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
                let badge = file_badge(file.language).to_string();
                let label = file.label.to_string();
                let padding_left = format!("padding-left: {}px", depth * 14 + 8);

                view! {
                    <button
                        class=move || {
                            if app_state.active_file_id.get() == Some(file_id) {
                                "flex w-full items-center gap-2 px-2 py-1 text-left text-sm leading-5 bg-terminal-bg-hover text-terminal-fg-strong"
                            } else {
                                "flex w-full items-center gap-2 px-2 py-1 text-left text-sm leading-5 text-terminal-muted hover:bg-terminal-bg-hover hover:text-terminal-fg"
                            }
                        }
                        style=padding_left
                        on:click=move |_| app_state.select_editor_file(file_id)
                    >
                        <span
                            class=move || {
                                if file.language == "css" {
                                    "shrink-0 text-[#7b4fc9]"
                                } else {
                                    "shrink-0 text-terminal-info"
                                }
                            }
                        >
                            {badge}
                        </span>
                        <span class="min-w-0 flex-1 truncate">{label}</span>

                        <Show when=move || app_state.open_file_ids.get().contains(&file_id)>
                            <span class="text-terminal-dim shrink-0 text-xs">"open"</span>
                        </Show>

                        <Show when=move || is_file_dirty(&app_state.editor_buffers.get(), file_id)>
                            <span class="text-terminal-warn ml-auto shrink-0">"+"</span>
                        </Show>
                    </button>
                }
                    .into_any()
            }
        }}
    }
}
