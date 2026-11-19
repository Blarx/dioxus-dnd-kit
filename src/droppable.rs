use std::rc::Rc;

use crate::{
    DndItem, DragAxis, DragLayout,
    connected::{ConnectedListsContext, ListEntry, ListIdentity, ListSnapshot},
    context::list::ItemAccess,
    context::{list::ListAccess, transaction::DndTransactionContext},
    draggable::Draggable,
    hooks::{use_dnd_context, visible_container_rect, wait_for_layout},
};
use dioxus::{
    core::Task,
    html::geometry::{ClientPoint, PixelsRect, PixelsVector2D},
    prelude::*,
};

#[derive(Clone, Copy)]
pub(crate) struct DragLayoutContext(pub ReadSignal<DragLayout>);

/// Context for a container that can receive dropped items or handle sorting.
///
/// It tracks the geometry (rects) of all child elements to detect which item
/// is currently being hovered over during a drag operation.
#[derive(Debug, Clone, Default)]
pub struct DroppableContext<T: DndItem> {
    /// List of tracked items and their physical screen positions.
    pub elements: Signal<Vec<ItemGeometry<T>>>,
    /// Reference to the container node itself.
    pub dropable_ref: Signal<Option<Rc<MountedData>>>,
    /// Bounding box of the droppable container; clipped to its visible area in connected groups.
    pub dropable_rect: Signal<Option<PixelsRect>>,
}

// manual copy trait
impl<T: DndItem> Copy for DroppableContext<T> {}

/// Stores the association between a data item, its DOM reference, and its last measured rect.
#[derive(Debug, Clone)]
pub struct ItemGeometry<T: DndItem> {
    /// The user-provided data value.
    pub value: T,
    /// Cached bounding box, usually measured at the start of a drag or after a reorder.
    pub rect: Option<PixelsRect>,
    /// DOM reference used for re-measuring.
    pub mounted: Option<Rc<MountedData>>,
}

impl<T: DndItem> DroppableContext<T> {
    /// Create a new DroppableContext instance.
    fn new() -> Self {
        Self {
            elements: Signal::new(Vec::new()),
            dropable_ref: Signal::new(None),
            dropable_rect: Signal::new(None),
        }
    }

    /// Register a mounted element in the droppable context.
    pub fn register_element_mounted(&mut self, item: T, mounted: Rc<MountedData>) {
        self.elements.with_mut(|elements| {
            if let Some(index) = elements.iter().position(|e| e.value == item) {
                elements[index].mounted = Some(mounted);
            }
        });
    }

    pub(crate) fn register_element_by_key(
        &mut self,
        item: T,
        mounted: Rc<MountedData>,
        key_gen: Callback<T, String>,
    ) {
        let key = key_gen.call(item);
        self.elements.with_mut(|elements| {
            if let Some(element) = elements
                .iter_mut()
                .find(|e| key_gen.call(e.value.clone()) == key)
            {
                element.mounted = Some(mounted);
            }
        });
    }

    pub fn has_element(&self, item: &T) -> bool {
        self.elements
            .with(|elements| elements.iter().any(|e| e.value == *item))
    }

    /// Update the order of elements in the droppable context.
    pub fn update_elements_order(&mut self, items: Vec<T>) {
        self.elements.with_mut(|elements| {
            let mut old_map = std::mem::take(elements);

            for item in items {
                if let Some(pos) = old_map.iter().position(|e| e.value == item) {
                    elements.push(old_map.remove(pos));
                } else {
                    elements.push(ItemGeometry {
                        value: item,
                        rect: None,
                        mounted: None,
                    });
                }
            }
        });
    }

    fn update_elements_by_key(&mut self, items: Vec<T>, key_gen: Callback<T, String>) {
        self.elements.with_mut(|elements| {
            let mut previous = std::mem::take(elements);
            for item in items {
                let key = key_gen.call(item.clone());
                let mut element = previous
                    .iter()
                    .position(|e| key_gen.call(e.value.clone()) == key)
                    .map(|index| previous.remove(index))
                    .unwrap_or(ItemGeometry {
                        value: item.clone(),
                        rect: None,
                        mounted: None,
                    });
                element.value = item;
                elements.push(element);
            }
        });
    }

    pub async fn collect_elements_rects(&mut self) {
        let snapshot: Vec<_> = self
            .elements
            .peek()
            .iter()
            .map(|element| element.mounted.clone())
            .collect();

        let mut tasks = Vec::new();
        for mounted in snapshot.iter().flatten().cloned() {
            tasks.push(async move {
                let rect = mounted.get_client_rect().await.ok();
                (mounted, rect)
            });
        }
        let results = futures::future::join_all(tasks).await;
        self.elements.with_mut(|elements| {
            // A reorder or remount invalidates the entire batch, even if all
            // indices still exist. Never apply old geometry to a new DOM order.
            if elements.len() != snapshot.len()
                || !elements
                    .iter()
                    .zip(&snapshot)
                    .all(|(e, old)| match (&e.mounted, old) {
                        (Some(current), Some(old)) => Rc::ptr_eq(current, old),
                        (None, None) => true,
                        _ => false,
                    })
            {
                return;
            }
            for (mounted, rect) in results {
                if let Some(element) = elements.iter_mut().find(|e| {
                    e.mounted
                        .as_ref()
                        .is_some_and(|current| Rc::ptr_eq(current, &mounted))
                }) {
                    element.rect = rect;
                }
            }
        });
    }
}

/// A container component that sorts its `items` signal while dragging.
///
/// `layout` defaults to [`DragLayout::Absolute`] for existing applications. Use
/// [`DragLayout::Relative`] for content whose height should follow normal flow.
#[component]
pub fn Droppable<T: DndItem>(
    /// Stable, document-unique HTML ID. Non-empty IDs opt into the nearest ConnectedLists group.
    #[props(default)]
    id: ReadSignal<String>,
    /// CSS classes for the droppable container.
    #[props(default)]
    class: ReadSignal<String>,
    /// Default CSS classes to be passed down to each child [`Draggable`].
    #[props(default)]
    draggable_class: ReadSignal<String>,
    /// How the original items reserve space; the overlay is always fixed.
    #[props(default)]
    layout: ReadSignal<DragLayout>,
    /// Sorting direction; defaults to vertical. Match it with the container CSS.
    #[props(default)]
    axis: ReadSignal<DragAxis>,
    /// The reactive list of items. This signal will be modified during reordering.
    items: Signal<Vec<T>>,
) -> Element {
    let mut context = use_dnd_context::<T>();
    let connections =
        try_use_context::<ConnectedListsContext<T>>().filter(|group| group.belongs_to(context));
    let transactions = try_use_context::<DndTransactionContext<T>>();
    let owner = use_hook(dioxus::core::current_scope_id);
    use_context_provider(move || ListIdentity { owner, id });
    use_context_provider(move || DragLayoutContext(layout));
    let mut droppable_context = use_context_provider(move || {
        let mut zone = DroppableContext::<T>::new();
        zone.update_elements_by_key(items.peek().clone(), context.key_gen);
        zone
    });
    let mut measurement_task = use_signal(|| None::<Task>);
    let mut geometry_ready = use_signal(|| false);
    let mut measured_revision = use_signal(|| 0_u64);
    let mut last_mouse = use_signal(|| None::<ClientPoint>);
    let item_access = use_memo(use_reactive((&items,), |(items,)| ItemAccess::from(items)));

    use_effect(move || {
        let id = id();
        let items = item_access();
        if let Some(mut connections) = connections {
            connections.unregister(owner);
        }
        if let Some(mut transactions) = transactions {
            transactions.unregister(owner);
            let group = connections.filter(|_| !id.is_empty());
            if transactions.register(ListAccess {
                owner,
                id,
                items,
                group: group.map(|group| group.id()),
            }) && let Some(mut group) = group
            {
                group.register(ListEntry {
                    owner,
                    snapshot: ListSnapshot {
                        values: Vec::new(),
                        rect: None,
                        item_rects: Vec::new(),
                        ready: false,
                        measured_revision: 0,
                        axis: *axis.peek(),
                    },
                });
            }
        }
    });
    use_drop(move || {
        if let Some(mut connections) = connections {
            connections.unregister(owner);
        }
        if let Some(mut transactions) = transactions {
            transactions.unregister(owner);
        }
    });

    use_effect(move || {
        if let Some(mut connections) = connections {
            let _ = id();
            let _ = item_access();
            let values = items();
            let elements = droppable_context.elements.read();
            let order_matches = values.len() == elements.len()
                && values.iter().zip(elements.iter()).all(|(item, element)| {
                    context.key_gen.call(item.clone())
                        == context.key_gen.call(element.value.clone())
                });
            let snapshot = ListSnapshot {
                values,
                rect: (droppable_context.dropable_rect)(),
                item_rects: elements.iter().map(|element| element.rect).collect(),
                ready: geometry_ready() && order_matches,
                measured_revision: measured_revision(),
                axis: axis(),
            };
            drop(elements);
            connections.publish(owner, snapshot);
        }
    });

    let is_active = use_memo(move || {
        context.active.read().as_ref().is_some_and(|active| {
            let key = context.key_gen.call(active.clone());
            items
                .read()
                .iter()
                .any(|item| context.key_gen.call(item.clone()) == key)
        })
    });

    // Updating rects does not change this memo. Mounting, replacing or moving
    // nodes does, and must start a new measurement batch.
    let mounted_order = use_memo(move || {
        droppable_context
            .elements
            .read()
            .iter()
            .map(|element| {
                (
                    context.key_gen.call(element.value.clone()),
                    element
                        .mounted
                        .as_ref()
                        .map(|mounted| Rc::as_ptr(mounted) as usize),
                )
            })
            .collect::<Vec<_>>()
    });

    use_effect(move || {
        let current_items = items();
        let _ = axis();
        let revision = (context.recalculate_rects)();
        let _ = mounted_order();
        let mounted = (droppable_context.dropable_ref)();
        let active = is_active();
        let container_id = id();
        let connected = connections.is_some() && !container_id.is_empty();

        geometry_ready.set(false);
        if let Some(task) = measurement_task.take() {
            task.cancel();
        }
        droppable_context.update_elements_by_key(current_items, context.key_gen);
        if !active {
            last_mouse.set(None);
        }
        if !active && !connected {
            return;
        }

        measurement_task.set(Some(spawn(async move {
            wait_for_layout().await;
            if connected {
                droppable_context
                    .dropable_rect
                    .set(visible_container_rect(container_id).await);
            } else if let Some(mounted) = mounted
                && let Ok(rect) = mounted.get_client_rect().await
            {
                droppable_context.dropable_rect.set(Some(rect));
            }
            droppable_context.collect_elements_rects().await;
            measured_revision.set(revision);
            geometry_ready.set(true);
        })));
    });

    use_effect(move || {
        if !is_active() || !geometry_ready() || !(context.is_floating_ready)() {
            return;
        }
        let mouse = (context.mouse_pos)();
        let direction = axis();
        let previous = last_mouse
            .peek()
            .as_ref()
            .copied()
            .unwrap_or(*context.start_pos.peek());
        last_mouse.set(Some(mouse));

        let active_key = context
            .active
            .peek()
            .as_ref()
            .map(|item| context.key_gen.call(item.clone()));
        let elements = droppable_context.elements.peek();
        // External list edits may have landed before their measurement effect.
        let current_items = items.peek();
        if elements.len() != current_items.len()
            || !elements.iter().zip(current_items.iter()).all(|(e, item)| {
                context.key_gen.call(e.value.clone()) == context.key_gen.call(item.clone())
            })
        {
            return;
        }
        let Some(from) = elements
            .iter()
            .position(|e| Some(context.key_gen.call(e.value.clone())) == active_key)
        else {
            return;
        };
        let rects = elements
            .iter()
            .map(|element| element.rect)
            .collect::<Vec<_>>();
        let target = sort_target_on_axis(
            &rects,
            from,
            mouse,
            direction.coordinate(previous),
            direction,
        );
        drop(elements);
        drop(current_items);

        if let Some(to) = target {
            let moved = if let Some(mut transactions) = transactions {
                transactions.preview_move(owner, to).unwrap_or(false)
            } else {
                // Preserve custom low-level containers that provide DndContext
                // directly, without DraggableView's transaction provider.
                items.with_mut(|items| {
                    let item = items.remove(from);
                    items.insert(to, item);
                });
                true
            };
            if moved {
                // Pause collisions until the new DOM order has been measured.
                geometry_ready.set(false);
            }
        }
    });

    rsx! {
        div { id: if id().is_empty() { None } else { Some(id()) }, class: "{class}", position: "relative",
            onmounted: move |evt| {
                droppable_context.dropable_ref.set(Some(evt.data()));
            },
            onresize: move |_| {
                if connections.is_some() && !id().is_empty() {
                    context.recalculate_rects.with_mut(|version| *version += 1);
                }
            },
            for (index, item) in items().iter().enumerate() {
                Draggable { key: "{context.key_gen.call(item.clone())}", class: "{draggable_class}", item: item.clone(), index }
            }
        }
    }
}

/// Only cross a neighbor's midpoint in the direction the pointer is moving.
/// This prevents differently sized rows from swapping back after remeasurement.
#[cfg(test)]
fn sort_target(
    rects: &[Option<PixelsRect>],
    from: usize,
    mouse: ClientPoint,
    previous_y: f64,
) -> Option<usize> {
    sort_target_on_axis(rects, from, mouse, previous_y, DragAxis::Vertical)
}

fn sort_target_on_axis(
    rects: &[Option<PixelsRect>],
    from: usize,
    mouse: ClientPoint,
    previous: f64,
    axis: DragAxis,
) -> Option<usize> {
    let point = PixelsVector2D::new(mouse.x, mouse.y).to_point();
    rects.iter().enumerate().find_map(|(index, rect)| {
        let rect = rect.as_ref()?;
        if !rect.contains(point) {
            return None;
        }
        let center = axis.midpoint(*rect);
        let coordinate = axis.coordinate(mouse);
        let crosses_down = index > from && coordinate > previous && coordinate > center;
        let crosses_up = index < from && coordinate < previous && coordinate < center;
        (crosses_down || crosses_up).then_some(index)
    })
}

#[cfg(test)]
mod tests;
