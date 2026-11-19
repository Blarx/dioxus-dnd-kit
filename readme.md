# dioxus-dnd-kit 🦀
A flexible, performant, and layout-stable Drag-and-Drop library for **Dioxus 0.7**.

Developed to handle smooth reordering, even when using JIT-engines like **Tailwind CSS**. It avoids layout flickering by keeping the DOM structure stable during drag operations.

See the [changelog](./CHANGELOG.md) for version history and compatibility notes.

### Features
- **Layout Stability**: No more jumping elements when an item is picked up.
- **Tailwind Friendly**: Handles dynamic style injections gracefully.
- **High Performance**: Uses `join_all` for batch rect measurements.
- **Customizable**: Full control over handlers, overlays, and placeholders.
- **Relative Layout**: Opt in to normal-flow items with natural, dynamic heights.
- **Compatible Default**: Existing lists keep their absolute positioning mode.
- **Connected Lists**: Opt in to live transfers between explicitly identified containers.

### Installation
Add to your `Cargo.toml`:
```toml
[dependencies]
dioxus-dnd-kit = "0.3"
```

### Quick Start
```rust
use dioxus::prelude::*;
use dioxus_dnd_kit::prelude::*;

fn App() -> Element {
    let items = use_signal(|| vec!["Item 1".to_string(), "Item 2".to_string(), "Item 3".to_string()]);

    rsx! {
        DraggableView {
            class: "p-4 space-y-2",
            // Unique key for each item
            key_gen: move |item: String| item.clone(),
            // How to render each item in the list/overlay
            render: move |item: String| rsx! {
                div {
                    DraggableHandler { item: item.clone(), "{item}" }
                }
            },
            // The droppable container
            Droppable {
                class: "bg-gray-100 p-4 rounded-lg",
                items,
                // Items inside are rendered as Draggable automatically
            }
        }
    }
}
```

### Components
`DraggableView`

The root manager. Handles global mouse events and renders the DraggableOverlay.

`Draggable`

Wraps your items. It manages a placeholder to keep the list size consistent while an item is "floating".

`DraggableHandler`

A specific area to grab the item. Perfect if you only want a small icon to trigger the drag.

`Droppable`

The "brain" of the operation. It detects collisions and updates your items signal in real-time.

### Relative Layout

Set `layout` on a list to keep its items in normal document flow:

```rust,ignore
Droppable {
    items,
    layout: DragLayout::Relative,
    class: "my-list",
}
```

`DragLayout` is available in the prelude. Omitting `layout` keeps
`DragLayout::Absolute`, including for existing applications.

In relative mode, the original item stays mounted and keeps its natural height
while hidden during dragging. The fixed overlay follows the original grab point.
An optional `placeholder_render` is positioned over the reserved space and does
not add a second row height. Give its content `width: 100%; height: 100%` to fill
the slot. Card heights can change without manually sizing placeholders.

Use stable, unique IDs in `key_gen`; do not derive keys from list indices or
editable content. Sorting defaults to vertical and updates `items` while dragging. Item
size changes, viewport resize, and scrolling refresh collision geometry. The
overlay keeps its pickup dimensions until release.

Run the standalone example (plain CSS, no Tailwind dependency):

```sh
dx serve --example relative_list --web
```

It includes natural card heights, expandable content, a narrow-list toggle,
an optional custom placeholder, a nested scroll container, resizing the active
item during dragging, and a switch to compare absolute mode.

### Advanced: Tailwind & Dynamic Rects
Item size changes are observed through Dioxus resize events. You can also trigger
`recalculate_rects` on `DndContext` when an application changes item positions
without changing their sizes. The `simple_list` example additionally shows a
`MutationObserver` for Tailwind Play CDN style injection.

### Connected Lists

Place related lists inside `ConnectedLists::<YourItem>` under one `DraggableView`:

```rust,ignore
ConnectedLists::<Task> {
    Droppable { id: "todo", items: todo, layout: DragLayout::Relative }
    Droppable { id: "in-progress", items: in_progress, layout: DragLayout::Relative }
    Droppable { id: "done", items: done, layout: DragLayout::Relative }
}
```

Each list needs a distinct, stable HTML `id` and its own `Signal<Vec<Task>>`. Item keys
from `key_gen` must be unique across the entire `DraggableView`. Give empty lists
a non-zero CSS `min-height` so they have a drop area. Both absolute and relative
layouts are supported, including lists with different layout modes in one group.
Only visible parts of a drop area receive cards; scroll and overflow clipping are
respected. As with other HTML IDs, container IDs must be unique in the document.

Transfers update both signals immediately while dragging. A highlighted
placeholder marks an accepted drop, including when the handle remains outside
the list. Targets prefer the pointer; otherwise the greatest overlap covering
at least 25% of the smaller visible surface wins. Hover and release use the same
rule. Moving away from all targets hides the preview while preserving its space.
Escape, window blur, or release without an accepted target returns the item to
its original list and position. Rollback preserves unrelated edits made to other
items. Optional `on_drag_start` on
`ConnectedLists` receives the picked-up item once per drag.

Moving between containers remounts the item's original content. Keep UI state
that must survive a transfer in item data or in a parent keyed by item ID. The
fixed overlay remains mounted and retains its pickup dimensions and grab offset.

Lists outside the group, and lists without an `id`, remain independent. Existing
`Droppable { items, ... }` code keeps its default absolute behavior.

### Validating changes with an API

Set `on_drop` on `DraggableView` to receive a single transaction after either a
same-list reorder or a transfer. The item stays at the previewed position while
you await your API. Give lists stable `id` values to identify resources/masters;
map the list position to appointment times in your application.

```rust,ignore
on_drop: move |event: DropEvent<Booking>| async move {
    match api.update_booking(event.change.clone()).await {
        Ok(()) => { event.accept(); }
        Err(_) => {
            if event.reject() == DropResult::Conflict {
                // A server refresh moved/deleted this booking already.
                // Reload authoritative data instead of forcing a rollback.
            }
        }
    }
},
```

`event.change` contains `item`, `key`, and `from`/`to` locations with `list_id`,
`index`, `previous_key`, and `next_key`. The source is captured at pickup. There
are no drop requests for intermediate hovers, cancellation, or unchanged positions.
Anonymous lists report an empty `list_id`.

Use synchronous `can_drop: |change: DropChange<Booking>| -> bool { ... }` for fast
local eligibility checks; keep remote business checks in `on_drop`. While a drop
is pending, new pickups in that view are blocked. `use_dnd_state::<Booking>()`
below the view exposes `is_pending()`, `pending()`, and `last_outcome()`.
`is_busy()` includes active drags and validation across the whole nested view tree.

Resolve the request with `accept()` or `reject()`. Dropping the final unresolved
event automatically rejects it. Rejection restores only the moved item, preserving
unrelated list/data edits. An authoritative relocation/deletion returns `Conflict`
and unlocks the view. Unmounting a participating list invalidates outstanding
requests, so late replies cannot mutate disposed or replacement lists.

An accepted event can be saved for a toast's Undo button: `event.undo()` performs
the inverse move once, with conflict checks. The next changed drop invalidates
older Undo handles. Perform any reverse API request in your application and check
the result of `undo()`. `on_drag_start` and `on_drag_cancel` provide lifecycle
metadata; without `on_drop`, existing lists still accept changes immediately.

### Sorting the columns themselves

Use `axis: DragAxis::Horizontal` on a `Droppable<Column>` and arrange it with
horizontal flex/grid CSS. Vertical remains the default; connected insertion
uses each destination's axis.

The board example nests `DraggableView<Column>` inside the task group's
`DraggableView<Task>` / `ConnectedLists<Task>`. The column renderer contains a task
list, so all tasks still share their outer context and connected group. Stable
column keys preserve each live list and its state when the columns move.

Set `overlay_render` for the column view to draw a passive snapshot of its
contents. It must not mount a second set of live `Droppable`s or duplicate IDs.
The regular `render` still draws the original column and its working task list.

Nested views isolate drag sessions and group membership, and share a single
drag/validation lease. During a column drag or API request, card pickups are
blocked; during card validation, column pickups are blocked. Use
`use_dnd_state::<T>().is_busy()` for shared disabled controls. Each view keeps its
own drop events and Undo history.

Run the Trello-style board example (includes sortable columns, delayed API
success/error at both levels, local eligibility, pending state, and separate Undo):

```sh
dx serve --example connected_lists --web
```

### Examples

[Try the live demos](https://blarx.github.io/dioxus-dnd-kit/) or browse the
[example source code](./examples).

### Compatibility

`0.3` adds connected lists through an opt-in wrapper and container IDs. Existing
component calls, the `prelude` imports, `DndItem` bounds, and public context fields
remain compatible. Omitting `layout` still selects `DragLayout::Absolute`.

### License

This library is dual-licensed under the [MIT license](./LICENSE-MIT) or the
[Apache License, Version 2.0](./LICENSE-APACHE), at your option.
