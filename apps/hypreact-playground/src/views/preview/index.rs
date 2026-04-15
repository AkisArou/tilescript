use leptos::ev::KeyboardEvent;
use leptos::prelude::*;

use hypreact_core::command::{FocusDirection, LayoutCycleDirection, WmCommand};
use hypreact_core::snapshot::WindowSnapshot;

use crate::app_state::AppState;
use crate::session::{PreviewDiagnostic, PreviewSessionState};
use crate::views::editor::EditorView;
use crate::views::system::SystemView;

use super::scene::{body_style, frame_style, pane_style};

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

const TOGGLE_BUTTON_BASE: &str = "border px-2 py-0.5 transition-colors duration-150";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PreviewSurfaceMode {
    Preview,
    Editor,
    System,
    Binds,
}

#[component]
pub fn PreviewView() -> impl IntoView {
    let app_state = expect_context::<AppState>();
    let surface_mode = RwSignal::new(PreviewSurfaceMode::Preview);

    view! {
        <section class="grid h-full min-h-0 w-full min-w-0 grid-cols-1 gap-2">
            <div class="grid min-h-0 gap-2">
                <div class="border-terminal-border bg-terminal-bg-subtle flex min-h-0 flex-col overflow-hidden border">
                    <div
                        class="border-terminal-border bg-terminal-topbar grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-2 border-b pr-2 text-xs text-terminal-dim"
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
                            {move || {
                                match surface_mode.get() {
                                    PreviewSurfaceMode::Preview => focused_window_label(app_state),
                                    PreviewSurfaceMode::Editor => "editor://workspace".to_string(),
                                    PreviewSurfaceMode::System => "system://state".to_string(),
                                    PreviewSurfaceMode::Binds => "binds://hyprland".to_string(),
                                }
                            }}
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
                                    if surface_mode.get() == PreviewSurfaceMode::System {
                                        format!(
                                            "{TOGGLE_BUTTON_BASE} border-terminal-info bg-terminal-info/10 text-terminal-info"
                                        )
                                    } else {
                                        format!(
                                            "{TOGGLE_BUTTON_BASE} border-terminal-border bg-terminal-bg-subtle text-terminal-dim hover:text-terminal-fg"
                                        )
                                    }
                                }
                                on:click=move |_| surface_mode.set(PreviewSurfaceMode::System)
                            >
                                "System"
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
                            PreviewSurfaceMode::System => view! {
                                <OverlaySurface>
                                    <SystemView />
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
            <div class="absolute inset-[2.25rem] border border-terminal-border-strong bg-terminal-bg shadow-[0_20px_70px_rgba(0,0,0,0.55)]">
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
        BindsLine::Comment("# Move focus with mainMod + hjkl"),
        BindsLine::Blank,
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
                    "akisarou@spiders"
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
    let cpu_rows = [
        ("0", "[||||||||||||||||||      71.2%]", "71.2", "31"),
        ("1", "[||||||||||||||||        64.8%]", "64.8", "16"),
        ("2", "[|||||||||||||           51.1%]", "51.1", "08"),
        ("3", "[||||||||||              39.6%]", "39.6", "04"),
    ];
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
                    <span>"  0[ ||||||||||||||||||       71.2%]   Tasks: 142, 498 thr; 2 running"</span>
                    <span>"Load average: 1.48 1.31 1.12"</span>
                </div>
                <div class="grid gap-0.5">
                    {cpu_rows.into_iter().map(|(index, meter, value, avg)| {
                        view! {
                            <div class="grid grid-cols-[3rem_minmax(0,1fr)_4rem_3rem] gap-2">
                                <span class="text-[#7dd3fc]">{format!("CPU{index}")}</span>
                                <span class="text-[#8bd450]">{meter}</span>
                                <span class="text-right text-[#f4d35e]">{format!("{value}%")}</span>
                                <span class="text-right text-[#c792ea]">{avg}</span>
                            </div>
                        }
                    }).collect_view()}
                </div>
                <div class="grid grid-cols-2 gap-4 pt-1">
                    <div class="truncate">
                        <span class="text-[#7dd3fc]">"Mem"</span>
                        <span class="ml-2 text-[#8bd450]">"[|||||||||||||||||       6.42G/11.7G]"</span>
                    </div>
                    <div class="truncate">
                        <span class="text-[#7dd3fc]">"Swp"</span>
                        <span class="ml-2 text-[#f4d35e]">"[||                      256M/8.00G]"</span>
                    </div>
                </div>
                <div class="grid grid-cols-2 gap-4">
                    <div>
                        <span class="text-[#7dd3fc]">"Uptime"</span>
                        <span class="ml-2">"01:24:17"</span>
                    </div>
                    <div>
                        <span class="text-[#7dd3fc]">"Battery"</span>
                        <span class="ml-2 text-[#8bd450]">"93%"</span>
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
                    {process_rows.into_iter().enumerate().map(|(index, row)| {
                        let row_class = if index == 0 {
                            "bg-[#0d2d57] text-[#ffffff]"
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
                                <span>{row.8}</span>
                                <span>{row.9}</span>
                                <span>{row.10}</span>
                                <span class="truncate">{row.11}</span>
                            </div>
                        }
                    }).collect_view()}
                </div>
            </div>

            <div class="mt-2 grid grid-cols-5 gap-px bg-[#2d2d2d] text-[11px]">
                {footer_keys.into_iter().map(|(key, label)| {
                    view! {
                        <div class="bg-[#1a1a1a] px-1.5 py-0.5 text-[#d6d6d6]">
                            <span class="bg-[#3b82f6] px-1 text-black">{key}</span>
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
