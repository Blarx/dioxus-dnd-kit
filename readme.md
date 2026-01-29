# dioxus-dnd-kit 🦀
A flexible, performant, and layout-stable Drag-and-Drop library for **Dioxus 0.7**.

Developed to handle smooth reordering, even when using JIT-engines like **Tailwind CSS**. It avoids layout flickering by keeping the DOM structure stable during drag operations.

### Features
- **Layout Stability**: No more jumping elements when an item is picked up.
- **Tailwind Friendly**: Handles dynamic style injections gracefully.
- **High Performance**: Uses `join_all` for batch rect measurements.
- **Customizable**: Full control over handlers, overlays, and placeholders.

### Installation
Add to your `Cargo.toml`:
```toml
[dependencies]
dioxus-dnd-kit = "0.1.0"
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

### Advanced: Tailwind & Dynamic Rects
If you use the Tailwind Play CDN or dynamic styles that change element sizes, `dioxus-dnd-kit` is ready for it. It uses a `MutationObserver` internally (or you can trigger `recalculate_rects`) to ensure coordinates are always fresh.

### Examples
Check the [examples](./examples) folder for more.

### License
MIT or Apache-2.0
