# dioxus-dnd-kit 🦀
A flexible, performant, and layout-stable Drag-and-Drop library for **Dioxus 0.7**.

Developed to handle smooth reordering, even when using JIT-engines like **Tailwind CSS**. It avoids layout flickering by keeping the DOM structure stable during drag operations.

### Features
- **Layout Stability**: No more jumping elements when an item is picked up.
- **Tailwind Friendly**: Handles dynamic style injections gracefully.
- **High Performance**: Uses `join_all` for batch rect measurements.
- **Customizable**: Full control over handlers, overlays, and placeholders.
- **Relative Layout**: Opt in to normal-flow items with natural, dynamic heights.
- **Compatible Default**: Existing lists keep their absolute positioning mode.

### Installation
Add to your `Cargo.toml`:
```toml
[dependencies]
dioxus-dnd-kit = "0.2"
```

### Quick Start
```rust
use dioxus::prelude::*;
use dioxus_dnd_kit::prelude::*;

fn App() -> Element {
    let mut items = use_signal(|| vec!["Item 1".to_string(), "Item 2".to_string(), "Item 3".to_string()]);

    rsx! {
        DraggableView {
            class: "p-4 space-y-2",
            // Unique key for each item
            key_gen: move |item: String| item.clone(),
            // How to render each item in the list/overlay
            render: move |item: String| rsx! {
                div { "{item}" }
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
editable content. Sorting is vertical and updates `items` while dragging. Item
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

### Examples
Check the [examples](./examples) folder for more.

### License
MIT or Apache-2.0
