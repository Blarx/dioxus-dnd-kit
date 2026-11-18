use std::rc::Rc;

use crate::{
    DndItem, DragLayout,
    draggable::Draggable,
    hooks::{use_dnd_context, wait_for_layout},
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
    /// CSS classes for the droppable container.
    #[props(default)]
    class: ReadSignal<String>,
    /// Default CSS classes to be passed down to each child [`Draggable`].
    #[props(default)]
    draggable_class: ReadSignal<String>,
    /// How the original items reserve space; the overlay is always fixed.
    #[props(default)]
    layout: ReadSignal<DragLayout>,
    /// The reactive list of items. This signal will be modified during reordering.
    items: Signal<Vec<T>>,
) -> Element {
    let context = use_dnd_context::<T>();
    use_context_provider(move || DragLayoutContext(layout));
    let mut droppable_context = use_context_provider(move || {
        let mut zone = DroppableContext::<T>::new();
        zone.update_elements_by_key(items.peek().clone(), context.key_gen);
        zone
    });
    let mut measurement_task = use_signal(|| None::<Task>);
    let mut geometry_ready = use_signal(|| false);
    let mut last_mouse = use_signal(|| None::<ClientPoint>);

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
        let _ = (context.recalculate_rects)();
        let _ = mounted_order();
        let mounted = (droppable_context.dropable_ref)();
        let active = is_active();

        geometry_ready.set(false);
        if let Some(task) = measurement_task.take() {
            task.cancel();
        }
        droppable_context.update_elements_by_key(current_items, context.key_gen);
        if !active {
            last_mouse.set(None);
            return;
        }

        measurement_task.set(Some(spawn(async move {
            wait_for_layout().await;
            if let Some(mounted) = mounted
                && let Ok(rect) = mounted.get_client_rect().await
            {
                droppable_context.dropable_rect.set(Some(rect));
            }
            droppable_context.collect_elements_rects().await;
            geometry_ready.set(true);
        })));
    });

    use_effect(move || {
        if !is_active() || !geometry_ready() || !(context.is_floating_ready)() {
            return;
        }
        let mouse = (context.mouse_pos)();
        let previous_y = last_mouse
            .peek()
            .as_ref()
            .map(|point| point.y)
            .unwrap_or(context.start_pos.peek().y);
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
        let target = sort_target(&rects, from, mouse, previous_y);
        drop(elements);
        drop(current_items);

        if let Some(to) = target {
            // Pause collision handling until the new DOM order has been measured.
            geometry_ready.set(false);
            items.with_mut(|items| {
                let item = items.remove(from);
                items.insert(to, item);
            });
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

/// Only cross a neighbor's midpoint in the direction the pointer is moving.
/// This prevents differently sized rows from swapping back after remeasurement.
fn sort_target(
    rects: &[Option<PixelsRect>],
    from: usize,
    mouse: ClientPoint,
    previous_y: f64,
) -> Option<usize> {
    let point = PixelsVector2D::new(mouse.x, mouse.y).to_point();
    rects.iter().enumerate().find_map(|(index, rect)| {
        let rect = rect.as_ref()?;
        if !rect.contains(point) {
            return None;
        }
        let center_y = rect.min_y() + rect.height() / 2.0;
        let crosses_down = index > from && mouse.y > previous_y && mouse.y > center_y;
        let crosses_up = index < from && mouse.y < previous_y && mouse.y < center_y;
        (crosses_down || crosses_up).then_some(index)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus::html::geometry::PixelsSize;
    use dioxus::html::{MountedResult, RenderedElementBacking};
    use std::{
        cell::RefCell,
        future::Future,
        pin::Pin,
        task::{Context, Poll},
    };

    fn rect(y: f64, height: f64) -> Option<PixelsRect> {
        Some(PixelsRect::new(
            PixelsVector2D::new(0.0, y).to_point(),
            PixelsSize::new(200.0, height),
        ))
    }

    #[test]
    fn crosses_midpoints_in_both_directions() {
        let rects = [rect(0.0, 60.0), rect(72.0, 160.0), rect(244.0, 80.0)];
        assert_eq!(
            sort_target(&rects, 0, ClientPoint::new(20.0, 150.0), 100.0),
            None
        );
        assert_eq!(
            sort_target(&rects, 0, ClientPoint::new(20.0, 160.0), 150.0),
            Some(1)
        );
        assert_eq!(
            sort_target(&rects, 2, ClientPoint::new(20.0, 150.0), 160.0),
            Some(1)
        );
    }

    #[test]
    fn mixed_heights_do_not_swap_back_after_remeasurement() {
        // A tall active row moved below a short row. After remeasurement the
        // pointer overlaps the short row, but is still travelling downwards.
        let rects = [rect(0.0, 60.0), rect(72.0, 160.0)];
        assert_eq!(
            sort_target(&rects, 1, ClientPoint::new(20.0, 25.0), 24.0),
            None
        );
        assert_eq!(
            sort_target(&rects, 1, ClientPoint::new(20.0, 25.0), 25.0),
            None
        );
        assert_eq!(
            sort_target(&rects, 1, ClientPoint::new(20.0, 25.0), 26.0),
            Some(0)
        );
    }

    #[test]
    fn ignores_gaps_outside_width_and_missing_rects() {
        let rects = [rect(0.0, 60.0), rect(72.0, 160.0), None];
        assert_eq!(
            sort_target(&rects, 0, ClientPoint::new(20.0, 65.0), 50.0),
            None
        );
        assert_eq!(
            sort_target(&rects, 0, ClientPoint::new(220.0, 170.0), 150.0),
            None
        );
        assert_eq!(
            sort_target(&rects, 0, ClientPoint::new(20.0, 250.0), 150.0),
            None
        );
    }

    #[derive(Clone, PartialEq)]
    struct Item {
        id: u32,
        expanded: bool,
    }

    #[test]
    fn preserves_registration_when_item_data_changes() {
        fn app() -> Element {
            let mut zone = use_hook(DroppableContext::<Item>::new);
            let key_gen = use_callback(|item: Item| item.id.to_string());
            use_hook(move || {
                let item = Item {
                    id: 1,
                    expanded: false,
                };
                zone.update_elements_by_key(vec![item.clone()], key_gen);
                let node = Rc::new(MountedData::new(()));
                zone.register_element_by_key(item, node.clone(), key_gen);
                zone.update_elements_by_key(
                    vec![Item {
                        id: 1,
                        expanded: true,
                    }],
                    key_gen,
                );
                let elements = zone.elements.peek();
                assert!(elements[0].value.expanded);
                assert!(Rc::ptr_eq(elements[0].mounted.as_ref().unwrap(), &node));
            });
            rsx! {}
        }
        VirtualDom::new(app).rebuild_in_place();
    }

    struct DelayedRect(RefCell<Option<futures::channel::oneshot::Receiver<PixelsRect>>>);

    impl RenderedElementBacking for DelayedRect {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn get_client_rect(&self) -> Pin<Box<dyn Future<Output = MountedResult<PixelsRect>>>> {
            let receiver = self.0.borrow_mut().take().unwrap();
            Box::pin(async move { Ok(receiver.await.unwrap()) })
        }
    }

    #[test]
    fn discards_rects_when_order_changes_during_measurement() {
        fn app() -> Element {
            let mut zone = use_hook(DroppableContext::<u32>::new);
            use_hook(move || {
                zone.update_elements_order(vec![1, 2]);
                let (first_tx, first_rx) = futures::channel::oneshot::channel();
                let (second_tx, second_rx) = futures::channel::oneshot::channel();
                zone.register_element_mounted(
                    1,
                    Rc::new(MountedData::new(DelayedRect(RefCell::new(Some(first_rx))))),
                );
                zone.register_element_mounted(
                    2,
                    Rc::new(MountedData::new(DelayedRect(RefCell::new(Some(second_rx))))),
                );
                let mut measuring = zone;
                let mut measurement = Box::pin(measuring.collect_elements_rects());
                let waker = futures::task::noop_waker();
                assert!(matches!(
                    measurement.as_mut().poll(&mut Context::from_waker(&waker)),
                    Poll::Pending
                ));

                zone.update_elements_order(vec![2, 1]);
                first_tx.send(rect(0.0, 60.0).unwrap()).unwrap();
                second_tx.send(rect(74.0, 120.0).unwrap()).unwrap();
                futures::executor::block_on(measurement);
                assert!(
                    zone.elements
                        .peek()
                        .iter()
                        .all(|element| element.rect.is_none())
                );
            });
            rsx! {}
        }
        VirtualDom::new(app).rebuild_in_place();
    }
}
