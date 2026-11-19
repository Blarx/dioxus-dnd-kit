use dioxus::prelude::*;

use crate::{
    DndItem, DragCancelReason,
    context::transaction::DndTransactionContext,
    context::{DndContext, DragEnd, DragPosition, DragSession},
};

use super::{
    geometry::{DropZone, insertion_index, target_zone},
    types::{GroupDrag, ListEntry, ListSnapshot, Preview},
};

/// Owns group membership, geometry and the accepted preview. The base DnD
/// context owns list data access, mutations and the transaction lifecycle.
pub(crate) struct ConnectedListsContext<T: DndItem> {
    pub(super) lists: Signal<Vec<ListEntry<T>>>,
    pub(super) drag: Signal<Option<GroupDrag>>,
    pub(super) preview: Signal<Option<Preview>>,
    dnd: DndContext<T>,
    session: DragSession<T>,
    pub(super) transactions: DndTransactionContext<T>,
    pub(super) owner: ScopeId,
}

impl<T: DndItem> Copy for ConnectedListsContext<T> {}
impl<T: DndItem> Clone for ConnectedListsContext<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: DndItem> ConnectedListsContext<T> {
    pub(super) fn new(
        dnd: DndContext<T>,
        session: DragSession<T>,
        transactions: DndTransactionContext<T>,
    ) -> Self {
        Self {
            lists: Signal::new(Vec::new()),
            drag: Signal::new(None),
            preview: Signal::new(None),
            dnd,
            session,
            transactions,
            owner: dioxus::core::current_scope_id(),
        }
    }

    pub(crate) fn belongs_to(&self, dnd: DndContext<T>) -> bool {
        self.dnd.active == dnd.active
    }

    pub(crate) fn shows_placeholder(&self, owner: ScopeId, key: &str) -> bool {
        self.preview
            .read()
            .as_ref()
            .is_some_and(|preview| preview.owner == owner && preview.key == key)
    }

    fn set_preview(&mut self, preview: Option<Preview>) {
        if *self.preview.peek() != preview {
            self.preview.set(preview);
        }
    }

    pub(crate) fn id(&self) -> ScopeId {
        self.owner
    }

    pub(crate) fn register(&mut self, entry: ListEntry<T>) {
        self.lists.with_mut(|lists| {
            lists.retain(|list| list.owner != entry.owner);
            lists.push(entry);
        });
    }

    pub(crate) fn publish(&mut self, owner: ScopeId, snapshot: ListSnapshot<T>) {
        let changed = self
            .lists
            .peek()
            .iter()
            .any(|list| list.owner == owner && list.snapshot != snapshot);
        if changed {
            self.lists.with_mut(|lists| {
                if let Some(list) = lists.iter_mut().find(|list| list.owner == owner) {
                    list.snapshot = snapshot;
                }
            });
        }
    }

    pub(crate) fn unregister(&mut self, owner: ScopeId) {
        // A child's cleanup can follow disposal of the group itself.
        let Ok(lists) = self.lists.try_peek().map(|lists| lists.clone()) else {
            return;
        };
        let drag = self.drag.peek().clone();
        if let Some(drag) = drag {
            let removes_drag = drag.owner == owner
                || self
                    .find_source(&lists, &drag.key)
                    .is_some_and(|(index, _)| lists[index].owner == owner);
            if removes_drag {
                self.cancel_drag(DragCancelReason::Unmounted);
            }
        }
        self.lists
            .with_mut(|lists| lists.retain(|list| list.owner != owner));
    }

    /// Called inside the provider's effect, so these reads subscribe that effect.
    pub(super) fn update(&mut self, on_drag_start: Option<Callback<T>>) {
        let active = (self.dnd.active)();
        let end = (self.session.last_end)();
        let cancelled = (self.session.cancelled)();
        let revision = (self.dnd.recalculate_rects)();
        let _ = self.lists.read();
        if active.is_none() && self.drag.peek().is_none() {
            self.set_preview(None);
            return;
        }
        let position = self.dnd.drag_position((self.dnd.mouse_pos)());
        self.finish_ended_drag(end, active.is_none(), position, cancelled, revision);
        if let Some(item) = active
            && let Some(key) = self.begin_drag(item, on_drag_start)
            && (self.dnd.is_floating_ready)()
        {
            self.preview_at(&key, position, revision).ok();
        }
    }

    fn finish_ended_drag(
        &mut self,
        end: Option<DragEnd>,
        inactive: bool,
        position: DragPosition,
        cancelled: bool,
        revision: u64,
    ) {
        let Some(drag) = self.drag.peek().clone() else {
            return;
        };
        let recorded = end.filter(|end| end.generation == drag.generation);
        if recorded.is_some() || inactive {
            let (position, cancelled) = recorded
                .map(|end| (end.position, end.cancelled))
                .unwrap_or((position, cancelled));
            self.finish_drag(position, cancelled, revision);
        }
    }

    fn begin_drag(&mut self, item: T, callback: Option<Callback<T>>) -> Option<String> {
        let key = self.dnd.key_gen.call(item.clone());
        if self.drag.peek().is_none() {
            let lists = self.lists.peek().clone();
            let Some((source, _)) = self.find_source(&lists, &key) else {
                self.set_preview(None);
                return None;
            };
            self.drag.set(Some(GroupDrag {
                generation: *self.session.generation.peek(),
                key: key.clone(),
                owner: lists[source].owner,
            }));
            if let Some(callback) = callback {
                callback.call(item);
            }
        }
        Some(key)
    }

    /// Both hover and release select and apply the same accepted candidate.
    pub(super) fn preview_at(
        &mut self,
        key: &str,
        position: DragPosition,
        revision: u64,
    ) -> Result<ScopeId, DragCancelReason> {
        let result = self.apply_candidate(key, position, revision);
        self.set_preview(result.as_ref().ok().map(|owner| Preview {
            owner: *owner,
            key: key.into(),
        }));
        result
    }

    fn apply_candidate(
        &mut self,
        key: &str,
        position: DragPosition,
        revision: u64,
    ) -> Result<ScopeId, DragCancelReason> {
        // Callbacks can update items; never keep a registry borrow across them.
        let lists = self.lists.peek().clone();
        let (source, source_index) = self
            .find_source(&lists, key)
            .ok_or(DragCancelReason::Outside)?;
        let zones = self.drop_zones(&lists, revision);
        let target =
            target_zone(&zones, position, Some(source)).ok_or(DragCancelReason::Outside)?;
        let index = if source == target {
            source_index
        } else {
            let snapshot = &lists[target].snapshot;
            insertion_index(
                &snapshot.item_rects,
                snapshot.axis.coordinate(position.mouse),
                snapshot.axis,
            )
            .ok_or(DragCancelReason::Outside)?
        };
        self.transactions.preview_move(lists[target].owner, index)?;
        Ok(lists[target].owner)
    }

    pub(super) fn finish_drag(&mut self, position: DragPosition, cancelled: bool, revision: u64) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        let reason = if cancelled {
            Some(DragCancelReason::Cancelled)
        } else {
            self.preview_at(&drag.key, position, revision).err()
        };
        self.transactions.finish(drag.generation, reason);
        self.set_preview(None);
    }

    fn cancel_drag(&mut self, reason: DragCancelReason) {
        self.drag.set(None);
        self.set_preview(None);
        self.transactions.cancel(reason);
        self.dnd.finish_drag(self.session, true);
    }

    pub(super) fn dispose(&mut self) {
        if self.drag.peek().is_some() {
            self.cancel_drag(DragCancelReason::Unmounted);
        }
    }

    pub(super) fn find_source(&self, lists: &[ListEntry<T>], key: &str) -> Option<(usize, usize)> {
        let (entry, index, _) =
            self.transactions
                .registry
                .locate(key, self.dnd.key_gen, Some(self.owner))?;
        lists
            .iter()
            .position(|list| list.owner == entry.owner)
            .map(|list| (list, index))
    }

    pub(super) fn drop_zones(&self, lists: &[ListEntry<T>], revision: u64) -> Vec<DropZone> {
        lists
            .iter()
            .map(|entry| {
                let snapshot = &entry.snapshot;
                let order_matches = self
                    .transactions
                    .registry
                    .get(entry.owner)
                    .and_then(|entry| entry.items.read.call(()))
                    .is_some_and(|items| {
                        items.len() == snapshot.values.len()
                            && items.iter().zip(&snapshot.values).all(|(item, measured)| {
                                self.dnd.key_gen.call(item.clone())
                                    == self.dnd.key_gen.call(measured.clone())
                            })
                    });
                DropZone {
                    rect: snapshot.rect,
                    ready: snapshot.ready
                        && order_matches
                        && snapshot.measured_revision == revision,
                }
            })
            .collect()
    }
}
