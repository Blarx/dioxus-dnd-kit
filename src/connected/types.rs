use dioxus::{html::geometry::PixelsRect, prelude::*};

use crate::{DndItem, DragAxis};

#[derive(Clone, PartialEq)]
pub(crate) struct ListSnapshot<T: DndItem> {
    pub values: Vec<T>,
    pub rect: Option<PixelsRect>,
    pub item_rects: Vec<Option<PixelsRect>>,
    pub ready: bool,
    pub measured_revision: u64,
    pub axis: DragAxis,
}

#[derive(Clone)]
pub(crate) struct ListEntry<T: DndItem> {
    pub owner: ScopeId,
    pub snapshot: ListSnapshot<T>,
}

/// Group membership during a drag; transaction origin and rollback live in DndTransactionContext.
#[derive(Clone)]
pub(super) struct GroupDrag {
    pub generation: u64,
    pub key: String,
    pub owner: ScopeId,
}

#[derive(Clone, PartialEq)]
pub(super) struct Preview {
    pub owner: ScopeId,
    pub key: String,
}

/// Identity of the nearest Droppable. Group membership is checked separately.
#[derive(Clone, Copy)]
pub(crate) struct ListIdentity {
    pub owner: ScopeId,
    pub id: ReadSignal<String>,
}
