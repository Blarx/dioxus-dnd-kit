//! Sortable lists for **Dioxus 0.7**, with relative or absolute item layout.
//!
//! Import [`prelude`] to use the components, hooks, and data types together.
//! [`DragLayout`] and [`DndItem`] are also available directly at the crate root;
//! the prelude re-exports the same types.
//!
//! # Quick start
//!
//! A [`DraggableView`](prelude::DraggableView) provides the shared drag context
//! and fixed overlay. A [`Droppable`](prelude::Droppable) renders and reorders
//! its `Signal<Vec<T>>`. Include a [`DraggableHandler`](prelude::DraggableHandler)
//! in the item renderer to provide a grab target.
//!
//! ```no_run
//! use dioxus::prelude::*;
//! use dioxus_dnd_kit::prelude::*;
//!
//! #[derive(Clone, PartialEq)]
//! struct Task {
//!     id: u32,
//!     title: String,
//! }
//!
//! #[component]
//! fn App() -> Element {
//!     let items = use_signal(|| vec![
//!         Task { id: 1, title: "First task".into() },
//!         Task { id: 2, title: "Second task".into() },
//!     ]);
//!
//!     rsx! {
//!         DraggableView {
//!             key_gen: |task: Task| task.id.to_string(),
//!             render: |task: Task| rsx! {
//!                 article {
//!                     DraggableHandler { item: task.clone(), "Drag" }
//!                     span { "{task.title}" }
//!                 }
//!             },
//!             Droppable { items, layout: DragLayout::Relative }
//!         }
//!     }
//! }
//!
//! fn main() {
//!     launch(App);
//! }
//! ```
//!
//! # Choosing a layout
//!
//! - [`DragLayout::Absolute`] is the default and preserves the original layout:
//!   item content is absolutely positioned over a measured placeholder.
//! - [`DragLayout::Relative`] keeps item content in normal flow, so its height
//!   follows its content. During dragging, the original stays mounted and hidden,
//!   preserving its space while the overlay follows the pointer.
//!
//! The overlay is always fixed, regardless of the list layout. An optional
//! `placeholder_render` callback on [`DraggableView`](prelude::DraggableView)
//! receives `(height, width)` in pixels. In relative mode its output covers the
//! reserved slot without contributing another row height; use
//! `width: 100%; height: 100%` to fill that slot.
//!
//! # Item identity and geometry
//!
//! Item data only needs to implement [`DndItem`]; the blanket implementation
//! covers any `PartialEq + Clone + 'static` type. Generate unique, stable keys
//! from item IDs, not list indices or editable text. Reordering updates `items`
//! while dragging.
//!
//! Item resize, viewport resize, and scrolling during a drag refresh collision
//! geometry. For other layout changes, increment `recalculate_rects` through
//! [`use_dnd_context`](prelude::use_dnd_context). The overlay keeps its pickup
//! dimensions until release.
//!
//! # API and examples
//!
//! Most applications only need [`DraggableView`](prelude::DraggableView),
//! [`Droppable`](prelude::Droppable), and
//! [`DraggableHandler`](prelude::DraggableHandler). The lower-level
//! [`Draggable`](prelude::Draggable), [`DraggableOverlay`](prelude::DraggableOverlay),
//! and context hooks are available in [`prelude`].
//!
//! Sorting currently supports a mouse and a vertical list. Transfers between
//! lists, touch input, and keyboard dragging are not implemented.
//!
//! The [repository examples](https://github.com/Blarx/dioxus-dnd-kit/tree/main/examples)
//! demonstrate both layouts:
//!
//! - `relative_list`: natural heights, expandable cards, and nested scrolling.
//! - `simple_list`: absolute layout with Tailwind.
//! - `resizable_list`: absolute layout with explicit geometry recalculation.
//!
//! Run an example from the repository with `dx serve --example relative_list --web`.
//!
//! See the [changelog](https://github.com/Blarx/dioxus-dnd-kit/blob/main/CHANGELOG.md)
//! for release history and compatibility notes.

mod context;
mod draggable;
mod droppable;
mod hooks;

/// How list items participate in document layout. The drag overlay is always fixed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DragLayout {
    /// Preserve the original absolute content and measured placeholder layout.
    #[default]
    Absolute,
    /// Keep content in normal flow, including while its overlay is being dragged.
    Relative,
}

/// Item data accepted by the drag-and-drop components.
///
/// Implemented automatically for every `PartialEq + Clone + 'static` type; no
/// manual implementation is needed. Use a stable ID in `DraggableView::key_gen`
/// to preserve identity when the item's editable data changes.
pub trait DndItem: PartialEq + Clone + 'static {}

impl<T: PartialEq + Clone + 'static> DndItem for T {}

/// Common imports for building a sortable list.
///
/// ```
/// use dioxus_dnd_kit::prelude::*;
///
/// let layout = DragLayout::Relative;
/// ```
///
/// Re-exports the components, context hooks, [`DragLayout`], and [`DndItem`].
/// The types re-exported here are the same types as at their original paths.
pub mod prelude {
    pub use crate::context::DndContext;
    pub use crate::context::DraggableView;
    pub use crate::draggable::{Draggable, DraggableHandler, DraggableOverlay};
    pub use crate::droppable::Droppable;
    pub use crate::hooks::{
        use_body_on_mouse_up, use_dnd_context, use_droppable_context, use_window_on_resize,
    };
    pub use crate::{DndItem, DragLayout};
}
