use leptos::prelude::*;

#[component]
pub fn Tooltip(
    children: Children,
    #[prop(into)] content: Signal<String>,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] reveal_class: String,
) -> impl IntoView {
    let wrapper_class = if class.is_empty() {
        "group/tooltip relative flex items-center".to_string()
    } else {
        format!("group/tooltip relative flex items-center {class}")
    };
    let bubble_class = if reveal_class.is_empty() {
        "border-terminal-border bg-terminal-bg-panel text-terminal-fg pointer-events-none absolute top-full right-0 z-10 mt-1 hidden w-56 max-w-[calc(100vw-2rem)] border px-2 py-1 text-[11px] leading-4 wrap-break-word shadow-[0_14px_40px_rgba(0,0,0,0.45)] group-focus-within/tooltip:block group-hover/tooltip:block".to_string()
    } else {
        format!(
            "border-terminal-border bg-terminal-bg-panel text-terminal-fg pointer-events-none absolute top-full right-0 z-10 mt-1 hidden w-56 max-w-[calc(100vw-2rem)] border px-2 py-1 text-[11px] leading-4 wrap-break-word shadow-[0_14px_40px_rgba(0,0,0,0.45)] group-focus-within/tooltip:block group-hover/tooltip:block {reveal_class}"
        )
    };

    view! {
        <div class=wrapper_class>
            {children()}
            <div class=bubble_class>{move || content.get()}</div>
        </div>
    }
}
