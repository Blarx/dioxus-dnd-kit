use dioxus::prelude::*;
use dioxus_dnd_kit::prelude::*;

const CSS: &str = r#"
* { box-sizing: border-box; }
body { margin: 0; background: #f3f5f9; color: #203047; font: 16px/1.5 system-ui, sans-serif; }
button, input { font: inherit; }
button { cursor: pointer; }
.page { max-width: 980px; margin: auto; padding: 32px 20px; }
h1 { margin: 0; font-size: 28px; }
.intro { color: #58677c; max-width: 720px; }
.toolbar { display: flex; flex-wrap: wrap; align-items: center; gap: 16px; margin: 24px 0 12px; }
.toolbar label { display: flex; align-items: center; gap: 8px; }
.order { display: block; margin-bottom: 16px; }
.scroller { max-height: 540px; overflow: auto; padding: 12px; border: 1px solid #ccd5e2; border-radius: 12px; background: #e9eef6; }
.list { display: flex; flex-direction: column; gap: 14px; padding: 8px; }
.card { padding: 18px; background: white; border: 1px solid #d2dbe8; border-radius: 10px; box-shadow: 0 2px 4px #172c4810; overflow-wrap: anywhere; }
.card-head { display: flex; gap: 12px; align-items: center; }
.handle { flex: none; padding: 6px 10px; border: 1px solid #c7d2e0; border-radius: 6px; background: #f4f7fc; cursor: grab; user-select: none; }
.handle:active { cursor: grabbing; }
.card h2 { margin: 0; font-size: 17px; }
.card p { margin: 14px 0 0; }
.card button { margin-top: 14px; padding: 6px 10px; color: #22508c; border: 1px solid #c7d2e0; border-radius: 6px; background: #f4f7fc; }
.detail { padding-top: 12px; color: #58677c; }
.placeholder { width: 100%; height: 100%; border: 2px dashed #6487b7; border-radius: 10px; background: #dce8f7; }
.status { color: #58677c; margin-top: 16px; }
@media (max-width: 600px) { .page { padding: 20px 12px; } .card { padding: 12px; } }
"#;

#[derive(Clone, PartialEq)]
struct Task {
    id: usize,
    title: &'static str,
    description: &'static str,
    expanded: bool,
}

fn initial_tasks() -> Vec<Task> {
    vec![
        Task {
            id: 1,
            title: "A short card",
            description: "Its height comes entirely from its content.",
            expanded: false,
        },
        Task {
            id: 2,
            title: "A card with more text",
            description: "Drag the short card across this one in both directions. Text wraps naturally when you narrow the list, so the collision rectangles must follow the new layout rather than the original height.",
            expanded: true,
        },
        Task {
            id: 3,
            title: "Expandable content",
            description: "Expand and collapse this card before dragging it. Its stable ID stays the same even though its data and height change.",
            expanded: false,
        },
        Task {
            id: 4,
            title: "Another short card",
            description: "Use the grip to change the order.",
            expanded: false,
        },
        Task {
            id: 5,
            title: "Scroll while dragging",
            description: "Hold a card near the top or bottom edge of the scroll area. The list scrolls and you can keep sorting with the pointer held still.",
            expanded: true,
        },
    ]
}

fn main() {
    launch(App);
}

#[component]
fn App() -> Element {
    let mut tasks = use_signal(initial_tasks);
    let mut layout = use_signal(|| DragLayout::Relative);
    let mut narrow = use_signal(|| false);
    let mut auto_scroll = use_signal(|| true);
    let mut custom_placeholder = use_signal(|| true);
    let mut resize_during_drag = use_signal(|| false);
    let order = tasks()
        .iter()
        .map(|task| task.id.to_string())
        .collect::<Vec<_>>()
        .join(" → ");

    rsx! {
        document::Title { "Relative DnD list" }
        style { "{CSS}" }
        DraggableView {
            class: "page",
            auto_scroll,
            key_gen: |task: Task| task.id.to_string(),
            render: move |task: Task| {
                let id = task.id;
                rsx! {
                    article { class: "card",
                        div { class: "card-head",
                            DraggableHandler { class: "handle", item: task.clone(), "⠿" }
                            h2 { "#{id} {task.title}" }
                        }
                        p { "{task.description}" }
                        button {
                            onclick: move |_| {
                                tasks.with_mut(|tasks| {
                                    if let Some(task) = tasks.iter_mut().find(|task| task.id == id) {
                                        task.expanded = !task.expanded;
                                    }
                                });
                            },
                            if task.expanded { "Collapse" } else { "Expand" }
                        }
                        if task.expanded {
                            div { class: "detail",
                                "Extra content uses normal document flow. No card height or placeholder height is prescribed by this example. Resize the list to see this paragraph wrap onto more lines."
                            }
                        }
                    }
                }
            },
            placeholder_render: move |_| rsx! {
                if custom_placeholder() { div { class: "placeholder" } }
            },

            ResizeDuringDrag { tasks, enabled: resize_during_drag }
            h1 { "Relative DnD list" }
            p { class: "intro",
                "Natural card heights, stable keys and a fixed drag overlay. Compare the relative layout with the compatible absolute mode."
            }
            div { class: "toolbar",
                label {
                    input {
                        r#type: "checkbox", checked: layout() == DragLayout::Relative,
                        onchange: move |event| layout.set(if event.checked() { DragLayout::Relative } else { DragLayout::Absolute }),
                    }
                    "Relative layout"
                }
                label {
                    input { r#type: "checkbox", checked: narrow(), onchange: move |event| narrow.set(event.checked()) }
                    "Narrow list"
                }
                label {
                    input {
                        r#type: "checkbox", checked: auto_scroll(),
                        onchange: move |event| auto_scroll.set(event.checked()),
                    }
                    "Auto-scroll"
                }
                label {
                    input { r#type: "checkbox", checked: custom_placeholder(), onchange: move |event| custom_placeholder.set(event.checked()) }
                    "Custom placeholder"
                }
                label {
                    input { r#type: "checkbox", checked: resize_during_drag(), onchange: move |event| resize_during_drag.set(event.checked()) }
                    "Resize active during drag"
                }
                button { onclick: move |_| tasks.set(initial_tasks()), "Reset" }
            }
            code { class: "order", "Order: {order}" }
            div {
                class: "scroller",
                style: if narrow() { "width: min(100%, 360px);" } else { "width: 100%;" },
                Droppable { class: "list", items: tasks, layout }
            }
            DragStatus {}
        }
    }
}

#[component]
fn DragStatus() -> Element {
    let context = use_dnd_context::<Task>();
    let active_id = context.active.read().as_ref().map(|task| task.id);
    rsx! {
        p { class: "status",
            if let Some(id) = active_id { "Dragging #{id}" } else { "Ready — drag a card by its grip." }
        }
    }
}

#[component]
fn ResizeDuringDrag(mut tasks: Signal<Vec<Task>>, enabled: Signal<bool>) -> Element {
    let context = use_dnd_context::<Task>();
    let mut pending = use_signal(|| None::<dioxus::core::Task>);
    use_effect(move || {
        let active = context.active.read().clone();
        let enabled = enabled();
        if let Some(task) = pending.take() {
            task.cancel();
        }
        if enabled && let Some(active) = active {
            pending.set(Some(spawn(async move {
                gloo_timers::future::TimeoutFuture::new(500).await;
                tasks.with_mut(|tasks| {
                    if let Some(task) = tasks.iter_mut().find(|task| task.id == active.id) {
                        task.expanded = !task.expanded;
                    }
                });
            })));
        }
    });
    rsx! {}
}
