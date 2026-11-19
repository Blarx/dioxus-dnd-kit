use super::{DndTransactionContext, types::*};
use crate::{DndItem, context::DragCoordinator};
use dioxus::prelude::*;

/// Read-only reactive state of the nearest matching `DraggableView<T>`.
///
/// Obtain it through [`use_dnd_state`]. Methods subscribe the calling Dioxus
/// reactive scope to the corresponding state. Pending data and outcomes belong
/// to this view; [`is_busy`](Self::is_busy) includes its entire nested view tree,
/// even when parent and child views use different item types.
/// Keep this state handle within the owning view's lifetime. For asynchronous
/// decisions that may outlive the view, retain a [`DropEvent`](crate::DropEvent),
/// whose methods safely report invalidation after unmount.
pub struct DndState<T: DndItem> {
    pub(super) pending: Signal<Option<super::Record<T>>>,
    pub(super) outcome: Signal<Option<DropOutcome<T>>>,
    pub(super) coordinator: DragCoordinator,
}
impl<T: DndItem> Copy for DndState<T> {}
impl<T: DndItem> Clone for DndState<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: DndItem> DndState<T> {
    /// Whether a drag or pending validation holds the nested view tree's lock.
    ///
    /// Covers this view, its ancestors, and other descendants sharing that tree.
    /// Use it for controls that must stay disabled while either cards or their
    /// containing columns are being dragged or validated. Independent view trees
    /// do not share this lock.
    pub fn is_busy(&self) -> bool {
        self.coordinator.is_busy()
    }
    /// Whether this view has a released move awaiting acceptance or rejection.
    ///
    /// An active drag alone is not pending. A request belonging to another view
    /// in the same tree can make [`is_busy`](Self::is_busy) true while this remains false.
    pub fn is_pending(&self) -> bool {
        self.pending.read().is_some()
    }
    /// Snapshot of this view's pending move, or `None` when no decision is awaited.
    ///
    /// Returns the released change, rather than live item data that may have
    /// changed since release. Use the [`DropEvent`](crate::DropEvent) received by
    /// `on_drop` to resolve it; changing this returned snapshot has no effect.
    pub fn pending(&self) -> Option<DropChange<T>> {
        self.pending
            .read()
            .as_ref()
            .map(|record| record.change.clone())
    }
    /// Latest resolution or pending-request invalidation recorded by this view.
    ///
    /// Initially `None`. Acceptance, rejection, Undo, and invalidation of a pending
    /// request update it, including automatic acceptance when `on_drop` is absent.
    /// A new pending request leaves the previous outcome visible until resolution.
    /// Active-drag cancellation and unchanged drops do not replace it.
    ///
    /// This is a historical snapshot. Later invalidation of an accepted Undo
    /// handle does not rewrite its earlier outcome; consult the retained event's
    /// [`status`](crate::DropEvent::status) when acting on that handle.
    pub fn last_outcome(&self) -> Option<DropOutcome<T>> {
        self.outcome.read().clone()
    }
}

/// Read reactive transaction state below the nearest matching `DraggableView<T>`.
///
/// Call this hook in a descendant component, with the same item type as its view.
/// The provider is created by `DraggableView`, so its parent component cannot read
/// that view's state through this hook. See [`DndState`] for view-local pending
/// state and the shared busy state of nested views.
///
/// ```no_run
/// use dioxus::prelude::*;
/// use dioxus_dnd_kit::prelude::*;
///
/// // Render this component inside a DraggableView whose item type is u32.
/// #[component]
/// fn MoveStatus() -> Element {
///     let state = use_dnd_state::<u32>();
///     rsx! {
///         span {
///             if state.is_pending() {
///                 "Waiting for the API"
///             } else if state.is_busy() {
///                 "A drag or validation is active in this tree"
///             } else {
///                 "Ready"
///             }
///         }
///     }
/// }
/// ```
///
/// # Panics
///
/// Panics if no matching view context is available or it is called outside a
/// Dioxus component's hook context.
pub fn use_dnd_state<T: DndItem>() -> DndState<T> {
    use_context::<DndTransactionContext<T>>().state
}
