use std::sync::{Arc, Mutex};

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use hypreact_core::snapshot::WindowSnapshot;
use hypreact_scene::ComputedStyle;

use crate::app_state::AppState;
use crate::session::PreviewSessionState;
use crate::views::editor::EditorView;

use super::index::BindsWindow;
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
    const PALETTE: [&str; 8] = [
        "var(--color-preview-accent-cyan)",
        "var(--color-preview-accent-orange)",
        "var(--color-preview-accent-green)",
        "var(--color-preview-accent-gold)",
        "var(--color-preview-accent-indigo)",
        "var(--color-preview-accent-teal)",
        "var(--color-preview-accent-pink)",
        "var(--color-preview-accent-rose)",
    ];

    let seed = window.title.as_deref().or(window.app_id.as_deref()).unwrap_or(window.id.as_str());
    let hash = seed
        .bytes()
        .chain(window.id.as_str().bytes())
        .fold(0usize, |acc, byte| acc + byte as usize);

    PALETTE[hash % PALETTE.len()].to_string()
}

fn preview_canvas_rect(session: &PreviewSessionState) -> hypreact_core::LayoutRect {
    let layout_space = session
        .model
        .current_workspace_id()
        .and_then(|workspace_id| session.model.workspaces.get(workspace_id))
        .and_then(|workspace| workspace.layout_space)
        .unwrap_or(hypreact_core::wm::DrawableSpace { width: 1240, height: 760 });

    hypreact_core::LayoutRect {
        x: 0.0,
        y: 0.0,
        width: layout_space.width as f32,
        height: layout_space.height as f32,
    }
}

fn fullscreen_window_render_state(
    session: &PreviewSessionState,
) -> Option<PreviewWindowRenderState> {
    let fullscreen_window =
        session.visible_windows().into_iter().find(|window| window.mode.is_fullscreen())?;

    Some(PreviewWindowRenderState {
        accent: window_accent(&fullscreen_window),
        rect: preview_canvas_rect(session),
        layout_style: None,
        window: fullscreen_window,
    })
}

#[derive(Clone)]
pub(super) struct PreviewWindowRenderState {
    pub(super) window: WindowSnapshot,
    pub(super) rect: hypreact_core::LayoutRect,
    pub(super) layout_style: Option<ComputedStyle>,
    pub(super) accent: String,
}

pub(super) fn preview_window_render_states(
    session: &PreviewSessionState,
) -> Vec<PreviewWindowRenderState> {
    if let Some(fullscreen_state) = fullscreen_window_render_state(session) {
        return vec![fullscreen_state];
    }

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

pub(super) fn preview_window_render_state_for_id(
    session: &PreviewSessionState,
    window_id: &hypreact_core::WindowId,
) -> Option<PreviewWindowRenderState> {
    if let Some(fullscreen_state) = fullscreen_window_render_state(session) {
        return (fullscreen_state.window.id == *window_id).then_some(fullscreen_state);
    }

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

#[component]
pub(super) fn PreviewSceneSurface() -> impl IntoView {
    let app_state = expect_context::<AppState>();

    view! {
        <div
            class="bg-terminal-bg relative h-full min-h-72 w-full overflow-hidden"
            style="background-image: linear-gradient(var(--color-preview-wallpaper-overlay-top), var(--color-preview-wallpaper-overlay-bottom)), url('/assets/hyprland-wallpaper.png'); background-position: center; background-size: cover;"
        >
            <Show when=move || preview_window_render_states(&app_state.session.get()).is_empty()>
                <div class="text-terminal-faint/80 pointer-events-none absolute inset-x-0 bottom-[20%] z-10 flex justify-center text-sm tracking-[0.08em]">
                    "Press Alt+Enter to open a window"
                </div>
            </Show>
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
    let frame_ref = NodeRef::<leptos::html::Div>::new();
    let window_id = state.window.id.clone();
    let effect_window_id = state.window.id.clone();
    let pane_focus_target = state.window.id.clone();
    let is_foot = state.window.app_id.as_deref() == Some("foot");
    let is_htop = state.window.app_id.as_deref() == Some("htop");
    let is_nvim = state.window.app_id.as_deref() == Some("nvim");
    let is_playground_editor = state.window.app_id.as_deref() == Some("playground-editor");
    let is_hyprland_config = state.window.app_id.as_deref() == Some("hyprland-config");
    let foot_focus_window_id = state.window.id.clone();
    let focused_class_id = state.window.id.clone();
    let focused_attr_id = state.window.id.clone();
    let focused_style_id = state.window.id.clone();
    let previous_rect = StoredValue::new(None::<hypreact_core::LayoutRect>);

    Effect::new(move |_| {
        let animations_enabled = app_state.preview_animations_enabled.get();
        let next_rect = app_state.session.with(|session| {
            preview_window_render_state_for_id(session, &effect_window_id).map(|state| state.rect)
        });

        let Some(next_rect) = next_rect else {
            previous_rect.set_value(None);
            return;
        };

        let Some(element) = frame_ref.get() else {
            previous_rect.set_value(Some(next_rect));
            return;
        };
        let Ok(element): Result<web_sys::HtmlElement, _> = element.dyn_into() else {
            previous_rect.set_value(Some(next_rect));
            return;
        };

        let prior_rect = previous_rect.get_value();
        previous_rect.set_value(Some(next_rect));

        if !animations_enabled {
            let _ = element.style().set_property("transform", "translate(0%, 0%)");
            return;
        }

        let Some(prior_rect) = prior_rect else {
            let _ = element.style().set_property("transform", "translate(0%, 0%)");
            return;
        };

        let dx = prior_rect.x - next_rect.x;
        let dy = prior_rect.y - next_rect.y;

        let _ = element.style().set_property("transition", "none");
        let _ = element.style().set_property("transform", &format!("translate({dx}px, {dy}px)"));
        let _ = element.offset_width();

        let element_for_frame = element.clone();
        let reset_transform = Closure::once_into_js(move || {
            let _ = element_for_frame.style().remove_property("transition");
            let _ = element_for_frame.style().set_property("transform", "translate(0%, 0%)");
        });

        if let Some(window) = web_sys::window() {
            let _ = window.request_animation_frame(reset_transform.unchecked_ref());
        }
    });

    view! {
        <div
            node_ref=frame_ref
            class=move || {
                let focused = app_state.session.get().focused_window_id().as_ref() == Some(&focused_class_id);
                if app_state.preview_animations_enabled.get() {
                    if focused {
                        "preview-window text-terminal-fg absolute z-30 cursor-pointer overflow-hidden text-left text-xs"
                    } else {
                        "preview-window text-terminal-fg absolute z-20 cursor-pointer overflow-hidden text-left text-xs"
                    }
                } else {
                    if focused {
                        "preview-window preview-window--static text-terminal-fg absolute z-30 cursor-pointer overflow-hidden text-left text-xs"
                    } else {
                        "preview-window preview-window--static text-terminal-fg absolute z-20 cursor-pointer overflow-hidden text-left text-xs"
                    }
                }
            }
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
                    pane_style(
                        render_state.rect,
                        &render_state.accent,
                        1240,
                        760,
                        app_state.preview_animations_enabled.get(),
                    ),
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
                } else if is_hyprland_config {
                    view! { <BindsWindow /> }.into_any()
                } else {
                    view! { <WindowSurface window=state.window.clone() /> }.into_any()
                }}
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
            "nvim apps/hypreact-playground/src/views/preview/windows.rs",
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
        <div class="text-[var(--color-preview-htop-fg)] flex h-full w-full flex-col bg-[var(--color-preview-htop-bg)] p-3 text-xs leading-5">
            <div class="grid gap-1">
                <div class="text-[var(--color-preview-htop-green)] flex items-center justify-between">
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
                                <span class="text-[var(--color-preview-htop-cyan)]">{format!("CPU{index}")}</span>
                                <span class="text-[var(--color-preview-htop-green)]">{move || htop_meter(metrics.get().cpu_usage(index), 18)}</span>
                                <span class="text-[var(--color-preview-htop-gold)] text-right">{move || format!("{:.1}%", metrics.get().cpu_usage(index))}</span>
                                <span class="text-[var(--color-preview-htop-purple)] text-right">{move || format!("{:02}", metrics.get().cpu_avg(index))}</span>
                            </div>
                        }
                    }).collect_view()}
                </div>
                <div class="grid grid-cols-2 gap-4 pt-1">
                    <div class="truncate">
                        <span class="text-[var(--color-preview-htop-cyan)]">"Mem"</span>
                        <span class="text-[var(--color-preview-htop-green)] ml-2">{move || htop_memory_meter(metrics.get().memory_gb(), 11.7, 17)}</span>
                    </div>
                    <div class="truncate">
                        <span class="text-[var(--color-preview-htop-cyan)]">"Swp"</span>
                        <span class="text-[var(--color-preview-htop-gold)] ml-2">{move || htop_swap_meter(metrics.get().swap_mb(), 8.0, 22)}</span>
                    </div>
                </div>
                <div class="grid grid-cols-2 gap-4">
                    <div>
                        <span class="text-[var(--color-preview-htop-cyan)]">"Uptime"</span>
                        <span class="ml-2">"01:24:17"</span>
                    </div>
                    <div>
                        <span class="text-[var(--color-preview-htop-cyan)]">"Battery"</span>
                        <span class="text-[var(--color-preview-htop-green)] ml-2">{move || format!("{}%", metrics.get().battery())}</span>
                    </div>
                </div>
            </div>

            <div class="border-[var(--color-preview-htop-border)] mt-3 min-h-0 flex-1 overflow-hidden border">
                <div class="text-[var(--color-preview-htop-header-fg)] bg-[var(--color-preview-htop-header)] border-[var(--color-preview-htop-border)] grid grid-cols-[4rem_6rem_3rem_3rem_4rem_4rem_4rem_3rem_4rem_4rem_7rem_minmax(0,1fr)] gap-2 border-b px-2 py-1 text-xs uppercase tracking-[0.14em]">
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
                <div class="bg-[var(--color-preview-htop-border)] grid auto-rows-min gap-px">
                    {move || {
                        let order = metrics.get().process_order();
                        order.into_iter().enumerate().map(|(index, row_index)| {
                        let row = process_rows[row_index];
                        let row_class = if index == 0 {
                            "bg-[var(--color-preview-htop-row-active)] text-[var(--color-preview-htop-row-active-fg)]"
                        } else {
                            "bg-[var(--color-preview-htop-bg)] text-[var(--color-preview-htop-fg)]"
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

            <div class="mt-2 grid grid-cols-5 gap-1 text-xs">
                {footer_keys.into_iter().map(|(key, label)| {
                    view! {
                        <div class="text-[var(--color-preview-htop-fg)] px-1.5 py-0.5">
                            <span class="text-[var(--color-preview-htop-gold)] px-1">{key}</span>
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
        ("Nvim v0.13.0-dev-172+g7bb8231577", "text-[var(--color-preview-nvim-rose)]"),
        ("", "text-[var(--color-preview-nvim-fg)]"),
        ("Nvim is open source and freely distributable", "text-[var(--color-preview-nvim-fg)]"),
        ("https://neovim.io/#chat", "text-[var(--color-preview-nvim-fg)]"),
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
        ("███╗   ██╗", "text-[var(--color-preview-nvim-gold)]"),
        ("████╗  ██║", "text-[var(--color-preview-nvim-gold)]"),
        ("██╔██╗ ██║", "text-[var(--color-preview-nvim-purple)]"),
        ("██║╚██╗██║", "text-[var(--color-preview-nvim-rose)]"),
        ("██║ ╚████║", "text-[var(--color-preview-nvim-rose)]"),
        ("╚═╝  ╚═══╝", "text-[var(--color-preview-nvim-rose)]"),
    ];

    view! {
        <div class="text-[var(--color-preview-nvim-fg)] flex h-full w-full flex-col bg-[var(--color-preview-nvim-bg)] text-xs">
            <div class="text-[var(--color-preview-nvim-muted)] flex items-start gap-2 px-3 pt-1 text-xs">
                <span>"1"</span>
                <div class="bg-[var(--color-preview-nvim-track)] relative mt-0.5 h-3.5 flex-1">
                    <span class="bg-[var(--color-preview-nvim-cursor)] absolute left-0 top-0 inline-block h-3.5 w-1.5" />
                </div>
            </div>

            <div class="flex min-h-0 flex-1 items-center justify-center px-6 pb-4 pt-4">
                <div class="grid w-full max-w-72 gap-1 text-center text-xs leading-4">
                    <div class="grid justify-center gap-0 text-sm leading-none">
                        {logo_lines
                            .into_iter()
                            .map(|(line, class_name)| view! { <div class=class_name>{line}</div> })
                            .collect_view()}
                    </div>

                    <div class="bg-[var(--color-preview-nvim-rule)] mx-auto h-px w-60" />

                    <div class="grid gap-0 text-xs leading-4">
                        {intro_lines
                            .into_iter()
                            .map(|(line, class_name)| view! { <div class=class_name>{line}</div> })
                            .collect_view()}
                    </div>

                    <div class="bg-[var(--color-preview-nvim-rule)] mx-auto h-px w-60" />

                    <div class="grid justify-center gap-0 text-left text-xs leading-4">
                        {help_lines
                            .into_iter()
                            .map(|(command, description)| {
                                view! {
                                    <div class="grid grid-cols-[2.1rem_7.7rem_minmax(0,1fr)] gap-1.5">
                                        <span>"type"</span>
                                        <span class="text-[var(--color-preview-nvim-blue)]">{command}</span>
                                        <span>{description}</span>
                                    </div>
                                }
                            })
                            .collect_view()}
                    </div>

                    <div class="bg-[var(--color-preview-nvim-rule)] mx-auto h-px w-60" />

                    <div class="grid justify-center gap-0 text-left text-xs leading-4">
                        <div class="grid grid-cols-[2.1rem_7.7rem_minmax(0,1fr)] gap-1.5">
                            <span>"type"</span>
                            <span class="text-[var(--color-preview-nvim-blue)]">{news_line.0}</span>
                            <span>{news_line.1}</span>
                        </div>
                    </div>

                    <div class="bg-[var(--color-preview-nvim-rule)] mx-auto h-px w-60" />

                    <div class="grid gap-0 text-xs leading-4">
                        <div>"Help poor children in Uganda!"</div>
                        <div class="grid justify-center text-left">
                            <div class="grid grid-cols-[2.1rem_7.7rem_minmax(0,1fr)] gap-1.5">
                                <span>"type"</span>
                                <span class="text-[var(--color-preview-nvim-blue)]">{donate_line.0}</span>
                                <span>{donate_line.1}</span>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            <div class="text-[var(--color-preview-nvim-muted)] mt-auto flex items-center justify-between px-3 pb-1.5 pt-1 text-xs">
                <span>"[No Name]"</span>
                <div class="flex items-center gap-4">
                    <span>"Top"</span>
                    <span>"1:1"</span>
                </div>
            </div>
        </div>
    }
}
