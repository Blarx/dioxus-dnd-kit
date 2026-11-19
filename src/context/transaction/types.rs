use crate::DndItem;

/// An item's position and immediate neighbors in a list.
///
/// Indices and neighbors describe the order with the item present. In
/// [`DropChange::from`] they refer to pickup; in [`DropChange::to`] they refer to
/// the proposed or released position. Neighbor keys help order-based APIs avoid
/// depending on indices alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DropLocation {
    /// The Droppable's stable HTML ID, or an empty string for an anonymous list.
    pub list_id: String,
    /// Zero-based position of the item in this list.
    pub index: usize,
    /// Stable key of the preceding item, or `None` at the start of the list.
    pub previous_key: Option<String>,
    /// Stable key of the following item, or `None` at the end of the list.
    pub next_key: Option<String>,
}

/// A proposed or released move, including sorting within the same list.
///
/// `DraggableView::can_drop` receives a candidate before it is applied.
/// [`DropEvent`](crate::DropEvent) contains the released move after the optimistic
/// list update. The source always describes pickup, even after several preview
/// transfers. List positions do not encode application concepts such as time,
/// duration, or resource capabilities; map them in your application.
#[derive(Clone, Debug, PartialEq)]
pub struct DropChange<T: DndItem> {
    /// Current item data when the candidate or drop is constructed.
    /// Rollback uses the item's current data rather than restoring this snapshot.
    pub item: T,
    /// Stable identity produced by the view's `key_gen` callback.
    pub key: String,
    /// Position at pickup, before any live preview transfers.
    pub from: DropLocation,
    /// Proposed position for `can_drop`, or released position for a drop event.
    pub to: DropLocation,
}

/// Pickup metadata passed to `DraggableView::on_drag_start`.
///
/// Emitted once for a registered-list drag, before preview changes. The original
/// position remains the source of its later [`DropChange`] or [`DragCancel`].
#[derive(Clone, Debug, PartialEq)]
pub struct DragStart<T: DndItem> {
    /// Item data captured at pickup.
    pub item: T,
    /// Stable identity produced by the view's `key_gen` callback.
    pub key: String,
    /// Position before any reordering or transfer.
    pub from: DropLocation,
}

/// Why a drag ended without a drop request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragCancelReason {
    /// Escape, window blur, or another explicit cancellation of the physical drag.
    Cancelled,
    /// No usable connected-list destination could be selected or applied at release.
    Outside,
    /// The final move could not be validated, for example because `can_drop`
    /// denied it or the active item could not be uniquely located.
    NotAllowed,
    /// The view, connected group, or a list participating in the drag unmounted.
    Unmounted,
}

/// A drag cancelled before a drop request, passed to `DraggableView::on_drag_cancel`.
///
/// The crate attempts to restore the pickup position before calling the handler.
/// Check [`rollback`](Self::rollback) before assuming that restoration succeeded.
/// Rejecting an existing [`DropEvent`](crate::DropEvent) is a transaction decision
/// reported through [`DropOutcome`], rather than another drag-cancellation event.
#[derive(Clone, Debug, PartialEq)]
pub struct DragCancel<T: DndItem> {
    /// Original pickup metadata, including the source list and position.
    pub start: DragStart<T>,
    /// Why the drag ended without a drop request.
    pub reason: DragCancelReason,
    /// Result of the restoration attempt; `Applied` also covers an unchanged position.
    pub rollback: DropResult,
}

/// Current lifecycle state of a [`DropEvent`](crate::DropEvent).
///
/// A pending event can be accepted or rejected. An accepted event can be undone
/// once while valid. Other states are terminal. A decision's [`DropResult`]
/// describes that particular call, while this status describes the event itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropStatus {
    /// The optimistic move is awaiting an application decision and blocks new pickups.
    Pending,
    /// The move was confirmed. The event may be retained for Undo.
    Accepted,
    /// The pending move was rejected and the item was restored successfully.
    Rejected,
    /// The accepted move was undone and the item was restored successfully.
    Undone,
    /// A newer changed drop, participating-list unmount, or view unmount invalidated
    /// the handle, or restoration became unavailable. Decisions can no longer apply.
    Invalidated,
    /// Rejection or Undo could not safely restore the item, for example after an
    /// authoritative relocation or deletion. The conflicting data was left intact.
    Conflict,
}

/// Result of `DropEvent::accept`, `reject`, `undo`, or a drag-restoration attempt.
///
/// Failure results do not force a stale rollback. In particular, refresh
/// authoritative data after a conflict rather than replacing whole lists with
/// an old snapshot. Inspect [`DropStatus`] for the event's resulting state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropResult {
    /// The decision succeeded: acceptance confirmed the optimistic move, or
    /// rejection, Undo, or cancellation restored the item.
    Applied,
    /// This decision does not apply to the event's current status, for example
    /// accepting twice or requesting Undo before acceptance. Nothing changed.
    AlreadyResolved,
    /// The item could not be safely restored, for example because its position or
    /// identity changed externally. No stale restoration was forced.
    Conflict,
    /// The event, owning view, or required list is no longer available.
    Unavailable,
    /// Undo was requested during a drag or pending validation in the nested view
    /// tree. The accepted event is unchanged and can be retried if it remains valid.
    Busy,
}

/// Snapshot of the latest transaction resolution or pending-request invalidation.
///
/// Available through [`DndState::last_outcome`](crate::DndState::last_outcome).
/// This records an outcome at that time; a retained event's status can change
/// later, for example when a newer changed drop invalidates its Undo handle.
#[derive(Clone, Debug, PartialEq)]
pub struct DropOutcome<T: DndItem> {
    /// The original released move, even when it was subsequently rolled back.
    pub change: DropChange<T>,
    /// Event status when this outcome was recorded.
    pub status: DropStatus,
    /// Result of the decision or invalidation that produced this outcome.
    pub result: DropResult,
}
