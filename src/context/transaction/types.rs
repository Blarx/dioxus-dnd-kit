use crate::DndItem;

/// A position in a list. Indices describe the order with the item in that list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DropLocation {
    /// The Droppable's stable HTML ID, or an empty string for an anonymous list.
    pub list_id: String,
    pub index: usize,
    /// Stable keys of the immediate neighbors, useful for order-based APIs.
    pub previous_key: Option<String>,
    pub next_key: Option<String>,
}

/// One completed move, including sorting within the same list.
#[derive(Clone, Debug, PartialEq)]
pub struct DropChange<T: DndItem> {
    /// Current item data at release. Rollback preserves subsequent data edits.
    pub item: T,
    pub key: String,
    /// Position at pickup, before any live preview transfers.
    pub from: DropLocation,
    pub to: DropLocation,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DragStart<T: DndItem> {
    pub item: T,
    pub key: String,
    pub from: DropLocation,
}

/// Why a drag ended without a drop request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragCancelReason {
    /// Escape, window blur, or an explicit cancellation of the drag context.
    Cancelled,
    Outside,
    /// The final candidate failed `can_drop`.
    NotAllowed,
    Unmounted,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DragCancel<T: DndItem> {
    pub start: DragStart<T>,
    pub reason: DragCancelReason,
    pub rollback: DropResult,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropStatus {
    Pending,
    Accepted,
    Rejected,
    Undone,
    /// A newer operation or unmount invalidated the handle.
    Invalidated,
    /// Authoritative data changed the moved item's position; no rollback applied.
    Conflict,
}

/// Result of an explicit transaction decision. These methods never force a stale rollback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropResult {
    Applied,
    AlreadyResolved,
    Conflict,
    Unavailable,
    /// Undo was requested while another drag or validation was in progress.
    Busy,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DropOutcome<T: DndItem> {
    pub change: DropChange<T>,
    pub status: DropStatus,
    pub result: DropResult,
}
