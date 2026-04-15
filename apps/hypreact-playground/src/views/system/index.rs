use leptos::prelude::*;

use crate::app_state::AppState;
use crate::views::editor::command_palette_entries;

use super::state::{active_file_path, dirty_file_count};

#[component]
pub fn SystemView() -> impl IntoView {
    let app_state = expect_context::<AppState>();

    view! {
        <section class="page-grid page-grid-two">
            <div class="panel">
                <div class="panel-bar">"system://state"</div>
                <div class="panel-body info-grid">
                    {move || {
                        let session = app_state.session.get();
                        let buffers = app_state.editor_buffers.get();
                        let dirty_count = dirty_file_count(&buffers);
                        let preview_state = if session.scene.is_some() {
                            "ready"
                        } else if session.error.is_some() {
                            "degraded"
                        } else {
                            "booting"
                        };

                        vec![
                            ("workspace", session.active_workspace_name()),
                            ("layout", session.active_layout_name()),
                            (
                                "focused",
                                session
                                    .focused_window_id()
                                    .as_ref()
                                    .map(|window_id| session.window_name(window_id))
                                    .unwrap_or_else(|| "none".to_string()),
                            ),
                            ("dirty buffers", dirty_count.to_string()),
                            ("preview", preview_state.to_string()),
                            ("active file", active_file_path(app_state)),
                        ]
                            .into_iter()
                            .map(|(label, value)| (label.to_string(), value))
                            .map(|(label, value)| {
                                view! {
                                    <div class="info-row info-row-inline">
                                        <span class="muted">{label}</span>
                                        <span class="strong">{value}</span>
                                    </div>
                                }
                            })
                            .collect_view()
                    }}
                </div>
            </div>

            <section class="status-grid">
                <div class="panel">
                    <div class="panel-bar">"system://editor-actions"</div>
                    <div class="panel-body bindings-list">
                        {command_palette_entries()
                            .into_iter()
                            .map(|entry| {
                                view! {
                                    <div class="binding-item">
                                        <div class="strong">{entry.label}</div>
                                        <div class="muted">{entry.detail}</div>
                                    </div>
                                }
                            })
                            .collect_view()}
                    </div>
                </div>

                <div class="panel">
                    <div class="panel-bar">"system://session"</div>
                    <div class="panel-body info-grid">
                        {move || {
                            app_state
                                .session
                                .get()
                                .session_summary_rows()
                                .into_iter()
                                .map(|(label, value)| {
                                    view! {
                                        <div class="info-row info-row-inline">
                                            <span class="muted">{label}</span>
                                            <span class="strong">{value}</span>
                                        </div>
                                    }
                                })
                                .collect_view()
                        }}
                    </div>
                </div>

                <div class="panel">
                    <div class="panel-bar">"system://diagnostics"</div>
                    <div class="panel-body diagnostic-list">
                        <Show
                            when=move || !app_state.session.get().diagnostics.is_empty()
                            fallback=move || view! { <div class="muted">"no diagnostics"</div> }
                        >
                            {move || {
                                app_state
                                    .session
                                    .get()
                                    .diagnostics
                                    .into_iter()
                                    .map(|diagnostic| {
                                        view! {
                                            <div class="diagnostic-item">
                                                <div class="eyebrow">{format!("{} {}", diagnostic.severity, diagnostic.code)}</div>
                                                <div class="strong">{diagnostic.path}</div>
                                                <div class="muted">{diagnostic.message}</div>
                                                <code>{diagnostic.range}</code>
                                            </div>
                                        }
                                    })
                                    .collect_view()
                            }}
                        </Show>
                    </div>
                </div>

                <div class="panel">
                    <div class="panel-bar">"system://actions"</div>
                    <div class="panel-body event-log">
                        {move || {
                            app_state
                                .session
                                .get()
                                .event_log
                                .into_iter()
                                .rev()
                                .map(|entry| view! { <div class="event-item muted">{entry}</div> })
                                .collect_view()
                        }}
                    </div>
                </div>
            </section>
        </section>
    }
}
