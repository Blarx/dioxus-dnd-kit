//! Sortable and connected lists for **Dioxus 0.7**, with relative or absolute layout.
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
//! Sorting defaults to [`DragAxis::Vertical`]. Set `axis: DragAxis::Horizontal`
//! on a Droppable to reorder columns; use matching flex/grid CSS for the container.
//! Connected transfers follow the destination list's axis.
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
//! # Connecting lists
//!
//! Wrap participating lists in [`ConnectedLists`](prelude::ConnectedLists),
//! inside the same [`DraggableView`](prelude::DraggableView). Assign distinct,
//! stable HTML `id` values and a separate item signal to each list. Transfers update
//! both signals live; empty containers work as long as CSS gives them a drop area.
//!
//! ```no_run
//! use dioxus::prelude::*;
//! use dioxus_dnd_kit::prelude::*;
//!
//! #[component]
//! fn Board() -> Element {
//!     let todo = use_signal(|| vec![1_u32, 2]);
//!     let done = use_signal(Vec::<u32>::new);
//!     rsx! {
//!         DraggableView {
//!             key_gen: |id: u32| id.to_string(),
//!             render: |id: u32| rsx! {
//!                 DraggableHandler { item: id, "Task {id}" }
//!             },
//!             ConnectedLists::<u32> {
//!                 Droppable { id: "todo", class: "column", items: todo, layout: DragLayout::Relative }
//!                 Droppable { id: "done", class: "column", items: done, layout: DragLayout::Relative }
//!             }
//!         }
//!     }
//! }
//! ```
//!
//! Give `.column` a non-zero `min-height` and suitable width in your CSS. Each
//! list may use either layout. Lists outside the group, or without an `id`,
//! remain independent. Keys must be unique across the entire `DraggableView`.
//! Container IDs must be unique in the HTML document. Drop areas respect viewport
//! bounds and clipping by scrollable ancestors.
//!
//! The highlighted placeholder marks an accepted drop, even if the handle is
//! outside the list. Targets prefer the pointer, then the greatest card overlap
//! covering at least 25% of the smaller visible surface. Hover and release use
//! the same rule. Moving away from all targets hides the preview while preserving
//! its reserved space. Escape, window blur, or release without an accepted target
//! restores the item to its original list and index. Rollback moves only that
//! item, preserving unrelated edits to other items. When transferring, the original content is
//! remounted in the destination: keep persistent UI state in item data or in a
//! parent keyed by item ID.
//!
//! # Validating and saving a drop
//!
//! Configure `on_drop` on [`DraggableView`](prelude::DraggableView) for both
//! same-list sorting and cross-list transfers. It fires once for a changed,
//! accepted drop, never for hover updates, cancellation, or a no-op. Give lists
//! stable `id` values to identify them in your API; anonymous lists report `""`.
//!
//! ```no_run
//! use dioxus::prelude::*;
//! use dioxus_dnd_kit::prelude::*;
//!
//! async fn save_order(_change: DropChange<u32>) -> Result<(), ()> {
//!     // Send your application's API request here.
//!     Ok(())
//! }
//!
//! #[component]
//! fn ValidatedList() -> Element {
//!     let items = use_signal(|| vec![1_u32, 2, 3]);
//!     rsx! {
//!         DraggableView {
//!             key_gen: |id: u32| id.to_string(),
//!             render: |id: u32| rsx! { DraggableHandler { item: id, "Task {id}" } },
//!             on_drop: move |event: DropEvent<u32>| async move {
//!                 if save_order(event.change.clone()).await.is_ok() {
//!                     event.accept();
//!                 } else {
//!                     let result = event.reject();
//!                     // Conflict means authoritative data moved/deleted the item:
//!                     // refresh it rather than forcing an outdated rollback.
//!                     if result == DropResult::Conflict { /* refresh */ }
//!                 }
//!             },
//!             Droppable { id: "tasks", items, layout: DragLayout::Relative }
//!         }
//!     }
//! }
//! ```
//!
//! [`DropChange`] contains current item data, a stable key, and `from`/`to`
//! locations with list IDs, indices, and neighboring keys. `from` always refers
//! to pickup, even after several preview transfers. `can_drop` provides a fast,
//! synchronous check for candidate positions; asynchronous business validation
//! belongs in `on_drop`. The crate sorts lists; applications map these positions
//! to appointment times, resource IDs, durations, and server versions.
//!
//! Moves stay optimistic while awaiting a decision. Further pickups in the same
//! view are blocked until resolution; [`use_dnd_state`] exposes pending state and
//! the last outcome. Resolve every request. Dropping its last event handle rejects
//! it automatically, including when an async handler is cancelled.
//! Without `on_drop`, changes accept immediately as before.
//!
//! Rejection moves only the item back, using surviving origin neighbors and
//! preserving other inserts, removals, and data edits. If authoritative data
//! moves or deletes the item while awaiting a response, rejection returns
//! [`DropResult::Conflict`] and unlocks the view without overwriting that data.
//! Unmounting a participating list or the view invalidates outstanding handles;
//! late decisions are harmless. Network request cancellation remains the
//! application's responsibility.
//!
//! Keep an accepted [`DropEvent`] for an Undo button and call [`DropEvent::undo`].
//! It uses the same conflict checks and remains valid until the next changed
//! drop, or a participating list unmounts. Applications perform any reverse
//! server request themselves, then handle the returned result. `on_drag_start`
//! and `on_drag_cancel` on the view expose pickup and cancellation metadata.
//!
//! # Nested sorting
//!
//! A board can have an outer `DraggableView<Task>` and `ConnectedLists<Task>`,
//! containing a `DraggableView<Column>` with one horizontal Droppable. Each column's
//! renderer contains a Task Droppable. Keep column IDs stable and task signals in
//! their common parent, so moving a keyed column preserves its contents and state.
//!
//! Use the optional `overlay_render` on the column view to render a **passive**
//! snapshot. It must not register copies of nested Droppables or duplicate HTML
//! IDs. The original column stays mounted while its overlay moves. See the
//! `connected_lists` example for this arrangement and separate Undo controls.
//!
//! Nested views isolate their typed drag sessions and group membership, while
//! sharing a single drag/validation lease. A card cannot start moving while a
//! column is being dragged or saved, and vice versa. [`DndState::is_busy`] observes
//! this entire tree; `is_pending()` describes the current view's own request.
//! Independent view trees keep separate leases and operation histories.
//!
//! # API and examples
//!
//! Most applications only need [`DraggableView`](prelude::DraggableView),
//! [`Droppable`](prelude::Droppable), and
//! [`DraggableHandler`](prelude::DraggableHandler). The lower-level
//! [`Draggable`](prelude::Draggable), [`DraggableOverlay`](prelude::DraggableOverlay),
//! and context hooks are available in [`prelude`].
//!
//! Sorting currently supports mouse input and linear ordering within each
//! list. Connected containers may be placed side by side. Touch input and
//! keyboard dragging are not implemented.
//!
//! The [repository examples](https://github.com/Blarx/dioxus-dnd-kit/tree/main/examples)
//! demonstrate both layouts:
//!
//! - `relative_list`: natural heights, expandable cards, and nested scrolling.
//! - `simple_list`: absolute layout with Tailwind.
//! - `resizable_list`: absolute layout with explicit geometry recalculation.
//! - `connected_lists`: a three-column board, live transfers, cancellation,
//!   empty destinations, and an independent legacy list.
//!
//! Run an example from the repository with `dx serve --example relative_list --web`.
//!
//! See the [changelog](https://github.com/Blarx/dioxus-dnd-kit/blob/main/CHANGELOG.md)
//! for release history and compatibility notes.

mod connected;
mod context;
mod draggable;
mod droppable;
mod hooks;

pub use context::transaction::{
    DndState, DragCancel, DragCancelReason, DragStart, DropChange, DropEvent, DropLocation,
    DropOutcome, DropResult, DropStatus, use_dnd_state,
};

/// How list items participate in document layout. The drag overlay is always fixed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DragLayout {
    /// Preserve the original absolute content and measured placeholder layout.
    #[default]
    Absolute,
    /// Keep content in normal flow, including while its overlay is being dragged.
    Relative,
}

/// Primary sorting direction. The container's CSS still determines its layout.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DragAxis {
    #[default]
    Vertical,
    Horizontal,
}

impl DragAxis {
    pub(crate) fn coordinate(self, point: dioxus::html::geometry::ClientPoint) -> f64 {
        match self {
            Self::Vertical => point.y,
            Self::Horizontal => point.x,
        }
    }
    pub(crate) fn midpoint(self, rect: dioxus::html::geometry::PixelsRect) -> f64 {
        match self {
            Self::Vertical => rect.min_y() + rect.height() / 2.0,
            Self::Horizontal => rect.min_x() + rect.width() / 2.0,
        }
    }
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
    pub use crate::connected::ConnectedLists;
    pub use crate::context::DndContext;
    pub use crate::context::DraggableView;
    pub use crate::draggable::{Draggable, DraggableHandler, DraggableOverlay};
    pub use crate::droppable::Droppable;
    pub use crate::hooks::{
        use_body_on_mouse_up, use_dnd_context, use_droppable_context, use_window_on_resize,
    };
    pub use crate::{
        DndItem, DndState, DragAxis, DragCancel, DragCancelReason, DragLayout, DragStart,
        DropChange, DropEvent, DropLocation, DropOutcome, DropResult, DropStatus, use_dnd_state,
    };
}
