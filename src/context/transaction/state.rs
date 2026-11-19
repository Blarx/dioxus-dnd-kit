use super::{DndTransactionContext, types::*};
use crate::{DndItem, context::DragCoordinator};
use dioxus::prelude::*;

/// Reactive state shared by a DraggableView. Read it through [`use_dnd_state`].
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
    /// Any drag or pending validation in this nested view tree.
    pub fn is_busy(&self) -> bool {
        self.coordinator.is_busy()
    }
    pub fn is_pending(&self) -> bool {
        self.pending.read().is_some()
    }
    pub fn pending(&self) -> Option<DropChange<T>> {
        self.pending
            .read()
            .as_ref()
            .map(|record| record.change.clone())
    }
    pub fn last_outcome(&self) -> Option<DropOutcome<T>> {
        self.outcome.read().clone()
    }
}

/// Read validation state below a DraggableView, for spinners and disabled controls.
pub fn use_dnd_state<T: DndItem>() -> DndState<T> {
    use_context::<DndTransactionContext<T>>().state
}
