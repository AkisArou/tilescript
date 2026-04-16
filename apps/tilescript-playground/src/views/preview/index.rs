use leptos::ev::KeyboardEvent;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use tilescript_core::command::{FocusDirection, LayoutCycleDirection, WmCommand};

use crate::app_state::AppState;
use crate::session::PreviewSessionState;

use super::windows::PreviewSceneSurface;

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

fn preview_diagnostics_counts(session: &PreviewSessionState) -> (usize, usize) {
    let mut error_count = 0;
    let mut warning_count = 0;

    for diagnostic in &session.diagnostics {
        if diagnostic.severity == "error" {
            error_count += 1;
        } else {
            warning_count += 1;
        }
    }

    if error_count == 0 && warning_count == 0 && session.error.is_some() {
        error_count = 1;
    }

    (error_count, warning_count)
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
                    <div class="border-terminal-border grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 border-b bg-terminal-topbar pr-2 text-xs text-terminal-dim">
                        <PreviewWorkspaceTabs />
                        <div class="min-w-0" />
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
                                        "border border-transparent bg-terminal-bg-hover px-2 py-0.5 text-terminal-fg transition-colors duration-150"
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
fn PreviewToolbar() -> impl IntoView {
    let app_state = expect_context::<AppState>();

    view! {
        <div class="flex items-center justify-self-end gap-2 py-1">
            <Show when=move || preview_diagnostics_summary(&app_state.session.get()).is_some()>
                <PreviewDiagnosticsWidget />
            </Show>
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
            <a
                class="border-terminal-border bg-terminal-bg-subtle text-terminal-faint hover:text-terminal-fg flex h-4 items-center justify-center gap-1 border px-1 transition-colors duration-150"
                href="https://github.com/AkisArou/tilescript"
                target="_blank"
                rel="noreferrer"
                title="Open GitHub repository"
            >
                <svg aria-hidden="true" viewBox="0 0 24 24" class="h-3.5 w-3.5 fill-current">
                    <path d="M12 1.25C6.062 1.25 1.25 6.153 1.25 12.2c0 4.838 3.075 8.942 7.342 10.39.537.102.733-.238.733-.529 0-.262-.01-.956-.015-1.876-2.987.665-3.618-1.474-3.618-1.474-.489-1.268-1.194-1.606-1.194-1.606-.976-.683.074-.669.074-.669 1.08.078 1.648 1.134 1.648 1.134.96 1.684 2.518 1.198 3.131.916.097-.713.376-1.199.684-1.474-2.384-.279-4.892-1.213-4.892-5.401 0-1.193.417-2.169 1.1-2.933-.11-.28-.477-1.404.105-2.926 0 0 .898-.294 2.943 1.12a10.06 10.06 0 0 1 5.36 0c2.043-1.414 2.94-1.12 2.94-1.12.584 1.522.217 2.646.107 2.926.685.764 1.099 1.74 1.099 2.933 0 4.2-2.512 5.119-4.903 5.392.386.34.73 1.01.73 2.036 0 1.469-.013 2.655-.013 3.016 0 .294.193.636.74.528 4.264-1.45 7.336-5.552 7.336-10.389 0-6.047-4.813-10.95-10.75-10.95Z" />
                </svg>
                <p>Tilescript</p>
            </a>
        </div>
    }
}

#[component]
fn PreviewDiagnosticsWidget() -> impl IntoView {
    let app_state = expect_context::<AppState>();
    let error_count =
        Signal::derive(move || preview_diagnostics_counts(&app_state.session.get()).0);
    let warning_count =
        Signal::derive(move || preview_diagnostics_counts(&app_state.session.get()).1);

    view! {
        <div
            class="border-terminal-border bg-terminal-bg-subtle flex w-36 min-w-36 items-center gap-2 border px-2 py-0.5"
            title=move || preview_diagnostics_summary(&app_state.session.get()).unwrap_or_default()
        >
            <div class="flex shrink-0 items-center gap-1">
                <Show when=move || { error_count.get() > 0 }>
                    <span class="flex items-center gap-1 text-terminal-error">
                        <span class="inline-flex h-3.5 w-3.5 items-center justify-center rounded-full border border-current text-[0.58rem] font-bold leading-none">"!"</span>
                        <span class="text-xs leading-none">{move || error_count.get()}</span>
                    </span>
                </Show>
                <Show when=move || { warning_count.get() > 0 }>
                    <span class="flex items-center gap-1 text-terminal-warn">
                        <span class="inline-flex h-3.5 w-3.5 items-center justify-center text-[0.7rem] leading-none">"▲"</span>
                        <span class="text-xs leading-none">{move || warning_count.get()}</span>
                    </span>
                </Show>
            </div>
            <span
                class="min-w-0 truncate"
                class:text-terminal-error=move || preview_diagnostics_tone(&app_state.session.get()) == "error"
                class:text-terminal-warn=move || preview_diagnostics_tone(&app_state.session.get()) == "warning"
                class:text-terminal-faint=move || preview_diagnostics_tone(&app_state.session.get()) == "idle"
            >
                {move || preview_diagnostics_summary(&app_state.session.get()).unwrap_or_default()}
            </span>
        </div>
    }
}

#[component]
pub(super) fn BindsWindow() -> impl IntoView {
    #[derive(Clone, Copy)]
    enum BindsLine<'a> {
        Blank,
        Comment(&'a str),
        Var(&'a str),
        Bind { mods: &'a str, key: &'a str, command: &'a str, arg: &'a str },
    }

    let app_state = expect_context::<AppState>();

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
        BindsLine::Bind { mods: "$mainMod", key: "F", command: "fullscreen", arg: "0" },
        BindsLine::Blank,
        BindsLine::Comment("# Move focus with mainMod + hjkl"),
        BindsLine::Bind { mods: "$mainMod", key: "H", command: "tilescript:movefocus", arg: "left" },
        BindsLine::Bind { mods: "$mainMod", key: "L", command: "tilescript:movefocus", arg: "right" },
        BindsLine::Bind { mods: "$mainMod", key: "K", command: "tilescript:movefocus", arg: "up" },
        BindsLine::Bind { mods: "$mainMod", key: "J", command: "tilescript:movefocus", arg: "down" },
        BindsLine::Blank,
        BindsLine::Comment("# Move focused window with mainMod + Shift + hjkl"),
        BindsLine::Bind {
            mods: "$mainMod SHIFT",
            key: "H",
            command: "tilescript:movewindow",
            arg: "left",
        },
        BindsLine::Bind {
            mods: "$mainMod SHIFT",
            key: "L",
            command: "tilescript:movewindow",
            arg: "right",
        },
        BindsLine::Bind {
            mods: "$mainMod SHIFT",
            key: "K",
            command: "tilescript:movewindow",
            arg: "up",
        },
        BindsLine::Bind {
            mods: "$mainMod SHIFT",
            key: "J",
            command: "tilescript:movewindow",
            arg: "down",
        },
        BindsLine::Blank,
        BindsLine::Comment("# Resize focused window with mainMod + Control + hjkl"),
        BindsLine::Bind {
            mods: "$mainMod CONTROL",
            key: "H",
            command: "tilescript:resizewindow",
            arg: "left",
        },
        BindsLine::Bind {
            mods: "$mainMod CONTROL",
            key: "L",
            command: "tilescript:resizewindow",
            arg: "right",
        },
        BindsLine::Bind {
            mods: "$mainMod CONTROL",
            key: "K",
            command: "tilescript:resizewindow",
            arg: "up",
        },
        BindsLine::Bind {
            mods: "$mainMod CONTROL",
            key: "J",
            command: "tilescript:resizewindow",
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
        BindsLine::Blank,
        BindsLine::Comment("# Move active window to a workspace with mainMod + SHIFT + [0-9]"),
        BindsLine::Bind { mods: "$mainMod SHIFT", key: "1", command: "movetoworkspace", arg: "1" },
        BindsLine::Bind { mods: "$mainMod SHIFT", key: "2", command: "movetoworkspace", arg: "2" },
        BindsLine::Bind { mods: "$mainMod SHIFT", key: "3", command: "movetoworkspace", arg: "3" },
        BindsLine::Bind { mods: "$mainMod SHIFT", key: "4", command: "movetoworkspace", arg: "4" },
        BindsLine::Bind { mods: "$mainMod SHIFT", key: "5", command: "movetoworkspace", arg: "5" },
        BindsLine::Bind { mods: "$mainMod SHIFT", key: "6", command: "movetoworkspace", arg: "6" },
        BindsLine::Bind { mods: "$mainMod SHIFT", key: "7", command: "movetoworkspace", arg: "7" },
        BindsLine::Bind { mods: "$mainMod SHIFT", key: "8", command: "movetoworkspace", arg: "8" },
        BindsLine::Bind { mods: "$mainMod SHIFT", key: "9", command: "movetoworkspace", arg: "9" },
        BindsLine::Bind { mods: "$mainMod SHIFT", key: "0", command: "movetoworkspace", arg: "10" },
    ];

    view! {
        <div class="h-full w-full overflow-auto bg-[var(--color-preview-config-bg)] px-2 py-2 font-mono text-[0.8125rem] leading-6 text-[var(--color-preview-config-fg)]">
            <div class="grid auto-rows-min gap-0">
                <div class="grid grid-cols-[2.25rem_minmax(0,1fr)] gap-2">
                    <div class="pr-2 text-right text-[var(--color-preview-config-line)]">1</div>
                    <div class="text-[var(--color-preview-config-comment)]">"#~/.config/hypr/hyprland.conf"</div>
                </div>
                <div class="grid grid-cols-[2.25rem_minmax(0,1fr)] gap-2">
                    <div class="pr-2 text-right text-[var(--color-preview-config-line)]">2</div>
                    <div class="h-2.5" />
                </div>
                <div class="grid grid-cols-[2.25rem_minmax(0,1fr)] gap-2">
                    <div class="pr-2 text-right text-[var(--color-preview-config-line)]">3</div>
                    <div class="text-[var(--color-preview-config-accent)]">"animations {"</div>
                </div>
                <div class="grid grid-cols-[2.25rem_minmax(0,1fr)] gap-2">
                    <div class="pr-2 text-right text-[var(--color-preview-config-line)]">4</div>
                    <div class="flex items-center gap-2 pl-4">
                        <span>
                            <span class="text-[var(--color-preview-config-keyword)]">"enabled"</span>
                            <span class="text-[var(--color-preview-config-fg)]">" = "</span>
                        </span>
                        <div class="ui-select-wrap">
                            <select
                                class="ui-select min-w-36"
                                prop:value=move || {
                                    if app_state.preview_animations_enabled.get() {
                                        "yes, please :)"
                                    } else {
                                        "no"
                                    }
                                }
                                on:click=|event| event.stop_propagation()
                                on:mousedown=|event| event.stop_propagation()
                                on:change=move |event| {
                                    app_state.set_preview_animations_enabled(
                                        event_target_value(&event) == "yes, please :)"
                                    );
                                }
                            >
                                <option value="yes, please :)">"yes, please :)"</option>
                                <option value="no">"no"</option>
                            </select>
                        </div>
                    </div>
                </div>
                <div class="grid grid-cols-[2.25rem_minmax(0,1fr)] gap-2">
                    <div class="pr-2 text-right text-[var(--color-preview-config-line)]">5</div>
                    <div class="text-[var(--color-preview-config-accent)]">"}"</div>
                </div>
                <div class="grid grid-cols-[2.25rem_minmax(0,1fr)] gap-2">
                    <div class="pr-2 text-right text-[var(--color-preview-config-line)]">6</div>
                    <div class="h-2.5" />
                </div>
                {lines
                    .iter()
                    .enumerate()
                    .map(|(index, line)| {
                        let line_number = index + 7;
                        match *line {
                            BindsLine::Blank => view! {
                                <div class="grid grid-cols-[2.25rem_minmax(0,1fr)] gap-2">
                                    <div class="pr-2 text-right text-[var(--color-preview-config-line)]">{line_number}</div>
                                    <div class="h-2.5" />
                                </div>
                            }
                                .into_any(),
                            BindsLine::Comment(text) => view! {
                                <div class="grid grid-cols-[2.25rem_minmax(0,1fr)] gap-2">
                                    <div class="pr-2 text-right text-[var(--color-preview-config-line)]">{line_number}</div>
                                    <div class="text-[var(--color-preview-config-comment)]">{text.to_string()}</div>
                                </div>
                            }
                                .into_any(),
                            BindsLine::Var(left) => view! {
                                <div class="grid grid-cols-[2.25rem_minmax(0,1fr)] gap-2">
                                    <div class="pr-2 text-right text-[var(--color-preview-config-line)]">{line_number}</div>
                                    <div>
                                        <span class="text-[var(--color-preview-config-variable)]">{left.to_string()}</span>
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
                                    <div class="pr-2 text-right text-[var(--color-preview-config-line)]">{line_number}</div>
                                    <div>
                                        <span class="text-[var(--color-preview-config-keyword)]">"bind"</span>
                                        <span class="text-[var(--color-preview-config-fg)]">" = "</span>
                                        <span class="text-[var(--color-preview-config-mods)]">{mods.to_string()}</span>
                                        <span class="text-[var(--color-preview-config-fg)]">", "</span>
                                        <span class="text-[var(--color-preview-config-strong)]">{key.to_string()}</span>
                                        <span class="text-[var(--color-preview-config-fg)]">", "</span>
                                        <span class="text-[var(--color-preview-config-value)]">{command.to_string()}</span>
                                        <span class="text-[var(--color-preview-config-fg)]">", "</span>
                                        <span class="text-[var(--color-preview-config-value)]">{arg.to_string()}</span>
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

enum PreviewKeyAction {
    Command(WmCommand),
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardedPreviewKey {
    key: String,
    code: String,
    alt_key: bool,
    ctrl_key: bool,
    shift_key: bool,
}

pub fn handle_forwarded_preview_key(app_state: AppState, payload: &str) -> bool {
    let Ok(event) = serde_json::from_str::<ForwardedPreviewKey>(payload) else {
        return false;
    };

    let action = preview_command_from_parts(
        &event.key,
        &event.code,
        event.alt_key,
        event.ctrl_key,
        event.shift_key,
    );
    let Some(PreviewKeyAction::Command(command)) = action else {
        return false;
    };

    app_state.session.update(|state| state.apply_command(command, None));
    app_state.request_preview_reevaluation();
    true
}

fn preview_command_from_parts(
    key: &str,
    code: &str,
    alt_key: bool,
    ctrl_key: bool,
    shift_key: bool,
) -> Option<PreviewKeyAction> {
    let key_lower = key.to_ascii_lowercase();
    let digit_workspace = match code {
        "Digit1" => Some(1),
        "Digit2" => Some(2),
        "Digit3" => Some(3),
        "Digit4" => Some(4),
        "Digit5" => Some(5),
        "Digit6" => Some(6),
        "Digit7" => Some(7),
        "Digit8" => Some(8),
        "Digit9" => Some(9),
        "Digit0" => Some(10),
        _ => None,
    };

    if alt_key {
        if shift_key {
            if let Some(workspace) = digit_workspace {
                return Some(PreviewKeyAction::Command(
                    WmCommand::AssignFocusedWindowToWorkspace { workspace },
                ));
            }

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

        if ctrl_key {
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

        if let Some(workspace) = digit_workspace {
            return Some(PreviewKeyAction::Command(WmCommand::SelectWorkspace {
                workspace_id: workspace.to_string().into(),
            }));
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
            "f" => Some(PreviewKeyAction::Command(WmCommand::ToggleFullscreen)),
            "q" => Some(PreviewKeyAction::Command(WmCommand::CloseFocusedWindow)),
            "enter" => Some(PreviewKeyAction::Command(WmCommand::Spawn {
                command: "$openRandom".to_string(),
            })),
            _ => None,
        };
    }

    match key {
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

fn preview_command_from_key(event: &KeyboardEvent) -> Option<PreviewKeyAction> {
    preview_command_from_parts(
        &event.key(),
        &event.code(),
        event.alt_key(),
        event.ctrl_key(),
        event.shift_key(),
    )
}

fn should_ignore_preview_key_event(event: &web_sys::KeyboardEvent) -> bool {
    if event.alt_key() {
        return false;
    }

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
