use dioxus::prelude::*;

use crate::{
    DndItem, context::DragSession, context::transaction::DndTransactionContext,
    hooks::use_dnd_context,
};

mod context;
mod geometry;
mod types;

pub(crate) use context::ConnectedListsContext;
pub(crate) use types::{ListEntry, ListIdentity, ListSnapshot};

/// Connects explicitly identified lists inside a [`DraggableView`](crate::prelude::DraggableView).
///
/// Give each participating [`Droppable`](crate::prelude::Droppable) a distinct
/// `id` and its own `Signal<Vec<T>>`. Lists outside this component, or without an
/// `id`, remain independent. Use `ConnectedLists::<YourItem>` to specify the item
/// type, which must match the enclosing `DraggableView`.
///
/// Transfers update both item signals live. The highlighted placeholder marks
/// the accepted drop: the pointer takes priority, followed by the greatest card
/// overlap covering at least 25% of the smaller visible surface. The handle may
/// stay outside the list. Releasing without an accepted target, pressing Escape,
/// or losing window focus restores the item to its original list and index.
/// Keep IDs stable and item keys unique across the view. Each list can use either
/// absolute or relative layout.
///
/// Moving between containers remounts the item's original content. Store state
/// that should survive a transfer in item data or in a parent keyed by item ID.
#[component]
pub fn ConnectedLists<T: DndItem>(
    children: Element,
    /// Called once when an item starts a drag in this group, before any transfer.
    on_drag_start: Option<Callback<T>>,
) -> Element {
    let dnd = use_dnd_context::<T>();
    let session = use_context::<DragSession<T>>();
    let transactions = use_context::<DndTransactionContext<T>>();
    let mut group =
        use_context_provider(move || ConnectedListsContext::new(dnd, session, transactions));

    use_effect(move || group.update(on_drag_start));
    use_drop(move || group.dispose());

    rsx! { {children} }
}

#[cfg(test)]
mod tests;
