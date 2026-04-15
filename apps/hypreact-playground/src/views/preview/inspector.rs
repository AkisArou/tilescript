use leptos::prelude::*;

use crate::session::PreviewSessionState;
use hypreact_core::snapshot::WindowSnapshot;
use hypreact_scene::LayoutSnapshotNode;

#[component]
pub fn InspectorPanel(#[prop(into)] title: Oco<'static, str>, children: Children) -> impl IntoView {
    view! {
        <div class="panel">
            <div class="panel-bar">{title}</div>
            <div class="panel-body">{children()}</div>
        </div>
    }
}

#[component]
pub fn WindowList(
    #[prop(into)] windows: Signal<Vec<WindowSnapshot>>,
    #[prop(into)] empty_label: Oco<'static, str>,
) -> impl IntoView {
    let fallback_label = empty_label.clone();

    view! {
        <Show
            when=move || !windows.get().is_empty()
            fallback=move || view! { <div class="muted">{fallback_label.clone()}</div> }
        >
            <div class="status-grid">
                {move || {
                    windows
                        .get()
                        .into_iter()
                        .map(|window| {
                            let title = window_display_title(&window).to_string();
                            let app_id = window.app_id.clone().unwrap_or_else(|| "unknown".to_string());
                            view! {
                                <div class="status-row">
                                    <div class="info-row info-row-inline">
                                        <span class="strong">{title}</span>
                                        <span class="muted">{app_id}</span>
                                    </div>
                                    <div class="info-row info-row-inline">
                                        <span class="muted">"mode"</span>
                                        <span class="strong">{window_mode_label(&window)}</span>
                                    </div>
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
pub fn WindowOrderSummary(
    #[prop(into)] label: Oco<'static, str>,
    #[prop(into)] windows: Signal<Vec<WindowSnapshot>>,
) -> impl IntoView {
    view! {
        <div class="status-row">
            <div class="eyebrow">{label}</div>
            <div class="muted">
                {move || {
                    let rows = windows.get();
                    if rows.is_empty() {
                        "none".to_string()
                    } else {
                        rows.into_iter()
                            .enumerate()
                            .map(|(index, window)| format!("{}:{}", index + 1, window_display_title(&window)))
                            .collect::<Vec<_>>()
                            .join("  ->  ")
                    }
                }}
            </div>
        </div>
    }
}

#[component]
pub fn LayoutTreeNode(
    node: LayoutSnapshotNode,
    windows: Vec<WindowSnapshot>,
    #[prop(optional)] depth: usize,
) -> AnyView {
    let label = node.meta().id.clone().unwrap_or_else(|| "_".to_string());
    let rect = node.rect();
    let rect_label = format!("{}x{}", rect.width.round() as i32, rect.height.round() as i32);
    let descendants = descendant_window_titles(&node, &windows);
    let descendant_text = descendants.clone();
    let children = node.children().to_vec();
    let node_type = layout_node_type_label(&node);

    view! {
        <div class="status-grid">
            <div class="status-row" style=format!("margin-left: {}px;", depth * 12)>
                <div class="info-row info-row-inline">
                    <span class="muted">{node_type}</span>
                    <span class="strong">{label}</span>
                </div>
                <div class="info-row info-row-inline">
                    <span class="muted">"rect"</span>
                    <span class="strong">{rect_label}</span>
                </div>
                <Show when=move || !descendants.is_empty()>
                    <div class="muted">{descendant_text.clone()}</div>
                </Show>
            </div>
            {children
                .into_iter()
                .map(|child| {
                    view! { <LayoutTreeNode node=child windows=windows.clone() depth=depth + 1 /> }
                })
                .collect_view()}
        </div>
    }
    .into_any()
}

pub fn claimed_visible_windows(session: &PreviewSessionState) -> Vec<WindowSnapshot> {
    let Some(scene) = session.scene.as_ref() else {
        return Vec::new();
    };
    session
        .visible_windows()
        .into_iter()
        .filter(|window| scene.root.find_by_window_id(&window.id).is_some())
        .collect()
}

pub fn unclaimed_visible_windows(session: &PreviewSessionState) -> Vec<WindowSnapshot> {
    let claimed =
        claimed_visible_windows(session).into_iter().map(|window| window.id).collect::<Vec<_>>();
    session
        .visible_windows()
        .into_iter()
        .filter(|window| !claimed.iter().any(|claimed_id| claimed_id == &window.id))
        .collect()
}

fn descendant_window_titles(node: &LayoutSnapshotNode, windows: &[WindowSnapshot]) -> String {
    let mut ids = Vec::new();
    collect_descendant_window_ids(node, &mut ids);
    ids.into_iter()
        .map(|window_id| {
            windows
                .iter()
                .find(|window| window.id == window_id)
                .map(|window| window_display_title(window).to_string())
                .unwrap_or_else(|| window_id.as_str().to_string())
        })
        .collect::<Vec<_>>()
        .join("  |  ")
}

fn collect_descendant_window_ids(
    node: &LayoutSnapshotNode,
    ids: &mut Vec<hypreact_core::WindowId>,
) {
    if let LayoutSnapshotNode::Window { window_id: Some(window_id), .. } = node {
        ids.push(window_id.clone());
    }
    for child in node.children() {
        collect_descendant_window_ids(child, ids);
    }
}

fn layout_node_type_label(node: &LayoutSnapshotNode) -> &'static str {
    match node {
        LayoutSnapshotNode::Workspace { .. } => "workspace",
        LayoutSnapshotNode::Group { .. } => "group",
        LayoutSnapshotNode::Content { .. } => "content",
        LayoutSnapshotNode::Window { .. } => "window",
    }
}

fn window_display_title(window: &WindowSnapshot) -> &str {
    window.title.as_deref().unwrap_or_else(|| window.id.as_str())
}

fn window_mode_label(window: &WindowSnapshot) -> &'static str {
    if window.mode.is_fullscreen() {
        "fullscreen"
    } else if window.mode.is_floating() {
        "floating"
    } else {
        "tiled"
    }
}
