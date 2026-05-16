use dioxus::prelude::*;
use dioxus_dnd_kit::prelude::*;
use gloo_timers::future::TimeoutFuture;

const APP_CSS: &str = r#"
html,
body {
    margin: 0;
    min-height: 100%;
    background: #f5f7fb;
    color: #1f2937;
    font-family:
        Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}

button,
input {
    font: inherit;
}

.relative {
    position: relative;
}

.resize-page {
    box-sizing: border-box;
    min-height: 100vh;
    padding: 32px;
    background: #f5f7fb;
}

.resize-shell {
    width: min(1120px, 100%);
    margin: 0 auto;
    display: grid;
    grid-template-columns: minmax(360px, 1fr) 320px;
    gap: 24px;
    align-items: start;
}

.resize-header {
    grid-column: 1 / -1;
    display: flex;
    justify-content: space-between;
    gap: 24px;
    align-items: flex-end;
    padding-bottom: 8px;
}

.resize-title {
    margin: 0;
    font-size: 28px;
    line-height: 1.1;
    font-weight: 750;
}

.resize-subtitle {
    margin: 8px 0 0;
    max-width: 680px;
    color: #5b6472;
    line-height: 1.5;
}

.resize-counter {
    min-width: 136px;
    border: 1px solid #cfd7e6;
    border-radius: 8px;
    padding: 12px 14px;
    background: #ffffff;
    text-align: right;
}

.resize-counter span {
    display: block;
    color: #6b7280;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
}

.resize-counter strong {
    display: block;
    margin-top: 4px;
    font-size: 24px;
}

.resize-workspace,
.resize-inspector {
    border: 1px solid #d7dde8;
    border-radius: 8px;
    background: #ffffff;
}

.resize-workspace {
    padding: 18px;
}

.resize-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 16px;
}

.resize-toolbar-title {
    margin: 0;
    font-size: 15px;
    font-weight: 700;
}

.resize-toggle {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: #374151;
    font-size: 14px;
    white-space: nowrap;
}

.resize-toggle input {
    width: 18px;
    height: 18px;
    accent-color: #0f766e;
}

.resize-action {
    border: 1px solid #b8c2d4;
    border-radius: 8px;
    padding: 8px 11px;
    background: #f8fafc;
    color: #1f2937;
    cursor: pointer;
}

.resize-action:hover {
    background: #eef4ff;
}

.resize-droppable {
    display: grid;
    gap: 14px;
    min-height: 450px;
    padding: 14px;
    border: 1px solid #ccd5e3;
    border-radius: 8px;
    background: #f8fafc;
}

.resize-droppable > .relative {
    min-height: 74px;
}

.task-card {
    box-sizing: border-box;
    height: 74px;
    overflow: hidden;
    border: 1px solid #d5dce8;
    border-left: 5px solid #2563eb;
    border-radius: 8px;
    background: #ffffff;
    box-shadow: 0 1px 2px rgba(15, 23, 42, 0.06);
    transition:
        height 180ms ease,
        border-color 180ms ease,
        box-shadow 180ms ease;
}

.task-card-expanded {
    box-shadow: 0 10px 24px rgba(15, 23, 42, 0.10);
}

.task-content {
    box-sizing: border-box;
    height: 100%;
    padding: 12px;
    display: grid;
    grid-template-rows: auto 1fr;
    gap: 10px;
}

.task-main {
    display: grid;
    grid-template-columns: 38px minmax(0, 1fr) auto;
    gap: 12px;
    align-items: center;
}

.drag-handle {
    width: 38px;
    height: 38px;
    display: grid;
    place-items: center;
    border: 1px solid #cbd5e1;
    border-radius: 8px;
    background: #f1f5f9;
    color: #475569;
    cursor: grab;
    user-select: none;
}

.drag-handle:active {
    cursor: grabbing;
}

.task-name {
    min-width: 0;
}

.task-name strong {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 15px;
}

.task-name span {
    display: block;
    margin-top: 3px;
    color: #64748b;
    font-size: 12px;
}

.task-details {
    margin: 0;
    color: #475569;
    line-height: 1.45;
    font-size: 14px;
}

.resize-inspector {
    padding: 18px;
    display: grid;
    gap: 16px;
}

.inspector-title {
    margin: 0;
    font-size: 15px;
    font-weight: 700;
}

.metric {
    display: grid;
    gap: 4px;
}

.metric span {
    color: #64748b;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
}

.metric strong,
.metric code {
    color: #111827;
    font-size: 15px;
}

.height-list {
    display: grid;
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
}

.height-list li {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    border-bottom: 1px solid #e5e7eb;
    padding-bottom: 8px;
    color: #374151;
}

.height-list li:last-child {
    border-bottom: 0;
    padding-bottom: 0;
}

.placeholder {
    box-sizing: border-box;
    height: 100%;
    border: 1px dashed #94a3b8;
    border-radius: 8px;
    background: repeating-linear-gradient(
        135deg,
        #eef2ff,
        #eef2ff 10px,
        #e0f2fe 10px,
        #e0f2fe 20px
    );
}

@media (max-width: 860px) {
    .resize-page {
        padding: 18px;
    }

    .resize-shell {
        grid-template-columns: 1fr;
    }

    .resize-header {
        display: grid;
        gap: 14px;
    }

    .resize-counter {
        text-align: left;
    }

    .resize-toolbar {
        align-items: flex-start;
        flex-direction: column;
    }
}
"#;

fn main() {
    launch(App);
}

#[derive(Clone, Debug, PartialEq)]
struct ResizeTask {
    id: usize,
    title: &'static str,
    detail: &'static str,
    expanded: bool,
    collapsed_height: u32,
    expanded_height: u32,
    tone: &'static str,
}

impl ResizeTask {
    fn height(&self) -> u32 {
        if self.expanded {
            self.expanded_height
        } else {
            self.collapsed_height
        }
    }
}

#[component]
fn App() -> Element {
    let tasks = use_signal(|| {
        vec![
            ResizeTask {
                id: 1,
                title: "Inbox triage",
                detail: "Expanded content simulates a panel with controls, metadata, and wrapped text.",
                expanded: false,
                collapsed_height: 74,
                expanded_height: 156,
                tone: "#2563eb",
            },
            ResizeTask {
                id: 2,
                title: "Design review",
                detail: "This row starts expanded so the list has mixed heights before any drag begins.",
                expanded: true,
                collapsed_height: 74,
                expanded_height: 174,
                tone: "#0f766e",
            },
            ResizeTask {
                id: 3,
                title: "Release checklist",
                detail: "Collapse and expand this item, then drag around its neighbors to validate rects.",
                expanded: false,
                collapsed_height: 74,
                expanded_height: 146,
                tone: "#b45309",
            },
            ResizeTask {
                id: 4,
                title: "QA notes",
                detail: "The collision target should follow the current visual height, not a stale cached one.",
                expanded: false,
                collapsed_height: 74,
                expanded_height: 168,
                tone: "#be123c",
            },
        ]
    });
    let layout_version = use_signal(|| 0_u64);
    let recalculate_count = use_signal(|| 0_u64);
    let mut auto_resize_active = use_signal(|| false);

    let render_tasks = tasks;
    let render_layout_version = layout_version;
    let render = move |task: ResizeTask| {
        let tasks = render_tasks;
        let layout_version = render_layout_version;
        let id = task.id;
        let title = task.title;
        let detail = task.detail;
        let expanded = task.expanded;
        let height = task.height();
        let tone = task.tone;
        let state_label = if expanded { "Expanded" } else { "Collapsed" };
        let card_class = if expanded {
            "task-card task-card-expanded"
        } else {
            "task-card"
        };

        rsx! {
            div {
                class: "{card_class}",
                style: "height: {height}px; border-left-color: {tone};",
                div { class: "task-content",
                    div { class: "task-main",
                        DraggableHandler { class: "drag-handle", item: task.clone(), "::: " }
                        div { class: "task-name",
                            strong { "{title}" }
                            span { "#{id} / {state_label} / {height}px" }
                        }
                        label {
                            class: "resize-toggle",
                            onmousedown: move |evt| evt.stop_propagation(),
                            input {
                                r#type: "checkbox",
                                checked: expanded,
                                onchange: move |evt| {
                                    set_task_expanded(tasks, id, evt.checked());
                                    request_layout_recalculation(layout_version);
                                }
                            }
                            "Expanded"
                        }
                    }
                    p { class: "task-details", "{detail}" }
                }
            }
        }
    };

    let placeholder_render = move |_| {
        rsx! {
            div { class: "placeholder" }
        }
    };

    let order_text = tasks()
        .iter()
        .map(|task| task.id.to_string())
        .collect::<Vec<_>>()
        .join(" -> ");

    rsx! {
        style { "{APP_CSS}" }

        DraggableView {
            class: "resize-page",
            key_gen: |task: ResizeTask| task.id.to_string(),
            render,
            placeholder_render,

            ResizeRecalculationBridge {
                layout_version,
                recalculate_count,
            }
            ActiveResizeProbe {
                auto_resize_active,
                tasks,
                layout_version,
            }

            div { class: "resize-shell",
                header { class: "resize-header",
                    div {
                        h1 { class: "resize-title", "Resizable DnD list" }
                        p { class: "resize-subtitle",
                            "A focused regression surface for collapse/expand layout changes and cached collision rectangles."
                        }
                    }
                    div { class: "resize-counter",
                        span { "Recalculations" }
                        strong { "{recalculate_count()}" }
                    }
                }

                main { class: "resize-workspace",
                    div { class: "resize-toolbar",
                        h2 { class: "resize-toolbar-title", "Drag surface" }
                        label { class: "resize-toggle",
                            input {
                                r#type: "checkbox",
                                checked: auto_resize_active(),
                                onchange: move |evt| auto_resize_active.set(evt.checked()),
                            }
                            "Auto resize active"
                        }
                    }

                    Droppable {
                        class: "resize-droppable",
                        items: tasks,
                    }
                }

                aside { class: "resize-inspector",
                    h2 { class: "inspector-title", "Diagnostics" }
                    div { class: "metric",
                        span { "Order" }
                        code { "{order_text}" }
                    }
                    div { class: "metric",
                        span { "Layout version" }
                        strong { "{layout_version()}" }
                    }
                    button {
                        class: "resize-action",
                        onclick: move |_| request_layout_recalculation(layout_version),
                        "Force recalculation"
                    }
                    ul { class: "height-list",
                        for task in tasks().iter() {
                            li {
                                span { "#{task.id} {task.title}" }
                                strong { "{task.height()}px" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ResizeRecalculationBridge(
    layout_version: Signal<u64>,
    mut recalculate_count: Signal<u64>,
) -> Element {
    let mut context = use_context::<DndContext<ResizeTask>>();

    use_effect(move || {
        let version = layout_version();
        if version == 0 {
            return;
        }

        spawn(async move {
            TimeoutFuture::new(35).await;
            context.recalculate_rects.with_mut(|value| *value += 1);
            recalculate_count.with_mut(|value| *value += 1);
        });
    });

    rsx! {}
}

#[component]
fn ActiveResizeProbe(
    auto_resize_active: Signal<bool>,
    mut tasks: Signal<Vec<ResizeTask>>,
    mut layout_version: Signal<u64>,
) -> Element {
    let context = use_context::<DndContext<ResizeTask>>();
    let mut last_active_id = use_signal(|| None::<usize>);

    use_effect(move || {
        let active_id = context.active.read().as_ref().map(|task| task.id);

        if !auto_resize_active() {
            last_active_id.set(None);
            return;
        }

        let Some(active_id) = active_id else {
            last_active_id.set(None);
            return;
        };

        if *last_active_id.peek() == Some(active_id) {
            return;
        }
        last_active_id.set(Some(active_id));

        spawn(async move {
            TimeoutFuture::new(450).await;
            let still_active = context
                .active
                .peek()
                .as_ref()
                .map(|task| task.id == active_id)
                .unwrap_or(false);

            if still_active {
                toggle_task_height(tasks, active_id);
                request_layout_recalculation(layout_version);
            }
        });
    });

    rsx! {}
}

fn set_task_expanded(mut tasks: Signal<Vec<ResizeTask>>, id: usize, expanded: bool) {
    tasks.with_mut(|tasks| {
        if let Some(task) = tasks.iter_mut().find(|task| task.id == id) {
            task.expanded = expanded;
        }
    });
}

fn toggle_task_height(mut tasks: Signal<Vec<ResizeTask>>, id: usize) {
    tasks.with_mut(|tasks| {
        if let Some(task) = tasks.iter_mut().find(|task| task.id == id) {
            task.expanded = !task.expanded;
        }
    });
}

fn request_layout_recalculation(mut layout_version: Signal<u64>) {
    layout_version.with_mut(|version| *version += 1);
}
