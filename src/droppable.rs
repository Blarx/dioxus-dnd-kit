use std::rc::Rc;

use crate::{DndItem, draggable::Draggable, hooks::use_dnd_context};
use dioxus::{
    core::Task,
    html::geometry::{PixelsRect, PixelsVector2D},
    prelude::*,
};

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
    /// Bounding box of the droppable container.
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

    pub async fn collect_elements_rects(&mut self) {
        let items_to_measure: Vec<(usize, Rc<MountedData>)> = self
            .elements
            .read()
            .iter()
            .enumerate()
            .filter_map(|(i, e)| e.mounted.as_ref().map(|m| (i, m.clone())))
            .collect();

        let mut tasks = Vec::new();
        for (index, mounted) in items_to_measure {
            tasks.push(async move {
                let rect = mounted.get_client_rect().await.unwrap_or_default();
                (index, rect)
            });
        }
        let results = futures::future::join_all(tasks).await;
        self.elements.with_mut(|elements| {
            for (index, rect) in results {
                if let Some(element) = elements.get_mut(index) {
                    element.rect = Some(rect);
                }
            }
        });
    }
}

/// A container component that enables sorting and dropping for its child [`Draggable`] items.
///
/// It automatically manages the order of the provided `items` signal by detecting
/// collisions between the dragged item and other elements in the list.
#[component]
pub fn Droppable<T: DndItem>(
    /// CSS classes for the droppable container.
    #[props(default)]
    class: ReadSignal<String>,
    /// Default CSS classes to be passed down to each child [`Draggable`].
    #[props(default)]
    draggable_class: ReadSignal<String>,
    /// The reactive list of items. This signal will be modified during reordering.
    items: Signal<Vec<T>>,
) -> Element {
    let context = use_dnd_context::<T>();
    let mut droppable_context = use_context_provider(DroppableContext::<T>::new);
    let mut reorder_task = use_signal(|| Option::<Task>::None);

    use_effect(move || {
        if let Some(dropable) = (droppable_context.dropable_ref)() {
            spawn(async move {
                let rect = dropable.get_client_rect().await.unwrap_or_default();

                droppable_context.dropable_rect.set(Some(rect));
            });
        } else {
            droppable_context.dropable_rect.set(None);
        }
    });

    let is_active = use_memo(move || {
        context
            .active
            .read()
            .clone()
            .map(|active_item| droppable_context.has_element(&active_item))
            .unwrap_or(false)
    });

    // ordering
    use_effect(move || {
        droppable_context.update_elements_order(items());
    });

    // on drag start
    use_effect(move || {
        if is_active() {
            spawn(async move {
                droppable_context.collect_elements_rects().await;
            });
        }
    });

    // Refresh cached collision rects when the app reports that item layout changed.
    use_effect(move || {
        let _ = (context.recalculate_rects)();

        if is_active() {
            spawn(async move {
                gloo_timers::future::TimeoutFuture::new(10).await;

                if let Some(dropable) = (droppable_context.dropable_ref)() {
                    let rect = dropable.get_client_rect().await.unwrap_or_default();
                    droppable_context.dropable_rect.set(Some(rect));
                }

                droppable_context.collect_elements_rects().await;
            });
        }
    });

    // on dragging
    use_effect(move || {
        if is_active() {
            let mouse = context.mouse_pos.read();
            let target_point = PixelsVector2D::new(mouse.x, mouse.y).to_point();
            let active_item = context.active.read();
            let elements = droppable_context.elements.peek();

            if let Some(active_val) = active_item.as_ref() {
                let from_idx = elements.iter().position(|e| e.value == *active_val);

                let to_idx = elements.iter().position(|e| {
                    if let Some(rect) = e.rect
                        && rect.contains(target_point)
                    {
                        let center_y = rect.min_y() + rect.height() / 2.0;

                        if from_idx.unwrap_or(0)
                            < elements
                                .iter()
                                .position(|el| el.value == e.value)
                                .unwrap_or(0)
                        {
                            return target_point.y > center_y;
                        } else {
                            return target_point.y < center_y;
                        }
                    }
                    false
                });

                if let (Some(to), Some(from)) = (to_idx, from_idx)
                    && from != to
                {
                    let task = spawn(async move {
                        let mut next_items = items.peek().clone();
                        let item = next_items.remove(from);
                        next_items.insert(to, item);
                        items.set(next_items);

                        gloo_timers::future::TimeoutFuture::new(10).await;
                        droppable_context.collect_elements_rects().await;
                    });
                    if let Some(old_task) = *reorder_task.peek() {
                        old_task.cancel();
                    }
                    reorder_task.set(Some(task));
                }
            }
        }
    });

    rsx! {
        div { class: "{class}", position: "relative",
            onmounted: move |evt| {
                droppable_context.dropable_ref.set(Some(evt.data()));
            },
            for (index, item) in items().iter().enumerate() {
                Draggable { key: "{context.key_gen.call(item.clone())}", class: "{draggable_class}", item: item.clone(), index }
            }
        }
    }
}
