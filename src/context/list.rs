use dioxus::prelude::*;

use crate::DndItem;

/// Data callbacks run in the registering Droppable's scope. The item signal may
/// belong to a child column, so the base context accesses it through callbacks.
/// Keep the raw signal only for identity comparisons, never for access.
pub(crate) struct ItemAccess<T: DndItem> {
    identity: Signal<Vec<T>>,
    pub(crate) read: Callback<(), Option<Vec<T>>>,
    pub(crate) write: Callback<Vec<T>, bool>,
}

impl<T: DndItem> Copy for ItemAccess<T> {}
impl<T: DndItem> Clone for ItemAccess<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: DndItem> PartialEq for ItemAccess<T> {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
}

impl<T: DndItem> From<Signal<Vec<T>>> for ItemAccess<T> {
    fn from(mut items: Signal<Vec<T>>) -> Self {
        Self {
            identity: items,
            read: Callback::new(move |()| items.try_peek().ok().map(|items| items.clone())),
            write: Callback::new(move |value| {
                let Ok(mut items) = items.try_write() else {
                    return false;
                };
                *items = value;
                true
            }),
        }
    }
}

pub(crate) fn move_item<T: DndItem>(
    from: impl Into<ItemAccess<T>>,
    to: impl Into<ItemAccess<T>>,
    key: &str,
    index: usize,
    key_gen: Callback<T, String>,
) -> bool {
    let from = from.into();
    let to = to.into();
    if from == to {
        return false;
    }
    let Some(mut source) = from.read.call(()) else {
        return false;
    };
    let Some(from_index) = source
        .iter()
        .position(|item| key_gen.call(item.clone()) == key)
    else {
        return false;
    };
    let Some(mut target) = to.read.call(()) else {
        return false;
    };
    if target.iter().any(|item| key_gen.call(item.clone()) == key) {
        return false;
    }
    let original = source.clone();
    let item = source.remove(from_index);
    let index = index.min(target.len());
    target.insert(index, item);
    // These library callbacks only assign signals, in their owning scopes. No
    // await or user code runs between assignments; renders see a complete move.
    if !from.write.call(source) {
        return false;
    }
    if !to.write.call(target) {
        from.write.call(original);
        return false;
    }
    true
}

#[derive(Clone)]
pub(crate) struct ListAccess<T: DndItem> {
    pub owner: ScopeId,
    pub id: String,
    pub items: ItemAccess<T>,
    pub group: Option<ScopeId>,
}

/// The view owns list identities and scoped data access. Connected groups keep
/// only their geometry and refer to these entries by owner.
pub(crate) struct ListRegistry<T: DndItem> {
    entries: Signal<Vec<ListAccess<T>>>,
}

impl<T: DndItem> Copy for ListRegistry<T> {}
impl<T: DndItem> Clone for ListRegistry<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: DndItem> ListRegistry<T> {
    pub fn new() -> Self {
        Self {
            entries: Signal::new(Vec::new()),
        }
    }

    pub fn register(&mut self, entry: ListAccess<T>) -> bool {
        let conflict = entry.group.is_some()
            && self.entries.peek().iter().any(|list| {
                list.owner != entry.owner
                    && list.group == entry.group
                    && (list.id == entry.id || list.items == entry.items)
            });
        if conflict {
            dioxus::logger::tracing::error!(
                "ConnectedLists: list IDs and item signals must be distinct: {}",
                entry.id
            );
            return false;
        }
        self.entries.with_mut(|lists| {
            lists.retain(|list| list.owner != entry.owner);
            lists.push(entry);
        });
        true
    }

    pub fn get(&self, owner: ScopeId) -> Option<ListAccess<T>> {
        self.entries
            .peek()
            .iter()
            .find(|entry| entry.owner == owner)
            .cloned()
    }

    pub fn contains(&self, expected: &ListAccess<T>) -> bool {
        self.get(expected.owner).is_some_and(|entry| {
            entry.id == expected.id
                && entry.items == expected.items
                && entry.group == expected.group
        })
    }

    pub fn unregister(&mut self, owner: ScopeId) {
        self.entries
            .with_mut(|lists| lists.retain(|list| list.owner != owner));
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn locate(
        &self,
        key: &str,
        key_gen: Callback<T, String>,
        group: Option<ScopeId>,
    ) -> Option<(ListAccess<T>, usize, Vec<T>)> {
        let entries = self.entries.peek().clone();
        let mut found = None;
        for entry in entries {
            if group.is_some() && entry.group != group {
                continue;
            }
            let Some(values) = entry.items.read.call(()) else {
                continue;
            };
            for (index, item) in values.iter().enumerate() {
                if key_gen.call(item.clone()) == key {
                    // Reject ambiguous keys rather than choosing a list by order.
                    if found.is_some() {
                        return None;
                    }
                    found = Some((entry.clone(), index, values.clone()));
                }
            }
        }
        found
    }
}
