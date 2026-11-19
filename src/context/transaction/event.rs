use super::types::*;
use crate::DndItem;
use dioxus::prelude::*;
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy)]
pub(super) enum Decision {
    Accept,
    Reject,
    Undo,
}

pub(super) struct Control {
    pub(super) id: u64,
    pub(super) status: Cell<DropStatus>,
    pub(super) resolve: Cell<Option<Callback<(u64, Decision), DropResult>>>,
}

/// A reversible, optimistic drop request, safe to keep across an `await`.
///
/// `DraggableView::on_drop` receives one event after an item is released at a
/// changed, eligible position, for both local sorting and connected transfers.
/// The list signals already contain the optimistic move. Intermediate previews,
/// unchanged positions, and cancelled drags do not produce drop events.
///
/// Resolve the request with [`accept`](Self::accept) or [`reject`](Self::reject),
/// including after awaiting your API. Clones share the same decision and status;
/// resolving any clone resolves the request for all of them. Dropping the last
/// unresolved clone attempts rejection automatically. Retaining an unresolved
/// clone keeps the request pending and the nested view tree blocked. Without
/// `on_drop`, the crate accepts changed moves immediately.
///
/// | Current status | Available decision | Successful status |
/// | --- | --- | --- |
/// | [`Pending`](DropStatus::Pending) | [`accept()`](Self::accept) | [`Accepted`](DropStatus::Accepted) |
/// | [`Pending`](DropStatus::Pending) | [`reject()`](Self::reject) | [`Rejected`](DropStatus::Rejected) |
/// | [`Accepted`](DropStatus::Accepted) | [`undo()`](Self::undo) | [`Undone`](DropStatus::Undone) |
///
/// Rejection and Undo can instead end in [`Conflict`](DropStatus::Conflict) or
/// [`Invalidated`](DropStatus::Invalidated). Check their returned [`DropResult`].
/// Retain an accepted event for an Undo button. Its handle stays valid until the
/// next changed drop in the same view, either participating list unmounts, or the
/// view unmounts. A new drag without a changed drop does not invalidate it.
/// Applications perform server requests themselves; decision methods only manage
/// local list state and do not send or cancel network requests.
#[derive(Clone)]
pub struct DropEvent<T: DndItem> {
    /// Released item and source/destination snapshots for your application's API.
    /// Editing this field does not change list signals or the recorded transaction.
    pub change: DropChange<T>,
    pub(super) control: Rc<Control>,
}

impl<T: DndItem> DropEvent<T> {
    /// Monotonically increasing operation ID within this `DraggableView`.
    /// Shared by all clones; it is not a globally unique application ID.
    pub fn id(&self) -> u64 {
        self.control.id
    }
    /// Current status shared by all clones of this request.
    ///
    /// Reading this method does not subscribe a Dioxus component to changes.
    /// Use [`use_dnd_state`](crate::use_dnd_state) for reactive pending/outcome UI.
    pub fn status(&self) -> DropStatus {
        self.control.status.get()
    }
    /// Confirm a pending optimistic move and release its validation lock.
    ///
    /// Does not rewrite current list data or call your API. On success returns
    /// [`Applied`](DropResult::Applied) and sets [`Accepted`](DropStatus::Accepted).
    /// Retain this event if the application should offer Undo.
    /// Already resolved requests return [`AlreadyResolved`](DropResult::AlreadyResolved);
    /// invalidated handles return [`Unavailable`](DropResult::Unavailable).
    pub fn accept(&self) -> DropResult {
        self.decide(Decision::Accept)
    }
    /// Reject a pending move and attempt to restore only the moved item.
    ///
    /// Restoration uses surviving source neighbors, then the original index,
    /// while preserving current item data and unrelated list edits. Returns
    /// [`Applied`](DropResult::Applied) with status [`Rejected`](DropStatus::Rejected)
    /// when restoration succeeds. An externally changed position or missing item
    /// can return [`Conflict`](DropResult::Conflict); unavailable handles or lists
    /// return [`Unavailable`](DropResult::Unavailable). Refresh authoritative data
    /// in those cases. The request no longer holds its validation lock after
    /// resolution, even when restoration fails.
    ///
    /// Already resolved requests return [`AlreadyResolved`](DropResult::AlreadyResolved).
    /// To reverse an accepted move, use [`undo`](Self::undo). Rejection does not
    /// emit `on_drag_cancel`; its outcome is exposed through [`DndState`](crate::DndState).
    pub fn reject(&self) -> DropResult {
        self.decide(Decision::Reject)
    }
    /// Reverse an accepted move once, using the same restoration checks as rejection.
    ///
    /// Returns [`Applied`](DropResult::Applied) with status [`Undone`](DropStatus::Undone)
    /// on success. If another drag or validation is active in the nested view tree,
    /// returns [`Busy`](DropResult::Busy) and leaves this event accepted; retry only
    /// if it remains valid. A newer changed drop in the same view or a participating
    /// list/view unmount invalidates the handle and returns [`Unavailable`](DropResult::Unavailable).
    /// External position changes can return [`Conflict`](DropResult::Conflict).
    /// Other inapplicable or repeated decisions return [`AlreadyResolved`](DropResult::AlreadyResolved).
    ///
    /// If Undo also needs a server update, await that request in the application
    /// before calling this method, then check its result: the handle may have
    /// become busy, conflicted, or invalidated during the wait.
    pub fn undo(&self) -> DropResult {
        self.decide(Decision::Undo)
    }

    fn decide(&self, decision: Decision) -> DropResult {
        let applicable = matches!(
            (self.status(), decision),
            (DropStatus::Pending, Decision::Accept | Decision::Reject)
                | (DropStatus::Accepted, Decision::Undo)
        );
        if !applicable {
            return if self.status() == DropStatus::Invalidated {
                DropResult::Unavailable
            } else {
                DropResult::AlreadyResolved
            };
        }
        self.control
            .resolve
            .get()
            .map(|resolve| resolve.call((self.control.id, decision)))
            .unwrap_or(DropResult::Unavailable)
    }
}

impl<T: DndItem> Drop for DropEvent<T> {
    fn drop(&mut self) {
        if Rc::strong_count(&self.control) == 1 && self.status() == DropStatus::Pending {
            self.reject();
        }
    }
}
