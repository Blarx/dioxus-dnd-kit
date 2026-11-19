use crate::{
    DndItem,
    context::list::{ListAccess, ListRegistry, move_item},
    context::{DragCoordinator, DragEnd},
};
use dioxus::prelude::*;
use std::{
    cell::Cell,
    rc::{Rc, Weak},
};

mod event;
mod state;
mod types;

pub use event::DropEvent;
use event::{Control, Decision};
pub use state::{DndState, use_dnd_state};
pub use types::*;

pub(crate) struct Options<T: DndItem> {
    pub on_drop: Option<Callback<DropEvent<T>>>,
    pub can_drop: Option<Callback<DropChange<T>, bool>>,
    pub on_drag_start: Option<Callback<DragStart<T>>>,
    pub on_drag_cancel: Option<Callback<DragCancel<T>>>,
}
impl<T: DndItem> Copy for Options<T> {}
impl<T: DndItem> Clone for Options<T> {
    fn clone(&self) -> Self {
        *self
    }
}

#[derive(Clone)]
struct Started<T: DndItem> {
    generation: u64,
    start: DragStart<T>,
    list: ListAccess<T>,
    changed: bool,
}

#[derive(Clone)]
struct Record<T: DndItem> {
    id: u64,
    change: DropChange<T>,
    origin: ListAccess<T>,
    target: ListAccess<T>,
    order: Vec<String>,
    control: Weak<Control>,
}

/// Base DnD context for reversible list changes, from pickup through validation.
/// Geometry proposes destinations; this context applies changes and owns their
/// events, confirmation, rollback and invalidation.
pub(crate) struct DndTransactionContext<T: DndItem> {
    pub(crate) registry: ListRegistry<T>,
    active: Signal<Option<Started<T>>>,
    pending: Signal<Option<Record<T>>>,
    last: Signal<Option<Record<T>>>,
    next_id: Signal<u64>,
    options: Signal<Options<T>>,
    resolver: Signal<Option<Callback<(u64, Decision), DropResult>>>,
    pub state: DndState<T>,
    key_gen: Callback<T, String>,
    owner: ScopeId,
}
impl<T: DndItem> Copy for DndTransactionContext<T> {}
impl<T: DndItem> Clone for DndTransactionContext<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: DndItem> DndTransactionContext<T> {
    pub fn new(
        key_gen: Callback<T, String>,
        options: Options<T>,
        coordinator: DragCoordinator,
    ) -> Self {
        let pending = Signal::new(None);
        let mut transactions = Self {
            registry: ListRegistry::new(),
            active: Signal::new(None),
            pending,
            last: Signal::new(None),
            next_id: Signal::new(0),
            options: Signal::new(options),
            resolver: Signal::new(None),
            key_gen,
            owner: dioxus::core::current_scope_id(),
            state: DndState {
                pending,
                outcome: Signal::new(None),
                coordinator,
            },
        };
        let mut resolver = transactions;
        transactions
            .resolver
            .set(Some(Callback::new(move |(id, decision)| {
                resolver.resolve(id, decision)
            })));
        transactions
    }

    pub fn configure(&mut self, options: Options<T>) {
        self.options.set(options);
    }

    pub fn register(&mut self, list: ListAccess<T>) -> bool {
        self.registry.register(list)
    }

    pub fn can_start(&self) -> bool {
        self.pending.peek().is_none()
            && self.active.peek().is_none()
            && self.state.coordinator.available()
    }

    pub fn release_if_idle(&self) {
        if self.pending.peek().is_none() && self.active.peek().is_none() {
            self.state.coordinator.release(self.owner);
        }
    }

    pub fn begin(&mut self, key: String, generation: u64) -> bool {
        if !self.can_start() {
            return false;
        }
        let located = self.locate(&key);
        if !self.state.coordinator.acquire(self.owner) {
            return false;
        }
        // Custom low-level Draggables can have no registered list. They still
        // hold the nested-tree lease, but do not create a list transaction.
        let Some((list, index, values)) = located else {
            return true;
        };
        let start = DragStart {
            item: values[index].clone(),
            key,
            from: self.location(&list, &values, index),
        };
        self.active.set(Some(Started {
            generation,
            start: start.clone(),
            list,
            changed: false,
        }));
        let callback = self.options.peek().on_drag_start;
        if let Some(callback) = callback {
            callback.call(start);
        }
        true
    }

    pub fn mark_moved(&mut self) {
        self.active.with_mut(|active| {
            if let Some(active) = active {
                active.changed = true;
            }
        });
    }

    pub fn allows(&self, owner: ScopeId, index: usize) -> bool {
        let Some(started) = self.active.peek().clone() else {
            return true;
        };
        if self.options.peek().can_drop.is_none() {
            return true;
        }
        let Some(target) = self.registry.get(owner) else {
            return false;
        };
        let Some((_, actual, current)) = self.locate(&started.start.key) else {
            return false;
        };
        self.allows_candidate(&started, &target, index, current[actual].clone())
    }

    fn allows_candidate(
        &self,
        started: &Started<T>,
        target: &ListAccess<T>,
        index: usize,
        item: T,
    ) -> bool {
        let Some(callback) = self.options.peek().can_drop else {
            return true;
        };
        let Some(mut values) = target.items.read.call(()) else {
            return false;
        };
        values.retain(|value| self.key_gen.call(value.clone()) != started.start.key);
        let index = index.min(values.len());
        values.insert(index, item.clone());
        callback.call(DropChange {
            item,
            key: started.start.key.clone(),
            from: started.start.from.clone(),
            to: self.location(target, &values, index),
        })
    }

    /// The only preview mutation path for both local sorting and group transfers.
    /// Rollback deliberately bypasses eligibility checks.
    pub fn preview_move(&mut self, owner: ScopeId, index: usize) -> Result<bool, DragCancelReason> {
        let started = self
            .active
            .peek()
            .clone()
            .ok_or(DragCancelReason::Outside)?;
        let (source, from, mut values) = self
            .locate(&started.start.key)
            .ok_or(DragCancelReason::Outside)?;
        let target = self.registry.get(owner).ok_or(DragCancelReason::Outside)?;
        if !self.allows_candidate(&started, &target, index, values[from].clone()) {
            return Err(DragCancelReason::NotAllowed);
        }
        let moved = if source.items == target.items {
            let to = index.min(values.len() - 1);
            if from == to {
                return Ok(false);
            }
            let item = values.remove(from);
            values.insert(to, item);
            source.items.write.call(values)
        } else {
            move_item(
                source.items,
                target.items,
                &started.start.key,
                index,
                self.key_gen,
            )
        };
        if !moved {
            return Err(DragCancelReason::Outside);
        }
        self.mark_moved();
        Ok(true)
    }

    /// Adapts native handlers and the legacy public active signal to one lifecycle.
    pub fn observe_drag(
        &mut self,
        item: Option<T>,
        generation: u64,
        end: Option<DragEnd>,
        cancelled: bool,
    ) {
        if let Some(end) = end {
            self.finish_independent(end.generation, end.cancelled);
        }
        if let Some(item) = item {
            if self.can_start() {
                self.begin(self.key_gen.call(item), generation);
            }
        } else {
            // A caller may clear DndContext.active without producing DragEnd.
            // Groups perform their final hit test before finishing the transaction.
            let started = self.active.peek().clone();
            if let Some(started) = started
                && started.list.group.is_none()
            {
                self.finish(
                    started.generation,
                    cancelled.then_some(DragCancelReason::Cancelled),
                );
            }
            self.release_if_idle();
        }
    }

    /// Independent lists finish in the root; connected lists finish after their
    /// coordinator has processed the final pointer position and any rollback.
    pub fn finish_independent(&mut self, generation: u64, cancelled: bool) {
        let Some(started) = self.active.peek().clone() else {
            return;
        };
        if started.generation == generation && started.list.group.is_none() {
            self.finish(generation, cancelled.then_some(DragCancelReason::Cancelled));
        }
    }

    pub fn finish(&mut self, generation: u64, cancellation: Option<DragCancelReason>) {
        let Some(started) = self.active.peek().clone() else {
            return;
        };
        if started.generation != generation {
            return;
        }
        if let Some(reason) = cancellation {
            self.cancel(reason);
            return;
        }
        let Some((target, index, values)) = self.locate(&started.start.key) else {
            self.cancel(DragCancelReason::NotAllowed);
            return;
        };
        if !self.allows(target.owner, index) {
            self.cancel(DragCancelReason::NotAllowed);
            return;
        }
        self.active.set(None);
        if !started.changed
            || (target.items == started.list.items && index == started.start.from.index)
        {
            self.release_if_idle();
            return;
        }
        self.invalidate_last();
        let change = DropChange {
            item: values[index].clone(),
            key: started.start.key,
            from: started.start.from,
            to: self.location(&target, &values, index),
        };
        self.next_id.with_mut(|id| *id += 1);
        let id = *self.next_id.peek();
        let control = Rc::new(Control {
            id,
            status: Cell::new(DropStatus::Pending),
            resolve: Cell::new(*self.resolver.peek()),
        });
        self.pending.set(Some(Record {
            id,
            change: change.clone(),
            origin: started.list,
            target,
            order: self.keys(&values),
            control: Rc::downgrade(&control),
        }));
        let event = DropEvent { change, control };
        let callback = self.options.peek().on_drop;
        if let Some(callback) = callback {
            callback.call(event);
        } else {
            event.accept();
        }
    }

    fn resolve(&mut self, id: u64, decision: Decision) -> DropResult {
        let record = match decision {
            Decision::Accept | Decision::Reject => self.pending.peek().clone(),
            Decision::Undo => {
                if !self.can_start() {
                    return DropResult::Busy;
                }
                self.last.peek().clone()
            }
        };
        let Some(record) = record.filter(|record| record.id == id) else {
            return DropResult::Unavailable;
        };
        let (status, result) = match decision {
            Decision::Accept => (DropStatus::Accepted, DropResult::Applied),
            Decision::Reject | Decision::Undo => {
                let result = self.rollback_record(&record);
                let status = match result {
                    DropResult::Applied if matches!(decision, Decision::Undo) => DropStatus::Undone,
                    DropResult::Applied => DropStatus::Rejected,
                    DropResult::Conflict => DropStatus::Conflict,
                    _ => DropStatus::Invalidated,
                };
                (status, result)
            }
        };
        if let Some(control) = record.control.upgrade() {
            control.status.set(status);
            if status != DropStatus::Accepted {
                control.resolve.set(None);
            }
        }
        if matches!(decision, Decision::Undo) {
            self.last.set(None);
        } else {
            self.pending.set(None);
            self.release_if_idle();
            if status == DropStatus::Accepted {
                self.last.set(Some(record.clone()));
            }
        }
        self.state.outcome.set(Some(DropOutcome {
            change: record.change,
            status,
            result,
        }));
        result
    }

    fn rollback_record(&self, record: &Record<T>) -> DropResult {
        if !self.registered(&record.origin) || !self.registered(&record.target) {
            return DropResult::Unavailable;
        }
        let Some((list, index, values)) = self.locate(&record.change.key) else {
            return DropResult::Conflict;
        };
        if list.items != record.target.items {
            return DropResult::Conflict;
        }
        let order = self.keys(&values);
        let expected = record
            .order
            .iter()
            .position(|key| key == &record.change.key)
            .unwrap();
        // Ignore inserted/removed peers and edits to values, but never move an
        // item whose relative position was changed by authoritative data.
        for (old_index, key) in record.order.iter().enumerate() {
            if key == &record.change.key {
                continue;
            }
            if let Some(now) = order.iter().position(|candidate| candidate == key)
                && (old_index < expected) != (now < index)
            {
                return DropResult::Conflict;
            }
        }
        self.restore(&record.origin, &record.change.key, &record.change.from)
    }

    fn restore(&self, origin: &ListAccess<T>, key: &str, location: &DropLocation) -> DropResult {
        if !self.registered(origin) {
            return DropResult::Unavailable;
        }
        let Some((current, index, mut values)) = self.locate(key) else {
            return DropResult::Conflict;
        };
        let Some(mut original) = origin.items.read.call(()) else {
            return DropResult::Unavailable;
        };
        original.retain(|item| self.key_gen.call(item.clone()) != key);
        let keys = self.keys(&original);
        let insert = location
            .next_key
            .as_ref()
            .and_then(|key| keys.iter().position(|value| value == key))
            .or_else(|| {
                location
                    .previous_key
                    .as_ref()
                    .and_then(|key| keys.iter().position(|value| value == key))
                    .map(|index| index + 1)
            })
            .unwrap_or(location.index.min(original.len()));
        let applied = if current.items == origin.items {
            let item = values.remove(index);
            values.insert(insert, item);
            origin.items.write.call(values)
        } else {
            move_item(current.items, origin.items, key, insert, self.key_gen)
        };
        if applied {
            DropResult::Applied
        } else {
            DropResult::Conflict
        }
    }

    pub fn cancel(&mut self, reason: DragCancelReason) {
        let Some(started) = self.active.take() else {
            return;
        };
        let rollback = self.restore(&started.list, &started.start.key, &started.start.from);
        self.release_if_idle();
        let callback = self.options.peek().on_drag_cancel;
        if let Some(callback) = callback {
            callback.call(DragCancel {
                start: started.start,
                reason,
                rollback,
            });
        }
    }

    pub fn unregister(&mut self, owner: ScopeId) {
        let active = self.active.peek().clone();
        if let Some(active) = active
            && (active.list.owner == owner
                || self
                    .locate(&active.start.key)
                    .is_some_and(|(list, _, _)| list.owner == owner))
        {
            self.cancel(DragCancelReason::Unmounted);
        }
        let pending = self.pending.peek().clone();
        if let Some(record) = pending
            && (record.origin.owner == owner || record.target.owner == owner)
        {
            self.rollback_record(&record);
            self.invalidate(&record);
            self.pending.set(None);
            self.release_if_idle();
            self.state.outcome.set(Some(DropOutcome {
                change: record.change,
                status: DropStatus::Invalidated,
                result: DropResult::Unavailable,
            }));
        }
        let last = self.last.peek().clone();
        if last.is_some_and(|record| record.origin.owner == owner || record.target.owner == owner) {
            self.invalidate_last();
        }
        self.registry.unregister(owner);
    }

    pub fn shutdown(&mut self) {
        self.cancel(DragCancelReason::Unmounted);
        if let Some(record) = self.pending.take() {
            self.rollback_record(&record);
            self.invalidate(&record);
        }
        self.release_if_idle();
        self.invalidate_last();
        self.registry.clear();
    }

    fn invalidate(&self, record: &Record<T>) {
        if let Some(control) = record.control.upgrade() {
            control.status.set(DropStatus::Invalidated);
            control.resolve.set(None);
        }
    }
    fn invalidate_last(&mut self) {
        if let Some(record) = self.last.take() {
            self.invalidate(&record);
        }
    }
    fn registered(&self, expected: &ListAccess<T>) -> bool {
        self.registry.contains(expected)
    }
    fn keys(&self, items: &[T]) -> Vec<String> {
        items
            .iter()
            .map(|item| self.key_gen.call(item.clone()))
            .collect()
    }
    fn location(&self, list: &ListAccess<T>, items: &[T], index: usize) -> DropLocation {
        DropLocation {
            list_id: list.id.clone(),
            index,
            previous_key: index
                .checked_sub(1)
                .map(|index| self.key_gen.call(items[index].clone())),
            next_key: items
                .get(index + 1)
                .cloned()
                .map(|item| self.key_gen.call(item)),
        }
    }
    fn locate(&self, key: &str) -> Option<(ListAccess<T>, usize, Vec<T>)> {
        self.registry.locate(key, self.key_gen, None)
    }
}

#[cfg(test)]
mod tests;
