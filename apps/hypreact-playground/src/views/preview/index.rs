use std::sync::{Arc, Mutex};

use leptos::ev::KeyboardEvent;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use hypreact_core::command::{FocusDirection, LayoutCycleDirection, WmCommand};
use hypreact_core::snapshot::WindowSnapshot;

use crate::app_state::AppState;
use crate::session::PreviewSessionState;
use crate::views::editor::EditorView;

use super::scene::{body_style, frame_style, pane_style};

#[derive(Clone, Copy, Debug)]
struct HtopMetrics {
    tick: u32,
}

impl HtopMetrics {
    fn cpu_usage(self, index: usize) -> f32 {
        let base = [69.0, 61.0, 48.0, 34.0][index];
        let phase = self.tick as f32 * 0.23 + index as f32 * 0.82;
        (base + phase.sin() * 8.5 + (phase * 0.41).cos() * 3.25).clamp(9.0, 96.0)
    }

    fn cpu_avg(self, index: usize) -> u32 {
        let value =
            [29.0, 17.0, 9.0, 5.0][index] + (self.tick as f32 * 0.37 + index as f32).cos() * 2.4;
        value.round().clamp(1.0, 40.0) as u32
    }

    fn load_average(self) -> (f32, f32, f32) {
        (
            1.42 + (self.tick as f32 * 0.09).sin() * 0.19,
            1.27 + (self.tick as f32 * 0.06 + 0.8).sin() * 0.14,
            1.08 + (self.tick as f32 * 0.04 + 1.4).sin() * 0.11,
        )
    }

    fn memory_gb(self) -> f32 {
        (6.24 + (self.tick as f32 * 0.07).sin() * 0.31).clamp(5.9, 6.7)
    }

    fn swap_mb(self) -> u32 {
        (248.0 + (self.tick as f32 * 0.11).cos() * 18.0).round().clamp(200.0, 320.0) as u32
    }

    fn battery(self) -> u32 {
        (92.0 + (self.tick as f32 * 0.05).sin() * 2.0).round().clamp(88.0, 96.0) as u32
    }

    fn process_cpu(self, index: usize) -> f32 {
        let base = [33.2, 17.4, 8.1, 1.0][index];
        (base
            + (self.tick as f32 * (0.17 + index as f32 * 0.03)).sin() * (2.6 - index as f32 * 0.35))
            .clamp(0.1, 38.5)
    }

    fn process_mem(self, index: usize) -> f32 {
        let base = [4.5, 3.0, 1.9, 0.0][index];
        (base + (self.tick as f32 * (0.09 + index as f32 * 0.02)).cos() * 0.18).clamp(0.0, 5.1)
    }

    fn process_time(self, index: usize) -> String {
        let total_seconds =
            [72.0, 44.0, 13.0, 2.0][index] + self.tick as f32 * [0.82, 0.56, 0.34, 0.08][index];
        let minutes = (total_seconds / 60.0).floor() as u32;
        let seconds = (total_seconds % 60.0).floor() as u32;
        let centis = ((total_seconds.fract()) * 100.0).round() as u32 % 100;
        format!("{minutes:02}:{seconds:02}.{centis:02}")
    }

    fn process_order(self) -> [usize; 4] {
        let mut order = [0usize, 1, 2, 3];
        order.sort_by(|left, right| {
            self.process_cpu(*right)
                .partial_cmp(&self.process_cpu(*left))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        order
    }
}

fn htop_meter(value: f32, width: usize) -> String {
    let filled = ((value / 100.0) * width as f32).round().clamp(0.0, width as f32) as usize;
    format!("[{}{} {:>4.1}%]", "|".repeat(filled), " ".repeat(width - filled), value)
}

fn htop_memory_meter(used: f32, total: f32, width: usize) -> String {
    let ratio = (used / total * 100.0).clamp(0.0, 100.0);
    let filled = ((ratio / 100.0) * width as f32).round().clamp(0.0, width as f32) as usize;
    format!("[{}{} {:>4.2}G/{total:.1}G]", "|".repeat(filled), " ".repeat(width - filled), used)
}

fn htop_swap_meter(used_mb: u32, total_gb: f32, width: usize) -> String {
    let total_mb = total_gb * 1024.0;
    let ratio = (used_mb as f32 / total_mb * 100.0).clamp(0.0, 100.0);
    let filled = ((ratio / 100.0) * width as f32).round().clamp(0.0, width as f32) as usize;
    format!("[{}{} {:>3}M/{total_gb:.2}G]", "|".repeat(filled), " ".repeat(width - filled), used_mb)
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

fn claimed_visible_windows(session: &PreviewSessionState) -> Vec<WindowSnapshot> {
    let Some(scene) = session.scene.as_ref() else {
        return Vec::new();
    };

    session
        .visible_windows()
        .into_iter()
        .filter(|window| scene.root.find_by_window_id(&window.id).is_some())
        .collect()
}

#[derive(Clone)]
struct PreviewWindowRenderState {
    window: WindowSnapshot,
    rect: hypreact_core::LayoutRect,
    layout_style: Option<hypreact_scene::ComputedStyle>,
    accent: String,
}

fn preview_window_render_states(session: &PreviewSessionState) -> Vec<PreviewWindowRenderState> {
    let Some(scene) = session.scene.as_ref() else {
        return Vec::new();
    };

    claimed_visible_windows(session)
        .into_iter()
        .filter_map(|window| {
            let node = scene.root.find_by_window_id(&window.id)?;
            Some(PreviewWindowRenderState {
                accent: window_accent(&window),
                rect: node.rect(),
                layout_style: node.styles().map(|styles| styles.layout.clone()),
                window,
            })
        })
        .collect()
}

fn preview_window_render_state_for_id(
    session: &PreviewSessionState,
    window_id: &hypreact_core::WindowId,
) -> Option<PreviewWindowRenderState> {
    let scene = session.scene.as_ref()?;
    let window =
        claimed_visible_windows(session).into_iter().find(|window| window.id == *window_id)?;
    let node = scene.root.find_by_window_id(window_id)?;

    Some(PreviewWindowRenderState {
        accent: window_accent(&window),
        rect: node.rect(),
        layout_style: node.styles().map(|styles| styles.layout.clone()),
        window,
    })
}

fn preview_diagnostics_summary(session: &PreviewSessionState) -> Option<String> {
    if let Some(error) = session.error.as_ref() {
        return Some(error.clone());
    }

    let first = session.diagnostics.first()?;
    let remaining = session.diagnostics.len().saturating_sub(1);

    Some(if remaining == 0 {
        format!("{} {} {}: {}", first.severity, first.code, first.path, first.message)
    } else {
        format!(
            "{} {} {}: {} (+{} more)",
            first.severity, first.code, first.path, first.message, remaining
        )
    })
}

fn preview_diagnostics_tone(session: &PreviewSessionState) -> &'static str {
    if session.error.is_some()
        || session.diagnostics.iter().any(|diagnostic| diagnostic.severity == "error")
    {
        "error"
    } else if !session.diagnostics.is_empty() {
        "warning"
    } else {
        "idle"
    }
}

#[component]
pub fn PreviewView() -> impl IntoView {
    let app_state = expect_context::<AppState>();

    {
        let app_state = app_state;
        let installed = StoredValue::new(false);
        Effect::new(move |_| {
            if installed.get_value() {
                return;
            }
            let Some(window) = web_sys::window() else {
                return;
            };

            let handle_keydown = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
                if event.default_prevented() || should_ignore_preview_key_event(&event) {
                    return;
                }

                let event: KeyboardEvent = event.unchecked_into();
                if let Some(action) = preview_command_from_key(&event) {
                    event.prevent_default();
                    match action {
                        PreviewKeyAction::Command(command) => {
                            app_state.session.update(|state| {
                                let config = app_state.loaded_config.get_untracked();
                                state.apply_command(command, config.as_ref());
                            });
                        }
                    }
                    app_state.request_preview_reevaluation();
                }
            })
                as Box<dyn FnMut(web_sys::KeyboardEvent)>);

            window.set_onkeydown(Some(handle_keydown.as_ref().unchecked_ref()));
            handle_keydown.forget();
            installed.set_value(true);
        });
    }

    view! {
        <section class="grid h-full min-h-0 w-full min-w-0 grid-cols-1 gap-2">
            <div class="grid min-h-0 gap-2">
                <div class="border-terminal-border bg-terminal-bg-subtle flex min-h-0 flex-col overflow-hidden border">
                    <div
                        class="grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 bg-terminal-topbar px-2 text-xs text-terminal-dim"
                    >
                        <PreviewWorkspaceTabs />
                        <PreviewDiagnosticsBanner />
                        <PreviewToolbar />
                    </div>

                    <div class="min-h-0 flex-1 overflow-hidden">
                        <PreviewSceneSurface />
                    </div>
                </div>
            </div>
        </section>
    }
    .into_any()
}

#[component]
fn PreviewWorkspaceTabs() -> impl IntoView {
    let app_state = expect_context::<AppState>();

    view! {
        <div class="flex min-w-0 items-center gap-0 overflow-x-auto py-1">
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
                                        "border border-transparent bg-[#2b2b2b] px-2 py-0.5 text-terminal-fg transition-colors duration-150"
                                    } else {
                                        "border border-transparent bg-transparent px-2 py-0.5 text-terminal-faint transition-colors duration-150 hover:text-terminal-dim"
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
    }
}

#[component]
fn PreviewDiagnosticsBanner() -> impl IntoView {
    let app_state = expect_context::<AppState>();

    view! {
        <div
            class=move || {
                let session = app_state.session.get();
                match preview_diagnostics_tone(&session) {
                    "error" => "min-w-0 truncate py-1 text-terminal-error",
                    "warning" => "min-w-0 truncate py-1 text-terminal-warn",
                    _ => "min-w-0 truncate py-1 text-terminal-faint",
                }
            }
            title=move || preview_diagnostics_summary(&app_state.session.get()).unwrap_or_else(|| {
                "No diagnostics. Preview keeps showing the last successful scene.".to_string()
            })
        >
            {move || {
                preview_diagnostics_summary(&app_state.session.get())
                    .unwrap_or_else(|| "No diagnostics. Preview shows the live scene.".to_string())
            }}
        </div>
    }
}

#[component]
fn PreviewToolbar() -> impl IntoView {
    let app_state = expect_context::<AppState>();

    view! {
        <div class="flex items-center justify-self-end py-1">
            <div class="ui-select-wrap mr-1">
                <select
                    class="ui-select"
                    name="layout"
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
        </div>
    }
}

#[component]
fn BindsWindow() -> impl IntoView {
    #[derive(Clone, Copy)]
    enum BindsLine<'a> {
        Blank,
        Comment(&'a str),
        Var(&'a str),
        Bind { mods: &'a str, key: &'a str, command: &'a str, arg: &'a str },
    }

    let lines: &[BindsLine<'_>] = &[
        BindsLine::Comment("####################"),
        BindsLine::Comment("### KEYBINDINGS ###"),
        BindsLine::Comment("####################"),
        BindsLine::Blank,
        BindsLine::Var("$mainMod = ALT"),
        BindsLine::Blank,
        BindsLine::Comment("# Base"),
        BindsLine::Bind { mods: "$mainMod", key: "RETURN", command: "exec", arg: "$openRandom" },
        BindsLine::Bind { mods: "$mainMod", key: "Q", command: "killactive", arg: "" },
        BindsLine::Blank,
        BindsLine::Comment("# Move focus with mainMod + hjkl"),
        BindsLine::Bind { mods: "$mainMod", key: "H", command: "hypreact:movefocus", arg: "left" },
        BindsLine::Bind { mods: "$mainMod", key: "L", command: "hypreact:movefocus", arg: "right" },
        BindsLine::Bind { mods: "$mainMod", key: "K", command: "hypreact:movefocus", arg: "up" },
        BindsLine::Bind { mods: "$mainMod", key: "J", command: "hypreact:movefocus", arg: "down" },
        BindsLine::Blank,
        BindsLine::Comment("# Move focused window with mainMod + Shift + hjkl"),
        BindsLine::Bind {
            mods: "$mainMod SHIFT",
            key: "H",
            command: "hypreact:movewindow",
            arg: "left",
        },
        BindsLine::Bind {
            mods: "$mainMod SHIFT",
            key: "L",
            command: "hypreact:movewindow",
            arg: "right",
        },
        BindsLine::Bind {
            mods: "$mainMod SHIFT",
            key: "K",
            command: "hypreact:movewindow",
            arg: "up",
        },
        BindsLine::Bind {
            mods: "$mainMod SHIFT",
            key: "J",
            command: "hypreact:movewindow",
            arg: "down",
        },
        BindsLine::Blank,
        BindsLine::Comment("# Resize focused window with mainMod + Control + hjkl"),
        BindsLine::Bind {
            mods: "$mainMod CONTROL",
            key: "H",
            command: "hypreact:resizewindow",
            arg: "left",
        },
        BindsLine::Bind {
            mods: "$mainMod CONTROL",
            key: "L",
            command: "hypreact:resizewindow",
            arg: "right",
        },
        BindsLine::Bind {
            mods: "$mainMod CONTROL",
            key: "K",
            command: "hypreact:resizewindow",
            arg: "up",
        },
        BindsLine::Bind {
            mods: "$mainMod CONTROL",
            key: "J",
            command: "hypreact:resizewindow",
            arg: "down",
        },
        BindsLine::Blank,
        BindsLine::Comment("# Switch workspaces with mainMod + [0-9]"),
        BindsLine::Bind { mods: "$mainMod", key: "1", command: "workspace", arg: "1" },
        BindsLine::Bind { mods: "$mainMod", key: "2", command: "workspace", arg: "2" },
        BindsLine::Bind { mods: "$mainMod", key: "3", command: "workspace", arg: "3" },
        BindsLine::Bind { mods: "$mainMod", key: "4", command: "workspace", arg: "4" },
        BindsLine::Bind { mods: "$mainMod", key: "5", command: "workspace", arg: "5" },
        BindsLine::Bind { mods: "$mainMod", key: "6", command: "workspace", arg: "6" },
        BindsLine::Bind { mods: "$mainMod", key: "7", command: "workspace", arg: "7" },
        BindsLine::Bind { mods: "$mainMod", key: "8", command: "workspace", arg: "8" },
        BindsLine::Bind { mods: "$mainMod", key: "9", command: "workspace", arg: "9" },
        BindsLine::Bind { mods: "$mainMod", key: "0", command: "workspace", arg: "10" },
    ];

    view! {
        <div class="h-full w-full overflow-auto bg-[#1b1c1d] px-2 py-2 font-mono text-[0.8rem] leading-[1.45] text-[#d6d6d6]">
            <div class="grid auto-rows-min gap-0">
                {lines
                    .iter()
                    .enumerate()
                    .map(|(index, line)| {
                        let line_number = index + 1;
                        match *line {
                            BindsLine::Blank => view! {
                                <div class="grid grid-cols-[2.25rem_minmax(0,1fr)] gap-2">
                                    <div class="pr-2 text-right text-[#5c6370]">{line_number}</div>
                                    <div class="h-2.5" />
                                </div>
                            }
                                .into_any(),
                            BindsLine::Comment(text) => view! {
                                <div class="grid grid-cols-[2.25rem_minmax(0,1fr)] gap-2">
                                    <div class="pr-2 text-right text-[#5c6370]">{line_number}</div>
                                    <div class="text-[#7fb05f]">{text.to_string()}</div>
                                </div>
                            }
                                .into_any(),
                            BindsLine::Var(left) => view! {
                                <div class="grid grid-cols-[2.25rem_minmax(0,1fr)] gap-2">
                                    <div class="pr-2 text-right text-[#5c6370]">{line_number}</div>
                                    <div>
                                        <span class="text-[#82aaff]">{left.to_string()}</span>
                                    </div>
                                </div>
                            }
                                .into_any(),
                            BindsLine::Bind {
                                mods,
                                key,
                                command,
                                arg,
                            } => view! {
                                <div class="grid grid-cols-[2.25rem_minmax(0,1fr)] gap-2">
                                    <div class="pr-2 text-right text-[#5c6370]">{line_number}</div>
                                    <div>
                                        <span class="text-[#d989d9]">"bind"</span>
                                        <span class="text-[#d6d6d6]">" = "</span>
                                        <span class="text-[#7db7f0]">{mods.to_string()}</span>
                                        <span class="text-[#d6d6d6]">", "</span>
                                        <span class="text-[#f2f2f2]">{key.to_string()}</span>
                                        <span class="text-[#d6d6d6]">", "</span>
                                        <span class="text-[#dcdcdc]">{command.to_string()}</span>
                                        <span class="text-[#d6d6d6]">", "</span>
                                        <span class="text-[#dcdcdc]">{arg.to_string()}</span>
                                    </div>
                                </div>
                            }
                                .into_any(),
                        }
                    })
                    .collect_view()}
            </div>
        </div>
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

#[component]
fn PreviewSceneSurface() -> impl IntoView {
    let app_state = expect_context::<AppState>();

    view! {
        <div
            class="bg-terminal-bg relative h-full min-h-72 w-full overflow-hidden"
            style="background-image: linear-gradient(rgba(10, 10, 10, 0.55), rgba(10, 10, 10, 0.72)), url('/assets/hyprland-wallpaper.png'); background-position: center; background-size: cover;"
        >
            <For
                each=move || preview_window_render_states(&app_state.session.get())
                key=|state| state.window.id.clone()
                children=move |state| {
                    view! { <PreviewWindowFrame state=state /> }
                }
            />
        </div>
    }
}

#[component]
fn PreviewWindowFrame(state: PreviewWindowRenderState) -> impl IntoView {
    let app_state = expect_context::<AppState>();
    let window_id = state.window.id.clone();
    let pane_focus_target = state.window.id.clone();
    let is_foot = state.window.app_id.as_deref() == Some("foot");
    let is_htop = state.window.app_id.as_deref() == Some("htop");
    let is_nvim = state.window.app_id.as_deref() == Some("nvim");
    let is_playground_editor = state.window.app_id.as_deref() == Some("playground-editor");
    let is_binds = state.window.app_id.as_deref() == Some("binds");
    let foot_focus_window_id = state.window.id.clone();
    let focused_attr_id = state.window.id.clone();
    let focused_style_id = state.window.id.clone();

    view! {
        <div
            class="preview-window text-terminal-fg absolute z-20 cursor-pointer overflow-hidden text-left text-xs"
            attr:data-focused=move || {
                if app_state.session.get().focused_window_id().as_ref() == Some(&focused_attr_id) {
                    "true"
                } else {
                    "false"
                }
            }
            style=move || {
                let focused = app_state.session.get().focused_window_id().as_ref() == Some(&focused_style_id);
                let Some(render_state) = app_state
                    .session
                    .with(|session| preview_window_render_state_for_id(session, &focused_style_id))
                else {
                    return String::new();
                };
                format!(
                    "{} {}",
                    pane_style(render_state.rect, &render_state.accent, 1240, 760),
                    frame_style(render_state.layout_style.as_ref(), focused),
                )
            }
            on:click=move |_| {
                app_state.session.update(|session| session.set_focus(pane_focus_target.clone()));
                app_state.request_preview_reevaluation();
            }
        >
            <div style=move || {
                let layout_style = app_state
                    .session
                    .with(|session| {
                        preview_window_render_state_for_id(session, &window_id)
                            .and_then(|state| state.layout_style)
                    });
                format!(
                    "height: 100%; width: 100%; box-sizing: border-box; {}",
                    body_style(layout_style.as_ref()),
                )
            }>
                {if is_foot {
                    view! {
                        <FootTerminal focused=Signal::derive(move || {
                            app_state.session.get().focused_window_id().as_ref()
                                == Some(&foot_focus_window_id)
                        }) />
                    }
                        .into_any()
                } else if is_htop {
                    view! { <HtopWindow /> }.into_any()
                } else if is_nvim {
                    view! { <NvimStartupWindow /> }.into_any()
                } else if is_playground_editor {
                    view! { <PreviewEmbeddedEditorWindow /> }.into_any()
                } else if is_binds {
                    view! { <BindsWindow /> }.into_any()
                } else {
                    view! { <WindowSurface window=state.window.clone() /> }.into_any()
                }}
            </div>
        </div>
    }
}

#[component]
fn PreviewEmbeddedEditorWindow() -> impl IntoView {
    view! {
        <div
            class="h-full w-full overflow-hidden bg-terminal-bg"
            on:click=|event| event.stop_propagation()
            on:mousedown=|event| event.stop_propagation()
        >
            <EditorView />
        </div>
    }
}

#[component]
fn FootTerminal(focused: Signal<bool>) -> impl IntoView {
    view! {
        <div class="bg-terminal-bg text-terminal-fg flex h-full w-full items-start p-3 text-sm">
            <div>
                <span class=move || {
                    if focused.get() { "text-terminal-info" } else { "text-terminal-faint" }
                }>
                    "akisarou@hypreact"
                </span>
                <span class="text-terminal-dim">":$ "</span>
                <Show when=move || focused.get()>
                    <span class="foot-cursor bg-terminal-fg-strong inline-block h-4 w-2 align-[-0.125rem]" />
                </Show>
            </div>
        </div>
    }
}

#[component]
fn HtopWindow() -> impl IntoView {
    let metrics = RwSignal::new(HtopMetrics { tick: 0 });
    let timer = Arc::new(Mutex::new(None::<i32>));
    let timer_for_effect = Arc::clone(&timer);

    Effect::new(move |_| {
        if timer_for_effect.lock().map(|timer| timer.is_some()).unwrap_or(false) {
            return;
        }

        let Some(window) = web_sys::window() else {
            return;
        };

        let tick_metrics = metrics;
        let callback = Closure::wrap(Box::new(move || {
            tick_metrics.update(|value| {
                value.tick = value.tick.wrapping_add(1);
            });
        }) as Box<dyn FnMut()>);
        let callback = callback.into_js_value();
        let callback: js_sys::Function = callback.unchecked_into();

        let interval_id = window
            .set_interval_with_callback_and_timeout_and_arguments_0(callback.unchecked_ref(), 900)
            .ok();

        if let Some(interval_id) = interval_id {
            if let Ok(mut timer) = timer_for_effect.lock() {
                *timer = Some(interval_id);
            }
        }
    });

    on_cleanup(move || {
        let interval_id = timer.lock().ok().and_then(|mut timer| timer.take());
        if let Some(interval_id) = interval_id {
            if let Some(window) = web_sys::window() {
                window.clear_interval_with_handle(interval_id);
            }
        }
    });

    let process_rows = [
        (
            "1842",
            "akisarou",
            "20",
            "0",
            "612M",
            "164M",
            "142M",
            "R",
            "34.7",
            "4.6",
            "01:12.48",
            "cargo check -p hypreact-playground",
        ),
        (
            "2210",
            "akisarou",
            "20",
            "0",
            "488M",
            "118M",
            "8844",
            "S",
            "18.1",
            "3.1",
            "00:44.20",
            "trunk serve --config apps/hypreact-playground/Trunk.toml",
        ),
        (
            "0990",
            "akisarou",
            "20",
            "0",
            "222M",
            "6844",
            "2192",
            "S",
            "7.3",
            "1.8",
            "00:13.91",
            "nvim apps/hypreact-playground/src/views/preview/index.rs",
        ),
        (
            "0431",
            "root",
            "20",
            "0",
            "0",
            "0",
            "0",
            "S",
            "1.2",
            "0.0",
            "00:02.02",
            "[kworker/u16:4-events_unbound]",
        ),
    ];

    let footer_keys = [
        ("F1", "Help"),
        ("F2", "Setup"),
        ("F3", "Search"),
        ("F4", "Filter"),
        ("F5", "Tree"),
        ("F6", "SortBy"),
        ("F7", "Nice -"),
        ("F8", "Nice +"),
        ("F9", "Kill"),
        ("F10", "Quit"),
    ];

    view! {
        <div class="flex h-full w-full flex-col bg-[#0f0f0f] p-3 text-[12px] leading-5 text-[#d6d6d6]">
            <div class="grid gap-1">
                <div class="flex items-center justify-between text-[#8bd450]">
                    <span>{move || {
                        let metrics = metrics.get();
                        format!(
                            "  0{}   Tasks: 142, 498 thr; 2 running",
                            htop_meter(metrics.cpu_usage(0), 18)
                        )
                    }}</span>
                    <span>{move || {
                        let (a, b, c) = metrics.get().load_average();
                        format!("Load average: {a:.2} {b:.2} {c:.2}")
                    }}</span>
                </div>
                <div class="grid gap-0.5">
                    {(0..4).map(|index| {
                        view! {
                            <div class="grid grid-cols-[3rem_minmax(0,1fr)_4rem_3rem] gap-2">
                                <span class="text-[#7dd3fc]">{format!("CPU{index}")}</span>
                                <span class="text-[#8bd450]">{move || htop_meter(metrics.get().cpu_usage(index), 18)}</span>
                                <span class="text-right text-[#f4d35e]">{move || format!("{:.1}%", metrics.get().cpu_usage(index))}</span>
                                <span class="text-right text-[#c792ea]">{move || format!("{:02}", metrics.get().cpu_avg(index))}</span>
                            </div>
                        }
                    }).collect_view()}
                </div>
                <div class="grid grid-cols-2 gap-4 pt-1">
                    <div class="truncate">
                        <span class="text-[#7dd3fc]">"Mem"</span>
                        <span class="ml-2 text-[#8bd450]">{move || htop_memory_meter(metrics.get().memory_gb(), 11.7, 17)}</span>
                    </div>
                    <div class="truncate">
                        <span class="text-[#7dd3fc]">"Swp"</span>
                        <span class="ml-2 text-[#f4d35e]">{move || htop_swap_meter(metrics.get().swap_mb(), 8.0, 22)}</span>
                    </div>
                </div>
                <div class="grid grid-cols-2 gap-4">
                    <div>
                        <span class="text-[#7dd3fc]">"Uptime"</span>
                        <span class="ml-2">"01:24:17"</span>
                    </div>
                    <div>
                        <span class="text-[#7dd3fc]">"Battery"</span>
                        <span class="ml-2 text-[#8bd450]">{move || format!("{}%", metrics.get().battery())}</span>
                    </div>
                </div>
            </div>

            <div class="mt-3 min-h-0 flex-1 overflow-hidden border border-[#2d2d2d]">
                <div class="grid grid-cols-[4rem_6rem_3rem_3rem_4rem_4rem_4rem_3rem_4rem_4rem_7rem_minmax(0,1fr)] gap-2 border-b border-[#2d2d2d] bg-[#1a1a1a] px-2 py-1 text-[11px] uppercase tracking-[0.14em] text-[#9fb3c8]">
                    <span>"PID"</span>
                    <span>"USER"</span>
                    <span>"PRI"</span>
                    <span>"NI"</span>
                    <span>"VIRT"</span>
                    <span>"RES"</span>
                    <span>"SHR"</span>
                    <span>"S"</span>
                    <span>"CPU%"</span>
                    <span>"MEM%"</span>
                    <span>"TIME+"</span>
                    <span>"Command"</span>
                </div>
                <div class="grid auto-rows-min gap-px bg-[#2d2d2d]">
                    {move || {
                        let order = metrics.get().process_order();
                        order.into_iter().enumerate().map(|(index, row_index)| {
                        let row = process_rows[row_index];
                        let row_class = if index == 0 {
                            "bg-[#2b2b2b] text-[#f1f1f1]"
                        } else {
                            "bg-[#0f0f0f] text-[#d6d6d6]"
                        };
                        view! {
                            <div class=format!("grid grid-cols-[4rem_6rem_3rem_3rem_4rem_4rem_4rem_3rem_4rem_4rem_7rem_minmax(0,1fr)] gap-2 px-2 py-1 {row_class}")>
                                <span>{row.0}</span>
                                <span>{row.1}</span>
                                <span>{row.2}</span>
                                <span>{row.3}</span>
                                <span>{row.4}</span>
                                <span>{row.5}</span>
                                <span>{row.6}</span>
                                <span>{row.7}</span>
                                <span>{move || format!("{:.1}", metrics.get().process_cpu(row_index))}</span>
                                <span>{move || format!("{:.1}", metrics.get().process_mem(row_index))}</span>
                                <span>{move || metrics.get().process_time(row_index)}</span>
                                <span class="truncate">{row.11}</span>
                            </div>
                        }
                    }).collect_view()}}
                </div>
            </div>

            <div class="mt-2 grid grid-cols-5 gap-1 text-[11px]">
                {footer_keys.into_iter().map(|(key, label)| {
                    view! {
                        <div class="px-1.5 py-0.5 text-[#d6d6d6]">
                            <span class="px-1 text-[#f4d35e]">{key}</span>
                            <span class="ml-1">{label}</span>
                        </div>
                    }
                }).collect_view()}
            </div>
        </div>
    }
}

#[component]
fn NvimStartupWindow() -> impl IntoView {
    let intro_lines = [
        ("Nvim v0.13.0-dev-172+g7bb8231577", "text-[#d08f8f]"),
        ("", "text-[#d4d4d4]"),
        ("Nvim is open source and freely distributable", "text-[#d4d4d4]"),
        ("https://neovim.io/#chat", "text-[#d4d4d4]"),
    ];
    let help_lines = [
        (":help nvim<Enter>", "if you are new!"),
        (":checkhealth<Enter>", "to optimize Nvim"),
        (":q<Enter>", "to exit"),
        (":help<Enter>", "for help"),
    ];
    let news_line = (":help news<Enter>", "for v0.13 notes");
    let donate_line = (":help KCC<Enter>", "for information");
    let logo_lines = [
        ("███╗   ██╗", "text-[#d7ba7d]"),
        ("████╗  ██║", "text-[#d7ba7d]"),
        ("██╔██╗ ██║", "text-[#c586c0]"),
        ("██║╚██╗██║", "text-[#d08f8f]"),
        ("██║ ╚████║", "text-[#d08f8f]"),
        ("╚═╝  ╚═══╝", "text-[#d08f8f]"),
    ];

    view! {
        <div class="flex h-full w-full flex-col bg-[#1b1c1d] text-[7px] text-[#d4d4d4]">
            <div class="flex items-start gap-2 px-3 pt-1 text-[0.72rem] text-[#b9b9b9]">
                <span>"1"</span>
                <div class="relative mt-0.5 h-3.5 flex-1 bg-[#232425]">
                    <span class="absolute left-0 top-0 inline-block h-3.5 w-1.5 bg-[#d4d4d4]" />
                </div>
            </div>

            <div class="flex min-h-0 flex-1 items-center justify-center px-6 pb-4 pt-4">
                <div class="grid w-full max-w-[18rem] gap-1 text-center text-[0.6rem] leading-4.5">
                    <div class="grid justify-center gap-0 text-[0.8rem] leading-[1.0]">
                        {logo_lines
                            .into_iter()
                            .map(|(line, class_name)| view! { <div class=class_name>{line}</div> })
                            .collect_view()}
                    </div>

                    <div class="mx-auto h-px w-[15rem] bg-[#5a5a5a]" />

                    <div class="grid gap-0 text-[0.6rem] leading-4.5">
                        {intro_lines
                            .into_iter()
                            .map(|(line, class_name)| view! { <div class=class_name>{line}</div> })
                            .collect_view()}
                    </div>

                    <div class="mx-auto h-px w-[15rem] bg-[#5a5a5a]" />

                    <div class="grid justify-center gap-0 text-left text-[0.6rem] leading-4.5">
                        {help_lines
                            .into_iter()
                            .map(|(command, description)| {
                                view! {
                                    <div class="grid grid-cols-[2.1rem_7.7rem_minmax(0,1fr)] gap-1.5">
                                        <span>"type"</span>
                                        <span class="text-[#61afef]">{command}</span>
                                        <span>{description}</span>
                                    </div>
                                }
                            })
                            .collect_view()}
                    </div>

                    <div class="mx-auto h-px w-[15rem] bg-[#5a5a5a]" />

                    <div class="grid justify-center gap-0 text-left text-[0.6rem] leading-4.5">
                        <div class="grid grid-cols-[2.1rem_7.7rem_minmax(0,1fr)] gap-1.5">
                            <span>"type"</span>
                            <span class="text-[#61afef]">{news_line.0}</span>
                            <span>{news_line.1}</span>
                        </div>
                    </div>

                    <div class="mx-auto h-px w-[15rem] bg-[#5a5a5a]" />

                    <div class="grid gap-0 text-[0.6rem] leading-4.5">
                        <div>"Help poor children in Uganda!"</div>
                        <div class="grid justify-center text-left">
                            <div class="grid grid-cols-[2.1rem_7.7rem_minmax(0,1fr)] gap-1.5">
                                <span>"type"</span>
                                <span class="text-[#61afef]">{donate_line.0}</span>
                                <span>{donate_line.1}</span>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            <div class="mt-auto flex items-center justify-between px-3 pb-1.5 pt-1 text-[0.64rem] text-[#b9b9b9]">
                <span>"[No Name]"</span>
                <div class="flex items-center gap-4">
                    <span>"Top"</span>
                    <span>"1:1"</span>
                </div>
            </div>
        </div>
    }
}

enum PreviewKeyAction {
    Command(WmCommand),
}

fn preview_command_from_key(event: &KeyboardEvent) -> Option<PreviewKeyAction> {
    let key = event.key();
    let key_lower = key.to_ascii_lowercase();

    if event.alt_key() {
        if event.shift_key() {
            return match key_lower.as_str() {
                "h" => Some(PreviewKeyAction::Command(WmCommand::MoveDirection {
                    direction: FocusDirection::Left,
                })),
                "j" => Some(PreviewKeyAction::Command(WmCommand::MoveDirection {
                    direction: FocusDirection::Down,
                })),
                "k" => Some(PreviewKeyAction::Command(WmCommand::MoveDirection {
                    direction: FocusDirection::Up,
                })),
                "l" => Some(PreviewKeyAction::Command(WmCommand::MoveDirection {
                    direction: FocusDirection::Right,
                })),
                _ => None,
            };
        }

        if event.ctrl_key() {
            return match key_lower.as_str() {
                "h" => Some(PreviewKeyAction::Command(WmCommand::ResizeDirection {
                    direction: FocusDirection::Left,
                })),
                "j" => Some(PreviewKeyAction::Command(WmCommand::ResizeDirection {
                    direction: FocusDirection::Down,
                })),
                "k" => Some(PreviewKeyAction::Command(WmCommand::ResizeDirection {
                    direction: FocusDirection::Up,
                })),
                "l" => Some(PreviewKeyAction::Command(WmCommand::ResizeDirection {
                    direction: FocusDirection::Right,
                })),
                _ => None,
            };
        }

        return match key_lower.as_str() {
            "h" => Some(PreviewKeyAction::Command(WmCommand::FocusDirection {
                direction: FocusDirection::Left,
            })),
            "j" => Some(PreviewKeyAction::Command(WmCommand::FocusDirection {
                direction: FocusDirection::Down,
            })),
            "k" => Some(PreviewKeyAction::Command(WmCommand::FocusDirection {
                direction: FocusDirection::Up,
            })),
            "l" => Some(PreviewKeyAction::Command(WmCommand::FocusDirection {
                direction: FocusDirection::Right,
            })),
            "q" => Some(PreviewKeyAction::Command(WmCommand::CloseFocusedWindow)),
            "enter" => Some(PreviewKeyAction::Command(WmCommand::Spawn {
                command: "$openRandom".to_string(),
            })),
            "0" => Some(PreviewKeyAction::Command(WmCommand::SelectWorkspace {
                workspace_id: "10".into(),
            })),
            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" => {
                Some(PreviewKeyAction::Command(WmCommand::SelectWorkspace {
                    workspace_id: key_lower.as_str().into(),
                }))
            }
            _ => None,
        };
    }

    match key.as_str() {
        "[" => Some(PreviewKeyAction::Command(WmCommand::CycleLayout {
            direction: Some(LayoutCycleDirection::Previous),
        })),
        "]" => Some(PreviewKeyAction::Command(WmCommand::CycleLayout {
            direction: Some(LayoutCycleDirection::Next),
        })),
        "f" => Some(PreviewKeyAction::Command(WmCommand::ToggleFloating)),
        "Enter" => Some(PreviewKeyAction::Command(WmCommand::ToggleFullscreen)),
        "x" | "Backspace" | "Delete" => {
            Some(PreviewKeyAction::Command(WmCommand::CloseFocusedWindow))
        }
        _ => None,
    }
}

fn should_ignore_preview_key_event(event: &web_sys::KeyboardEvent) -> bool {
    let Some(target) = event.target() else {
        return false;
    };
    let Some(element) = target.dyn_ref::<web_sys::Element>() else {
        return false;
    };

    let tag = element.tag_name();
    if matches!(tag.as_str(), "INPUT" | "TEXTAREA" | "SELECT") {
        return true;
    }

    element
        .get_attribute("contenteditable")
        .as_deref()
        .is_some_and(|value| value == "" || value.eq_ignore_ascii_case("true"))
}
