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
fn horizontal_sort_crosses_midpoints_and_keeps_direction_after_remeasurement() {
    let horizontal = |x, width| {
        Some(PixelsRect::new(
            PixelsVector2D::new(x, 0.0).to_point(),
            PixelsSize::new(width, 300.0),
        ))
    };
    let rects = [
        horizontal(0.0, 100.0),
        horizontal(112.0, 200.0),
        horizontal(324.0, 120.0),
    ];
    let axis = DragAxis::Horizontal;
    assert_eq!(
        sort_target_on_axis(&rects, 0, ClientPoint::new(200.0, 20.0), 180.0, axis),
        None
    );
    assert_eq!(
        sort_target_on_axis(&rects, 0, ClientPoint::new(220.0, 20.0), 200.0, axis),
        Some(1)
    );
    assert_eq!(
        sort_target_on_axis(&rects, 2, ClientPoint::new(200.0, 20.0), 220.0, axis),
        Some(1)
    );
    assert_eq!(
        sort_target_on_axis(&rects, 0, ClientPoint::new(320.0, 20.0), 300.0, axis),
        None
    );
    assert_eq!(
        sort_target_on_axis(&rects, 0, ClientPoint::new(220.0, 400.0), 200.0, axis),
        None
    );
    let reordered = [horizontal(0.0, 100.0), horizontal(112.0, 200.0)];
    assert_eq!(
        sort_target_on_axis(&reordered, 1, ClientPoint::new(80.0, 20.0), 60.0, axis),
        None
    );
    assert_eq!(
        sort_target_on_axis(&reordered, 1, ClientPoint::new(40.0, 20.0), 80.0, axis),
        Some(0)
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
