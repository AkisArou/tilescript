use std::sync::{Arc, Mutex};

use leptos::ev::KeyboardEvent;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use hypreact_core::command::{FocusDirection, LayoutCycleDirection, WmCommand};
use hypreact_core::snapshot::WindowSnapshot;

use crate::app_state::AppState;
use crate::session::{
    PreviewDiagnostic, PreviewSessionState, focus_preview_window_by_direction,
    move_preview_window_by_direction, resize_preview_window_by_direction,
};
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

fn focused_window_chip_label(app_state: AppState) -> String {
    let snapshot = app_state.session.get();
    snapshot
        .focused_window_id()
        .as_ref()
        .and_then(|window_id| snapshot.model.windows.get(window_id))
        .map(|window| {
            let app_id = window.app_id.as_deref().unwrap_or("preview");
            app_id.to_string()
        })
        .unwrap_or_else(|| "preview".to_string())
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

fn has_error_diagnostics(session: &PreviewSessionState) -> bool {
    session.diagnostics.iter().any(|diagnostic| diagnostic.severity == "error")
}

const TOGGLE_BUTTON_BASE: &str = "border px-2 py-0.5 transition-colors duration-150";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PreviewSurfaceMode {
    Preview,
    Editor,
    Diagnostics,
    Binds,
}

#[component]
pub fn PreviewView() -> impl IntoView {
    let app_state = expect_context::<AppState>();
    let surface_mode = RwSignal::new(PreviewSurfaceMode::Preview);

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
                if surface_mode.get_untracked() != PreviewSurfaceMode::Preview {
                    return;
                }

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
                        PreviewKeyAction::FocusDirection(direction) => {
                            app_state.session.update(|state| {
                                focus_preview_window_by_direction(state, direction);
                            });
                        }
                        PreviewKeyAction::MoveDirection(direction) => {
                            app_state.session.update(|state| {
                                move_preview_window_by_direction(state, direction);
                            });
                        }
                        PreviewKeyAction::ResizeDirection(direction) => {
                            app_state.session.update(|state| {
                                resize_preview_window_by_direction(state, direction);
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
                        class="grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-2 bg-terminal-topbar pr-2 text-xs text-terminal-dim"
                    >
                        <div class="flex min-w-0 items-center gap-0 overflow-x-auto">
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
                                                            "{TOGGLE_BUTTON_BASE} border-transparent bg-[#2b2b2b] text-terminal-fg"
                                                        )
                                                    } else {
                                                        format!(
                                                            "{TOGGLE_BUTTON_BASE} border-transparent bg-transparent text-terminal-faint hover:text-terminal-dim"
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
                            <Show when=move || surface_mode.get() == PreviewSurfaceMode::Preview>
                                <span class="ml-2 min-w-0 truncate text-[12px] text-[#6f7277]">
                                    {move || focused_window_chip_label(app_state)}
                                </span>
                            </Show>
                        </div>

                        <div class="min-w-0 truncate px-2 text-center text-terminal-fg-strong">
                            {move || {
                                match surface_mode.get() {
                                    PreviewSurfaceMode::Preview => String::new(),
                                    PreviewSurfaceMode::Editor => "editor://workspace".to_string(),
                                    PreviewSurfaceMode::Diagnostics => {
                                        "diagnostics://journal".to_string()
                                    }
                                    PreviewSurfaceMode::Binds => "binds://hyprland".to_string(),
                                }
                            }}
                        </div>

                        <div class="flex items-center gap-2 justify-self-end">
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

                            <button
                                class=move || {
                                    if surface_mode.get() == PreviewSurfaceMode::Preview {
                                        format!(
                                            "{TOGGLE_BUTTON_BASE} border-terminal-info bg-terminal-info/10 text-terminal-info"
                                        )
                                    } else {
                                        format!(
                                            "{TOGGLE_BUTTON_BASE} border-terminal-border bg-terminal-bg-subtle text-terminal-dim hover:text-terminal-fg"
                                        )
                                    }
                                }
                                on:click=move |_| surface_mode.set(PreviewSurfaceMode::Preview)
                            >
                                "Preview"
                            </button>

                            <button
                                class=move || {
                                    if surface_mode.get() == PreviewSurfaceMode::Editor {
                                        format!(
                                            "{TOGGLE_BUTTON_BASE} border-terminal-info bg-terminal-info/10 text-terminal-info"
                                        )
                                    } else {
                                        format!(
                                            "{TOGGLE_BUTTON_BASE} border-terminal-border bg-terminal-bg-subtle text-terminal-dim hover:text-terminal-fg"
                                        )
                                    }
                                }
                                on:click=move |_| surface_mode.set(PreviewSurfaceMode::Editor)
                            >
                                "Editor"
                            </button>

                            <button
                                class=move || {
                                    let session = app_state.session.get();
                                    let has_errors = has_error_diagnostics(&session);
                                    if surface_mode.get() == PreviewSurfaceMode::Diagnostics {
                                        if has_errors {
                                            format!(
                                                "{TOGGLE_BUTTON_BASE} border-terminal-error bg-terminal-error/12 text-terminal-error"
                                            )
                                        } else {
                                            format!(
                                                "{TOGGLE_BUTTON_BASE} border-terminal-info bg-terminal-info/10 text-terminal-info"
                                            )
                                        }
                                    } else {
                                        if has_errors {
                                            format!(
                                                "{TOGGLE_BUTTON_BASE} border-terminal-error/50 bg-terminal-error/6 text-terminal-error hover:text-terminal-error"
                                            )
                                        } else {
                                            format!(
                                                "{TOGGLE_BUTTON_BASE} border-terminal-border bg-terminal-bg-subtle text-terminal-dim hover:text-terminal-fg"
                                            )
                                        }
                                    }
                                }
                                on:click=move |_| surface_mode.set(PreviewSurfaceMode::Diagnostics)
                            >
                                "Diagnostics"
                            </button>

                            <button
                                class=move || {
                                    if surface_mode.get() == PreviewSurfaceMode::Binds {
                                        format!(
                                            "{TOGGLE_BUTTON_BASE} border-terminal-info bg-terminal-info/10 text-terminal-info"
                                        )
                                    } else {
                                        format!(
                                            "{TOGGLE_BUTTON_BASE} border-terminal-border bg-terminal-bg-subtle text-terminal-dim hover:text-terminal-fg"
                                        )
                                    }
                                }
                                on:click=move |_| surface_mode.set(PreviewSurfaceMode::Binds)
                            >
                                "Binds"
                            </button>
                        </div>
                    </div>

                    <div class="min-h-0 flex-1 overflow-hidden">
                        {move || match surface_mode.get() {
                            PreviewSurfaceMode::Preview => {
                                view! {
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
                                                    <div
                                                        class="bg-terminal-bg relative h-full min-h-72 w-full overflow-hidden"
                                                        style="background-image: linear-gradient(rgba(10, 10, 10, 0.55), rgba(10, 10, 10, 0.72)), url('/assets/hyprland-wallpaper.png'); background-position: center; background-size: cover;"
                                                    >
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
                                                                let app_id = window.app_id.as_deref();
                                                                let is_foot = app_id == Some("foot");
                                                                let is_htop = app_id == Some("htop");
                                                                let is_nvim = app_id == Some("nvim");
                                                                let foot_focus_window_id = window.id.clone();

                                                                Some(view! {
                                                                    <div
                                                                        class="preview-window text-terminal-fg absolute z-20 cursor-pointer overflow-hidden text-left text-xs"
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
                                                                            } else {
                                                                                view! { <WindowSurface window=window.clone() /> }.into_any()
                                                                            }}
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
                                }
                                    .into_any()
                            }
                            PreviewSurfaceMode::Editor => view! {
                                <OverlaySurface>
                                    <EditorView />
                                </OverlaySurface>
                            }
                                .into_any(),
                            PreviewSurfaceMode::Diagnostics => view! {
                                <OverlaySurface>
                                    <DiagnosticsWindow diagnostics=Signal::derive(move || {
                                        app_state.session.get().diagnostics.clone()
                                    }) />
                                </OverlaySurface>
                            }
                                .into_any(),
                            PreviewSurfaceMode::Binds => view! {
                                <OverlaySurface>
                                    <BindsWindow />
                                </OverlaySurface>
                            }
                                .into_any(),
                        }}
                    </div>
                </div>
            </div>
        </section>
    }
    .into_any()
}

#[component]
fn OverlaySurface(children: Children) -> impl IntoView {
    view! {
        <div
            class="relative h-full min-h-72 w-full overflow-hidden bg-terminal-bg"
            style="background-image: linear-gradient(rgba(10, 10, 10, 0.4), rgba(10, 10, 10, 0.58)), url('/assets/hyprland-wallpaper.png'); background-position: center; background-size: cover;"
        >
            <div class="preview-layer absolute inset-[2.25rem] border border-terminal-border-strong bg-terminal-bg shadow-[0_20px_70px_rgba(0,0,0,0.55)]">
                <div class="h-full overflow-hidden">{children()}</div>
            </div>
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
fn DiagnosticsWindow(#[prop(into)] diagnostics: Signal<Vec<PreviewDiagnostic>>) -> impl IntoView {
    view! {
        <div class="h-full w-full overflow-auto bg-[#111317] px-3 py-3 font-mono text-[0.8rem] leading-[1.45] text-[#c9d1d9]">
            <Show
                when=move || !diagnostics.get().is_empty()
                fallback=move || {
                    view! {
                        <div class="text-[#8b949e]">
                            "Apr 15 18:24:03 hypreact-playground diagnostics[913]: -- No entries --"
                        </div>
                    }
                }
            >
                <div class="grid auto-rows-min gap-1">
                    {move || {
                        diagnostics
                            .get()
                            .into_iter()
                            .enumerate()
                            .map(|(index, diagnostic)| {
                                let timestamp = format!("Apr 15 18:24:{:02}", (index + 3) % 60);
                                let severity_color = if diagnostic.severity == "error" {
                                    "text-[#ff7b72]"
                                } else {
                                    "text-[#d29922]"
                                };

                                view! {
                                    <div class="border-l-2 border-[#2d333b] pl-3">
                                        <div class="text-[#8b949e]">
                                            {timestamp}
                                            <span class="ml-2 text-[#6cb6ff]">"hypreact-playground"</span>
                                            <span class="ml-1 text-[#8b949e]">"diagnostics[913]:"</span>
                                            <span class=format!("ml-2 {}", severity_color)>{format!("{} {}", diagnostic.severity, diagnostic.code)}</span>
                                        </div>
                                        <div class="mt-0.5 text-[#c9d1d9]">
                                            <span class="text-[#8b949e]">{diagnostic.path.clone()}</span>
                                            <span class="text-[#6e7681]">": "</span>
                                            <span>{diagnostic.message.clone()}</span>
                                        </div>
                                        <div class="mt-0.5 text-[#6e7681]">{diagnostic.range.clone()}</div>
                                    </div>
                                }
                            })
                            .collect_view()
                    }}
                </div>
            </Show>
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
    FocusDirection(FocusDirection),
    MoveDirection(FocusDirection),
    ResizeDirection(FocusDirection),
}

fn preview_command_from_key(event: &KeyboardEvent) -> Option<PreviewKeyAction> {
    let key = event.key();
    let key_lower = key.to_ascii_lowercase();

    if event.alt_key() {
        if event.shift_key() {
            return match key_lower.as_str() {
                "h" => Some(PreviewKeyAction::MoveDirection(FocusDirection::Left)),
                "j" => Some(PreviewKeyAction::MoveDirection(FocusDirection::Down)),
                "k" => Some(PreviewKeyAction::MoveDirection(FocusDirection::Up)),
                "l" => Some(PreviewKeyAction::MoveDirection(FocusDirection::Right)),
                _ => None,
            };
        }

        if event.ctrl_key() {
            return match key_lower.as_str() {
                "h" => Some(PreviewKeyAction::ResizeDirection(FocusDirection::Left)),
                "j" => Some(PreviewKeyAction::ResizeDirection(FocusDirection::Down)),
                "k" => Some(PreviewKeyAction::ResizeDirection(FocusDirection::Up)),
                "l" => Some(PreviewKeyAction::ResizeDirection(FocusDirection::Right)),
                _ => None,
            };
        }

        return match key_lower.as_str() {
            "h" => Some(PreviewKeyAction::FocusDirection(FocusDirection::Left)),
            "j" => Some(PreviewKeyAction::FocusDirection(FocusDirection::Down)),
            "k" => Some(PreviewKeyAction::FocusDirection(FocusDirection::Up)),
            "l" => Some(PreviewKeyAction::FocusDirection(FocusDirection::Right)),
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
