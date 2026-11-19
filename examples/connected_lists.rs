use dioxus::prelude::*;
use dioxus_dnd_kit::prelude::*;

const CSS: &str = r#"
* { box-sizing: border-box; }
body { margin: 0; color: #243247; background: #eef2f7; font: 15px/1.5 system-ui, sans-serif; }
button, input { font: inherit; }
button { cursor: pointer; }
.page { max-width: 1240px; margin: auto; padding: 32px 24px; }
h1 { margin: 0; font-size: 30px; letter-spacing: -.025em; }
.intro { margin: 8px 0 20px; color: #5e6d82; max-width: 760px; }
.toolbar { display: flex; align-items: center; flex-wrap: wrap; gap: 20px; margin-bottom: 20px; }
.toolbar label { display: flex; align-items: center; gap: 8px; }
.toolbar button { margin-left: auto; border: 1px solid #bdc9d8; background: white; border-radius: 7px; padding: 7px 14px; }
.total { font-weight: 650; color: #385a7d; }
.validation { display: flex; flex-wrap: wrap; align-items: center; gap: 12px 20px; margin: 0 0 18px; padding: 12px; border-radius: 8px; background: #fff; }
.validation label { display: flex; align-items: center; gap: 6px; }
.validation p { flex-basis: 100%; margin: 0; color: #536987; font-size: 13px; }
.validation button { padding: 4px 10px; }
button:disabled { opacity: .45; cursor: default; }
.board-scroll { overflow-x: auto; padding-bottom: 12px; }
.board { display: grid; grid-template-columns: repeat(3, minmax(240px, 1fr)); gap: 18px; min-width: 820px; align-items: start; }
.column { min-width: 0; padding: 12px; background: #e2e8f1; border: 1px solid #d1dbe8; border-radius: 12px; }
.column-head { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 0 4px 10px; }
.column-head h2 { margin: 0; font-size: 15px; }
.column-head .column-handle { flex: none; cursor: grab; color: #5e7594; padding: 0 7px; border-radius: 4px; background: #fff8; user-select: none; }
.column-head h2 { flex: 1; }
.column-placeholder { min-height: 350px; display: grid; place-items: center; color: #5e7594; }
.column-status { min-height: 34px; display: flex; align-items: center; gap: 16px; margin-bottom: 10px; color: #60718a; font-size: 13px; }
.column-status button { margin-left: auto; }
.column-snapshot { overflow: hidden; }
.count { background: #fff9; color: #586a82; min-width: 26px; text-align: center; border-radius: 6px; font-size: 13px; }
.kanban-list { display: flex; flex-direction: column; gap: 12px; min-height: 320px; max-height: 520px; overflow-y: auto; padding: 5px; border: 1px dashed transparent; border-radius: 8px; }
.kanban-list:has(.placeholder) { border-color: #6589b7; background: #dbe7f5; }
.kanban-list:empty::after { content: "Drop a card here"; color: #7c8ca1; text-align: center; padding: 56px 12px; pointer-events: none; }
.card { min-width: 0; padding: 14px; border: 1px solid #d2dce9; border-radius: 9px; background: white; box-shadow: 0 2px 4px #253e5b0b; overflow-wrap: anywhere; }
.card-head { display: flex; align-items: flex-start; gap: 10px; }
.handle { flex: none; padding: 2px 8px; color: #5e7594; background: #f0f4fa; border-radius: 4px; cursor: grab; user-select: none; }
.handle:active { cursor: grabbing; }
.card h3 { margin: 0; font-size: 15px; }
.card p { color: #596b82; margin: 10px 0 0; font-size: 13px; }
.card-foot { display: flex; align-items: center; justify-content: space-between; margin-top: 14px; }
.tag { color: #2e638f; background: #eaf2fb; border-radius: 4px; padding: 2px 6px; font-size: 11px; font-weight: 650; }
.card button { border: 0; background: transparent; color: #5e7594; font-size: 12px; padding: 2px 0; }
.card .details { border-top: 1px solid #edf0f6; margin-top: 12px; padding-top: 10px; }
.placeholder { height: 100%; width: 100%; min-height: 30px; border: 2px dashed #6d91bf; border-radius: 9px; background: #d4e3f7; }
.diagnostics { display: grid; gap: 6px; padding: 16px 0; color: #60718a; font-size: 13px; }
.orders { display: flex; flex-wrap: wrap; gap: 18px; }
.independent { margin-top: 12px; border-top: 1px solid #d4ddea; padding-top: 18px; }
.independent h2 { margin: 0; font-size: 16px; }
.independent > p { margin: 6px 0 12px; color: #6b7a8e; font-size: 13px; }
.standalone-list { max-width: 430px; }
@media(max-width: 650px) { .page { padding: 20px 12px; } h1 { font-size: 26px; } }
"#;

#[derive(Clone, PartialEq)]
struct Task {
    id: u32,
    title: &'static str,
    description: &'static str,
    tag: &'static str,
    expanded: bool,
}

#[derive(Clone, PartialEq)]
struct Column {
    id: &'static str,
    title: &'static str,
    items: Signal<Vec<Task>>,
}

#[derive(Clone, Copy)]
struct BoardData {
    columns: Signal<Vec<Column>>,
    standalone: Signal<Vec<Task>>,
}

fn task(id: u32, title: &'static str, description: &'static str, tag: &'static str) -> Task {
    Task {
        id,
        title,
        description,
        tag,
        expanded: false,
    }
}

fn initial_todo() -> Vec<Task> {
    vec![
        task(
            1,
            "Sketch the onboarding",
            "A short card. Move it to another column, then grab it again.",
            "Design",
        ),
        task(
            2,
            "Write the release checklist",
            "This card has more text and a different natural height. Try inserting it above and below another card, or dropping it between them.",
            "Planning",
        ),
        task(
            3,
            "Audit the empty state",
            "Done starts empty. Its entire drop area should accept this card.",
            "UX",
        ),
    ]
}

fn initial_progress() -> Vec<Task> {
    vec![task(
        4,
        "Ship connected sorting",
        "Transfers update both columns while dragging. Escape or release outside the board returns the card to where the drag began.",
        "Rust",
    )]
}

fn main() {
    launch(App);
}

#[component]
fn App() -> Element {
    let todo = use_signal(initial_todo);
    let progress = use_signal(initial_progress);
    let done = use_signal(Vec::<Task>::new);
    let standalone = use_signal(|| {
        vec![task(
            90,
            "Personal scratchpad",
            "This list is outside ConnectedLists and keeps the default absolute layout. It cannot send or receive board cards.",
            "Independent",
        )]
    });
    let columns = use_signal(move || {
        vec![
            Column {
                id: "todo",
                title: "To do",
                items: todo,
            },
            Column {
                id: "progress",
                title: "In progress",
                items: progress,
            },
            Column {
                id: "done",
                title: "Done",
                items: done,
            },
        ]
    });
    use_context_provider(move || BoardData {
        columns,
        standalone,
    });
    let mut layout = use_signal(|| DragLayout::Relative);
    let mut last_started = use_signal(|| "No card picked up yet".to_string());
    let mut reject_next = use_signal(|| false);
    let design_only = use_signal(|| false);
    let mut save_message = use_signal(|| {
        "Drop a card to simulate a 1.2s API request. Reorders and transfers use the same transaction.".to_string()
    });
    let mut undo = use_signal(|| None::<DropEvent<Task>>);
    let mut column_undo = use_signal(|| None::<DropEvent<Column>>);
    let total = todo.read().len() + progress.read().len() + done.read().len();
    let order = |items: Vec<Task>| {
        items
            .iter()
            .map(|task| task.id.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let todo_order = order(todo());
    let progress_order = order(progress());
    let done_order = order(done());

    rsx! {
        document::Title { "Connected DnD board" }
        style { "{CSS}" }
        DraggableView {
            class: "page",
            key_gen: |task: Task| task.id.to_string(),
            render: |task: Task| rsx! { TaskCard { task, interactive: true } },
            placeholder_render: |_| rsx! { div { class: "placeholder" } },
            can_drop: move |change: DropChange<Task>| {
                !design_only() || change.to.list_id != "done" || change.item.tag == "Design"
            },
            on_drop: move |event: DropEvent<Task>| async move {
                undo.set(None);
                let fail = *reject_next.peek();
                reject_next.set(false);
                let change = &event.change;
                save_message.set(format!("Saving #{}: {}[{}] → {}[{}]…", change.item.id,
                    change.from.list_id, change.from.index, change.to.list_id, change.to.index));
                gloo_timers::future::TimeoutFuture::new(1200).await;
                if fail {
                    let result = event.reject();
                    save_message.set(format!("Simulated API error. Rollback: {result:?}"));
                } else if event.accept() == DropResult::Applied {
                    save_message.set(format!("Saved #{}. You can undo this move.", event.change.item.id));
                    undo.set(Some(event));
                } else {
                    save_message.set("The operation is no longer available; refresh authoritative data.".into());
                }
            },
            on_drag_cancel: move |event: DragCancel<Task>| save_message.set(format!("Drag cancelled: {:?}", event.reason)),

            h1 { "A small board, connected lists" }
            p { class: "intro", "Drag a column by its header grip, or move cards by their own grips. Drop at the highlighted placeholder. Escape and window blur cancel. Both levels support delayed API validation and Undo." }
            div { class: "toolbar",
                label {
                    input { r#type: "checkbox", checked: layout() == DragLayout::Relative,
                        onchange: move |event| layout.set(if event.checked() { DragLayout::Relative } else { DragLayout::Absolute }),
                    }
                    "Relative layout"
                }
                span { class: "total", "Board total: {total} / 4" }
                ResetBoard { todo, progress, done, columns, undo, column_undo }
            }
            ValidationControls { reject_next, design_only, save_message, undo }
            ConnectedLists::<Task> {
                on_drag_start: move |task: Task| last_started.set(format!("Last picked up: #{}", task.id)),
                DraggableView {
                    key_gen: |column: Column| column.id.to_string(),
                    render: move |column: Column| rsx! { ColumnContent { column, layout } },
                    overlay_render: |column: Column| rsx! { ColumnSnapshot { column } },
                    placeholder_render: |_| rsx! { div { class: "placeholder column-placeholder", "Column" } },
                    on_drag_start: move |event: DragStart<Column>| last_started.set(format!("Last picked up: column {}", event.item.title)),
                    on_drag_cancel: move |event: DragCancel<Column>| save_message.set(format!("Column drag cancelled: {:?}", event.reason)),
                    on_drop: move |event: DropEvent<Column>| async move {
                        column_undo.set(None);
                        let fail = *reject_next.peek();
                        reject_next.set(false);
                        save_message.set(format!("Saving column {}: {} → {}…", event.change.item.title, event.change.from.index, event.change.to.index));
                        gloo_timers::future::TimeoutFuture::new(1200).await;
                        if fail {
                            let result = event.reject();
                            save_message.set(format!("Column API error. Rollback: {result:?}"));
                        } else if event.accept() == DropResult::Applied {
                            save_message.set(format!("Saved column order: {}.", event.change.item.title));
                            column_undo.set(Some(event));
                        }
                    },
                    ColumnStatus { columns, column_undo, save_message }
                    div { class: "board-scroll",
                        Droppable { id: "board-columns", class: "board", items: columns,
                            axis: DragAxis::Horizontal, layout }
                    }
                }
            }
            div { class: "diagnostics",
                ActiveStatus {}
                div { "{last_started}" }
                div { class: "orders",
                    code { "To do: [{todo_order}]" }
                    code { "In progress: [{progress_order}]" }
                    code { "Done: [{done_order}]" }
                }
            }
            section { class: "independent",
                h2 { "An independent list" }
                p { "No ConnectedLists wrapper, no new props. The existing absolute setup continues to work." }
                Droppable { class: "standalone-list", items: standalone }
            }
        }
    }
}

#[component]
fn ResetBoard(
    mut todo: Signal<Vec<Task>>,
    mut progress: Signal<Vec<Task>>,
    mut done: Signal<Vec<Task>>,
    mut columns: Signal<Vec<Column>>,
    mut undo: Signal<Option<DropEvent<Task>>>,
    mut column_undo: Signal<Option<DropEvent<Column>>>,
) -> Element {
    let state = use_dnd_state::<Task>();
    rsx! { button { disabled: state.is_busy(),
        onclick: move |_| {
            undo.set(None); column_undo.set(None);
            todo.set(initial_todo()); progress.set(initial_progress()); done.set(Vec::new());
            columns.with_mut(|columns| columns.sort_by_key(|column| match column.id { "todo" => 0, "progress" => 1, _ => 2 }));
        },
        "Reset board"
    } }
}

#[component]
fn ValidationControls(
    mut reject_next: Signal<bool>,
    mut design_only: Signal<bool>,
    mut save_message: Signal<String>,
    mut undo: Signal<Option<DropEvent<Task>>>,
) -> Element {
    let state = use_dnd_state::<Task>();
    let pending = state.is_pending();
    let busy = state.is_busy();
    let _ = state.last_outcome();
    rsx! { div { class: "validation",
        label { input { r#type: "checkbox", checked: reject_next(), disabled: busy,
            onchange: move |event| reject_next.set(event.checked()), }
            "Reject next API request"
        }
        label { input { r#type: "checkbox", checked: design_only(), disabled: busy,
            onchange: move |event| design_only.set(event.checked()), }
            "Done: Design cards only"
        }
        if pending { span { "Waiting for API…" } }
        if let Some(event) = undo().filter(|event| event.status() == DropStatus::Accepted) {
            button { disabled: busy, onclick: move |_| {
                // A real application awaits its reverse server request first.
                let result = event.undo();
                undo.set(None);
                save_message.set(format!("Undo: {result:?}"));
            }, "Undo card move" }
        }
        p { "{save_message}" }
    } }
}

#[component]
fn TaskCard(task: Task, interactive: bool) -> Element {
    let board = use_context::<BoardData>();
    let id = task.id;
    rsx! {
        article { class: "card",
            div { class: "card-head",
                if interactive { DraggableHandler { class: "handle", item: task.clone(), "⠿" } }
                else { span { class: "handle", "⠿" } }
                h3 { "#{id} {task.title}" }
            }
            p { "{task.description}" }
            div { class: "card-foot",
                span { class: "tag", "{task.tag}" }
                button { disabled: !interactive, onclick: move |_| {
                    let mut lists = board.columns.peek().iter().map(|column| column.items).collect::<Vec<_>>();
                    lists.push(board.standalone);
                    for mut items in lists {
                        if items.peek().iter().any(|task| task.id == id) {
                            items.with_mut(|items| {
                                if let Some(task) = items.iter_mut().find(|task| task.id == id) { task.expanded = !task.expanded; }
                            });
                            break;
                        }
                    }
                }, if task.expanded { "Less" } else { "Details" } }
            }
            if task.expanded {
                p { class: "details", "Expanded state lives in the item data and follows it between columns. The card height remains natural in relative layout." }
            }
        }
    }
}

#[component]
fn ColumnContent(column: Column, layout: Signal<DragLayout>) -> Element {
    rsx! {
        section { class: "column",
            div { class: "column-head",
                DraggableHandler { class: "column-handle", item: column.clone(), "⠿" }
                h2 { "{column.title}" }
                span { class: "count", "{column.items.read().len()}" }
            }
            Droppable { id: column.id, class: "kanban-list", items: column.items, layout }
        }
    }
}

// The overlay is a passive snapshot. The original keyed column stays mounted,
// preserving its list registration, scroll position, and component state.
#[component]
fn ColumnSnapshot(column: Column) -> Element {
    rsx! {
        section { class: "column",
            div { class: "column-head",
                span { class: "column-handle", "⠿" }
                h2 { "{column.title}" }
                span { class: "count", "{column.items.read().len()}" }
            }
            div { class: "kanban-list column-snapshot",
                for task in (column.items)() { TaskCard { key: "{task.id}", task, interactive: false } }
            }
        }
    }
}

#[component]
fn ColumnStatus(
    columns: Signal<Vec<Column>>,
    mut column_undo: Signal<Option<DropEvent<Column>>>,
    mut save_message: Signal<String>,
) -> Element {
    let state = use_dnd_state::<Column>();
    let busy = state.is_busy();
    let _ = state.last_outcome();
    let order = columns
        .read()
        .iter()
        .map(|column| column.title)
        .collect::<Vec<_>>()
        .join(" → ");
    rsx! { div { class: "column-status",
        span { "Columns: {order}" }
        if state.is_pending() { span { "Saving columns…" } }
        if let Some(event) = column_undo().filter(|event| event.status() == DropStatus::Accepted) {
            button { disabled: busy, onclick: move |_| {
                let result = event.undo();
                column_undo.set(None);
                save_message.set(format!("Column undo: {result:?}"));
            }, "Undo column move" }
        }
    } }
}

#[component]
fn ActiveStatus() -> Element {
    let context = use_dnd_context::<Task>();
    let active = context.active.read().as_ref().map(|task| task.id);
    rsx! { div { if let Some(id) = active { "Dragging #{id}" } else { "Ready to drag" } } }
}
