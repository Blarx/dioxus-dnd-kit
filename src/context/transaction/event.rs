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
/// Resolve `on_drop` requests with [`accept`](Self::accept) or
/// [`reject`](Self::reject). Dropping the last unresolved event rejects it.
/// Async Dioxus callbacks can own this event directly. While pending, the whole
/// DraggableView tree rejects new pickups. Without `on_drop`, moves accept immediately.
///
/// An accepted event can be retained for an Undo button. Undo is valid until the
/// next changed drop in the view, or either participating list unmounts. Perform
/// any reverse server request in the application before calling [`undo`](Self::undo).
#[derive(Clone)]
pub struct DropEvent<T: DndItem> {
    pub change: DropChange<T>,
    pub(super) control: Rc<Control>,
}

impl<T: DndItem> DropEvent<T> {
    /// Monotonically increasing operation ID within this DraggableView.
    pub fn id(&self) -> u64 {
        self.control.id
    }
    pub fn status(&self) -> DropStatus {
        self.control.status.get()
    }
    pub fn accept(&self) -> DropResult {
        self.decide(Decision::Accept)
    }
    /// Restore just this item. A conflicting position or missing list returns
    /// Conflict/Unavailable and unlocks the view; refresh authoritative data then.
    pub fn reject(&self) -> DropResult {
        self.decide(Decision::Reject)
    }
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
