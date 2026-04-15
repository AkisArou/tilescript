use leptos::ev::KeyboardEvent;
use leptos::prelude::*;

use hypreact_core::command::{FocusDirection, LayoutCycleDirection, WmCommand};
use hypreact_core::snapshot::WindowSnapshot;
use hypreact_scene::LayoutSnapshotNode;

use crate::app_state::AppState;
use crate::session::PreviewDiagnostic;

use super::inspector::{
    LayoutTreeNode, WindowList, WindowOrderSummary, claimed_visible_windows,
    unclaimed_visible_windows,
};
use super::scene::{body_style, frame_style, pane_style};

fn window_display_title(window: &WindowSnapshot) -> &str {
    window.title.as_deref().unwrap_or_else(|| window.id.as_str())
}

fn window_subtitle(window: &WindowSnapshot) -> &str {
    window.app_id.as_deref().unwrap_or("preview window")
}

fn window_accent(window: &WindowSnapshot) -> String {
    const PALETTE: [&str; 8] =
        ["#7dd3fc", "#f97316", "#34d399", "#facc15", "#818cf8", "#06b6d4", "#e879f9", "#fb7185"];

    let seed = window.title.as_deref().or(window.app_id.as_deref()).unwrap_or(window.id.as_str());
    let hash = seed
        .bytes()
        .chain(window.id.as_str().bytes())
        .fold(0usize, |acc, byte| acc + byte as usize);

    PALETTE[hash % PALETTE.len()].to_string()
}

fn focused_window_label(app_state: AppState) -> String {
    let snapshot = app_state.session.get();
    snapshot
        .focused_window_id()
        .as_ref()
        .map(|window_id| snapshot.window_name(window_id))
        .unwrap_or_else(|| "none".to_string())
}

const TOGGLE_BUTTON_BASE: &str = "border px-2 py-0.5 transition-colors duration-150";

fn descendant_window_ids(node: &LayoutSnapshotNode, ids: &mut Vec<String>) {
    if let LayoutSnapshotNode::Window { window_id: Some(window_id), .. } = node {
        ids.push(window_id.as_str().to_string());
    }

    for child in node.children() {
        descendant_window_ids(child, ids);
    }
}

fn descendant_window_count(node: &LayoutSnapshotNode) -> usize {
    let mut ids = Vec::new();
    descendant_window_ids(node, &mut ids);
    ids.len()
}

fn descendant_window_titles(node: &LayoutSnapshotNode, windows: &[WindowSnapshot]) -> String {
    let mut ids = Vec::new();
    descendant_window_ids(node, &mut ids);

    ids.into_iter()
        .map(|window_id| {
            windows
                .iter()
                .find(|window| window.id.as_str() == window_id)
                .map(|window| window_display_title(window).to_string())
                .unwrap_or(window_id)
        })
        .collect::<Vec<_>>()
        .join("  |  ")
}

#[component]
pub fn PreviewView() -> impl IntoView {
    let app_state = expect_context::<AppState>();
    let show_sidebar = app_state.preview_sidebar_open;

    view! {
        <section class=move || {
            if show_sidebar.get() {
                "grid h-full min-h-0 w-full min-w-0 grid-cols-1 gap-2 xl:grid-cols-[minmax(0,1.55fr)_22rem]"
            } else {
                "grid h-full min-h-0 w-full min-w-0 grid-cols-1 gap-2"
            }
        }>
            <div class="grid min-h-0 gap-2">
                <div class="border-terminal-border bg-terminal-bg-subtle flex min-h-0 flex-col overflow-hidden border">
                    <div
                        class="border-terminal-border bg-terminal-bg-bar grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-2 border-b px-2 py-1 text-xs text-terminal-dim"
                        tabindex="0"
                        on:keydown=move |event: KeyboardEvent| {
                            if let Some(command) = preview_command_from_key(&event) {
                                event.prevent_default();
                                app_state.session.update(|state| {
                                    let config = app_state.loaded_config.get_untracked();
                                    state.apply_command(command, config.as_ref());
                                });
                                app_state.request_preview_reevaluation();
                            }
                        }
                    >
                        <div class="flex min-w-0 items-center gap-1 overflow-x-auto">
                            {move || {
                                app_state
                                    .session
                                    .get()
                                    .workspace_names()
                                    .into_iter()
                                    .map(|workspace_name| {
                                        let target = workspace_name.clone();
                                        let current = workspace_name.clone();

                                        view! {
                                            <button
                                                class=move || {
                                                    if app_state.session.get().active_workspace_name() == current {
                                                        format!(
                                                            "{TOGGLE_BUTTON_BASE} border-terminal-info bg-terminal-info/10 text-terminal-info"
                                                        )
                                                    } else {
                                                        format!(
                                                            "{TOGGLE_BUTTON_BASE} border-terminal-border bg-terminal-bg-subtle text-terminal-dim hover:text-terminal-fg"
                                                        )
                                                    }
                                                }
                                                on:click=move |_| {
                                                    app_state.session.update(|state| state.select_workspace(&target));
                                                    app_state.request_preview_reevaluation();
                                                }
                                            >
                                                {workspace_name}
                                            </button>
                                        }
                                    })
                                    .collect_view()
                            }}
                        </div>

                        <div class="min-w-0 truncate px-2 text-center text-terminal-fg-strong">
                            {move || focused_window_label(app_state)}
                        </div>

                        <div class="flex items-center gap-2 justify-self-end">
                            <div class="ui-select-wrap">
                                <select
                                    class="ui-select"
                                    prop:value=move || app_state.session.get().active_layout_name()
                                    on:change=move |event| {
                                        let next = event_target_value(&event);
                                        app_state.session.update(|state| state.set_layout(next));
                                        app_state.request_preview_reevaluation();
                                    }
                                >
                                    {move || {
                                        app_state
                                            .loaded_config
                                            .get()
                                            .map(|config| {
                                                config
                                                    .layouts
                                                    .into_iter()
                                                    .map(|layout| {
                                                        let label = layout.name;
                                                        let value = label.clone();
                                                        view! { <option value=value>{label.clone()}</option> }
                                                            .into_any()
                                                    })
                                                    .collect::<Vec<_>>()
                                            })
                                            .unwrap_or_else(|| {
                                                vec![view! { <option value="none">"none"</option> }.into_any()]
                                            })
                                            .into_iter()
                                            .collect_view()
                                    }}
                                </select>
                            </div>

                            <span>
                                {move || format!("{} windows", app_state.session.get().visible_window_count())}
                            </span>

                            <button
                                class=move || {
                                    if show_sidebar.get() {
                                        format!(
                                            "{TOGGLE_BUTTON_BASE} border-terminal-info bg-terminal-info/10 text-terminal-info"
                                        )
                                    } else {
                                        format!(
                                            "{TOGGLE_BUTTON_BASE} border-terminal-border bg-terminal-bg-subtle text-terminal-dim hover:text-terminal-fg"
                                        )
                                    }
                                }
                                on:click=move |_| app_state.toggle_preview_sidebar()
                            >
                                {move || if show_sidebar.get() { "Hide info" } else { "Show info" }}
                            </button>
                        </div>
                    </div>

                    <div class="min-h-0 flex-1 overflow-hidden">
                        <Show
                            when=move || {
                                let session = app_state.session.get();
                                session.scene.is_some()
                                    || session.error.is_some()
                                    || !session.diagnostics.is_empty()
                            }
                            fallback=move || {
                                view! {
                                    <div class="text-terminal-faint flex h-full min-h-72 items-center justify-center p-3 text-sm">
                                        "loading wasm preview..."
                                    </div>
                                }
                            }
                        >
                            {move || {
                                let session = app_state.session.get();
                                if session.scene.is_none() {
                                    view! {
                                        <div class="border-terminal-border bg-terminal-bg-subtle text-terminal-muted min-h-72 h-full w-full overflow-auto border p-3 text-sm">
                                            <Show when=move || app_state.session.get().error.is_some()>
                                                <div class="border-terminal-error/40 bg-terminal-error/10 text-terminal-error mb-3 border px-3 py-2">
                                                    {move || app_state.session.get().error.unwrap_or_default()}
                                                </div>
                                            </Show>
                                            <DiagnosticsList diagnostics=Signal::derive(move || {
                                                app_state.session.get().diagnostics.clone()
                                            }) />
                                        </div>
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <div class="bg-terminal-bg relative h-full min-h-72 w-full overflow-hidden">
                                            {claimed_visible_windows(&app_state.session.get())
                                                .into_iter()
                                                .filter_map(|window| {
                                                    let session = app_state.session.get();
                                                    let scene = session.scene.clone()?;
                                                    let node = scene.root.find_by_window_id(&window.id)?;
                                                    let rect = node.rect();
                                                    let layout_style =
                                                        node.styles().map(|styles| styles.layout.clone());
                                                    let layout_style_frame = layout_style.clone();
                                                    let layout_style_body = layout_style.clone();
                                                    let pane_focus_target = window.id.clone();
                                                    let focused_id = window.id.clone();
                                                    let focused_attr_id = focused_id.clone();
                                                    let focused_style_id = focused_id.clone();
                                                    let accent = window_accent(&window);

                                                    Some(view! {
                                                        <div
                                                            class="text-terminal-fg absolute z-20 cursor-pointer overflow-hidden text-left text-xs"
                                                            attr:data-focused=move || {
                                                                if app_state.session.get().focused_window_id().as_ref()
                                                                    == Some(&focused_attr_id)
                                                                {
                                                                    "true"
                                                                } else {
                                                                    "false"
                                                                }
                                                            }
                                                            style=move || {
                                                                let session = app_state.session.get();
                                                                format!(
                                                                    "{} {}",
                                                                    pane_style(
                                                                        rect,
                                                                        &accent,
                                                                        1240,
                                                                        760,
                                                                    ),
                                                                    frame_style(
                                                                        layout_style_frame.as_ref(),
                                                                        session.focused_window_id().as_ref()
                                                                            == Some(&focused_style_id),
                                                                    )
                                                                )
                                                            }
                                                            on:click=move |_| {
                                                                app_state.session.update(|state| state.set_focus(pane_focus_target.clone()));
                                                                app_state.request_preview_reevaluation();
                                                            }
                                                        >
                                                            <div style=move || {
                                                                format!(
                                                                    "height: 100%; width: 100%; box-sizing: border-box; {}",
                                                                    body_style(layout_style_body.as_ref()),
                                                                )
                                                            }>
                                                                <WindowSurface window=window.clone() />
                                                            </div>
                                                        </div>
                                                    })
                                                })
                                                .collect_view()}
                                        </div>
                                    }
                                        .into_any()
                                }
                            }}
                        </Show>
                    </div>
                </div>
            </div>

            <Show when=move || show_sidebar.get()>
                <div class="grid min-h-0 gap-2 xl:grid-rows-[auto_auto_auto_minmax(10rem,0.8fr)_minmax(12rem,1fr)]">
                    <InspectorPanel title="session://windows">
                        <WindowList
                            windows=Signal::derive(move || app_state.session.get().visible_windows())
                            empty_label="no windows"
                        />
                    </InspectorPanel>

                    <InspectorPanel title="session://unclaimed">
                        <WindowList
                            windows=Signal::derive(move || {
                                unclaimed_visible_windows(&app_state.session.get())
                            })
                            empty_label="all claimed"
                        />
                    </InspectorPanel>

                    <InspectorPanel title="layout://order">
                        <div class="grid gap-3 p-2 text-sm">
                            <WindowOrderSummary
                                label="input"
                                windows=Signal::derive(move || app_state.session.get().visible_windows())
                            />
                            <WindowOrderSummary
                                label="claimed"
                                windows=Signal::derive(move || {
                                    claimed_visible_windows(&app_state.session.get())
                                })
                            />
                        </div>
                    </InspectorPanel>

                    <InspectorPanel title="scene://diagnostics">
                        <div class="min-h-0 overflow-auto p-2 text-sm">
                            <DiagnosticsList diagnostics=Signal::derive(move || {
                                app_state.session.get().diagnostics.clone()
                            }) />
                        </div>
                    </InspectorPanel>

                    <InspectorPanel title="scene://tree">
                        <div class="min-h-0 overflow-auto p-2">
                            {move || {
                                let session = app_state.session.get();
                                if let Some(scene) = session.scene.clone() {
                                    view! {
                                        <LayoutTreeNode
                                            node=scene.root
                                            windows=session.visible_windows()
                                        />
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <div class="text-terminal-faint text-sm">"no resolved tree"</div>
                                    }
                                        .into_any()
                                }
                            }}
                        </div>
                    </InspectorPanel>
                </div>
            </Show>
        </section>
    }
    .into_any()
}

#[component]
fn InspectorPanel(#[prop(into)] title: Oco<'static, str>, children: Children) -> impl IntoView {
    view! {
        <div class="border-terminal-border bg-terminal-bg-subtle flex min-h-0 flex-col overflow-hidden border">
            <div class="border-terminal-border bg-terminal-bg-bar text-terminal-dim border-b px-2 py-1 text-xs">
                {title}
            </div>
            <div class="min-h-0 flex-1 overflow-auto">{children()}</div>
        </div>
    }
}

#[component]
fn DiagnosticsList(#[prop(into)] diagnostics: Signal<Vec<PreviewDiagnostic>>) -> impl IntoView {
    view! {
        <Show
            when=move || !diagnostics.get().is_empty()
            fallback=move || view! { <div class="p-2 text-sm text-terminal-faint">"no diagnostics"</div> }
        >
            <div class="grid gap-1">
                {move || {
                    diagnostics
                        .get()
                        .into_iter()
                        .map(|diagnostic| {
                            let level_class = if diagnostic.severity == "error" {
                                "text-terminal-error"
                            } else {
                                "text-terminal-warn"
                            };

                            view! {
                                <div class="border-terminal-border bg-terminal-bg-panel px-2 py-1 text-terminal-muted border text-sm">
                                    <div class="flex items-center gap-2 text-xs">
                                        <span class=level_class>{format!("{} {}", diagnostic.severity, diagnostic.code)}</span>
                                        <span class="text-terminal-dim">{diagnostic.path.clone()}</span>
                                    </div>
                                    <div class="mt-1">{diagnostic.message}</div>
                                    <div class="mt-1 text-terminal-faint text-xs">{diagnostic.range}</div>
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </div>
        </Show>
    }
}

#[component]
fn WindowSurface(window: WindowSnapshot) -> impl IntoView {
    view! {
        <div class="text-terminal-muted flex h-full w-full flex-col text-sm">
            <div>
                <div class="text-terminal-fg-strong">{window_subtitle(&window).to_string()}</div>
                <div class="text-terminal-dim mt-1">
                    {window.title.unwrap_or_else(|| "unbound node".to_string())}
                </div>
            </div>
        </div>
    }
}

fn preview_command_from_key(event: &KeyboardEvent) -> Option<WmCommand> {
    match event.key().as_str() {
        "j" if event.shift_key() => Some(WmCommand::SelectNextWorkspace),
        "k" if event.shift_key() => Some(WmCommand::SelectPreviousWorkspace),
        "j" | "ArrowDown" => Some(WmCommand::FocusNextWindow),
        "k" | "ArrowUp" => Some(WmCommand::FocusPreviousWindow),
        "h" | "ArrowLeft" => Some(WmCommand::FocusDirection { direction: FocusDirection::Left }),
        "l" | "ArrowRight" => Some(WmCommand::FocusDirection { direction: FocusDirection::Right }),
        "[" => Some(WmCommand::CycleLayout { direction: Some(LayoutCycleDirection::Previous) }),
        "]" => Some(WmCommand::CycleLayout { direction: Some(LayoutCycleDirection::Next) }),
        "f" => Some(WmCommand::ToggleFloating),
        "Enter" => Some(WmCommand::ToggleFullscreen),
        "x" | "Backspace" | "Delete" => Some(WmCommand::CloseFocusedWindow),
        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" => {
            Some(WmCommand::SelectWorkspace { workspace_id: event.key().as_str().into() })
        }
        _ => None,
    }
}
