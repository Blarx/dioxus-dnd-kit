use super::*;
use super::{
    geometry::{DropZone, insertion_index, target_zone},
    types::Preview,
};
use crate::{
    DragAxis, DragCancelReason,
    context::list::move_item,
    context::{DndContext, DragPosition, DragSession},
    context::{
        list::ListAccess,
        transaction::{DndTransactionContext, Options},
    },
};
use dioxus::html::geometry::{ClientPoint, PixelsRect, PixelsSize, PixelsVector2D};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Default)]
struct OwnershipCapture {
    group: Option<ConnectedListsContext<u32>>,
    context: Option<DndContext<u32>>,
    show: Option<Signal<bool>>,
    columns: Vec<(ScopeId, Signal<Vec<u32>>)>,
}

#[component]
fn OwnershipProbe() -> Element {
    let capture = use_context::<Rc<RefCell<OwnershipCapture>>>();
    let group = use_context::<ConnectedListsContext<u32>>();
    let context = use_dnd_context::<u32>();
    use_hook(move || {
        let mut capture = capture.borrow_mut();
        capture.group = Some(group);
        capture.context = Some(context);
    });
    rsx! {}
}

#[component]
fn OwnedColumn(destination: bool) -> Element {
    let items = use_signal(move || if destination { vec![4_u32] } else { vec![1, 2] });
    let capture = use_context::<Rc<RefCell<OwnershipCapture>>>();
    use_hook(move || {
        capture
            .borrow_mut()
            .columns
            .push((dioxus::core::current_scope_id(), items))
    });
    let id = if destination { "destination" } else { "source" };
    rsx! { crate::prelude::Droppable { id, items, layout: crate::DragLayout::Relative } }
}

#[test]
fn coordinator_never_hoists_child_signals_and_unregisters_before_child_drop() {
    use dioxus::logger::tracing::{self, Subscriber, span};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct CaptureWarnings(Arc<AtomicUsize>);
    impl Subscriber for CaptureWarnings {
        fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
            *metadata.level() <= tracing::Level::WARN
        }
        fn new_span(&self, _: &span::Attributes<'_>) -> span::Id {
            span::Id::from_u64(1)
        }
        fn record(&self, _: &span::Id, _: &span::Record<'_>) {}
        fn record_follows_from(&self, _: &span::Id, _: &span::Id) {}
        fn enter(&self, _: &span::Id) {}
        fn exit(&self, _: &span::Id) {}
        fn event(&self, event: &tracing::Event<'_>) {
            struct Visitor<'a>(&'a AtomicUsize);
            impl tracing::field::Visit for Visitor<'_> {
                fn record_debug(&mut self, _: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                    if format!("{value:?}").contains("A Copy Value created in") {
                        self.0.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            event.record(&mut Visitor(&self.0));
        }
    }

    fn app() -> Element {
        let show = use_signal(|| true);
        let capture = use_context::<Rc<RefCell<OwnershipCapture>>>();
        use_hook(move || capture.borrow_mut().show = Some(show));
        rsx! {
            crate::prelude::DraggableView {
                key_gen: |item: u32| item.to_string(), render: |item: u32| rsx! { "{item}" },
                ConnectedLists::<u32> {
                    OwnershipProbe {}
                    OwnedColumn { destination: false }
                    if show() { OwnedColumn { destination: true } }
                }
            }
        }
    }

    let warnings = Arc::new(AtomicUsize::new(0));
    tracing::subscriber::with_default(CaptureWarnings(warnings.clone()), || {
        let capture = Rc::new(RefCell::new(OwnershipCapture::default()));
        let mut dom = VirtualDom::new(app).with_root_context(capture.clone());
        dom.rebuild_in_place();
        dom.render_immediate_to_vec();
        let state = capture.borrow().clone();
        let mut context = state.context.unwrap();
        dom.in_scope(context.active.origin_scope(), || {
            context.active.set(Some(1))
        });
        for step in 0..4 {
            dom.in_scope(context.mouse_pos.origin_scope(), || {
                context.mouse_pos.set(ClientPoint::new(20.0, step as f64))
            });
            dom.render_immediate_to_vec();
        }
        let group = state.group.unwrap();
        dom.in_scope(group.lists.origin_scope(), || {
            let lists = group.lists.peek();
            assert!(move_item(
                group
                    .transactions
                    .registry
                    .get(lists[0].owner)
                    .unwrap()
                    .items,
                group
                    .transactions
                    .registry
                    .get(lists[1].owner)
                    .unwrap()
                    .items,
                "1",
                0,
                context.key_gen
            ));
        });
        dom.render_immediate_to_vec();
        let (source_scope, mut source) = state.columns[0];
        let (target_scope, target) = state.columns[1];
        dom.in_scope(source_scope, || assert_eq!(*source.peek(), vec![2]));
        dom.in_scope(target_scope, || assert_eq!(*target.peek(), vec![1, 4]));
        dom.in_scope(source_scope, || source.with_mut(|items| items.push(3)));
        dom.render_immediate_to_vec();
        let mut show = state.show.unwrap();
        dom.in_scope(show.origin_scope(), || show.set(false));
        dom.render_immediate_to_vec();
        assert_eq!(group.lists.peek().len(), 1);
        assert!(context.active.peek().is_none());
        dom.in_scope(source_scope, || assert_eq!(*source.peek(), vec![1, 2, 3]));
        assert!(
            target.try_peek().is_err(),
            "column-owned item signal must be dropped with its scope"
        );
    });
    assert_eq!(
        warnings.load(Ordering::Relaxed),
        0,
        "coordinator accessed child-owned signals"
    );
}

#[test]
fn a_nested_view_does_not_join_its_parents_group_even_with_the_same_item_type() {
    fn app() -> Element {
        let outer = use_signal(|| vec![1_u32]);
        let inner = use_signal(|| vec![2_u32]);
        rsx! { crate::prelude::DraggableView {
            key_gen: |item: u32| item.to_string(), render: |item: u32| rsx! { "{item}" },
            ConnectedLists::<u32> {
                OwnershipProbe {}
                crate::prelude::Droppable { id: "outer", items: outer }
                crate::prelude::DraggableView {
                    key_gen: |item: u32| item.to_string(), render: |item: u32| rsx! { "{item}" },
                    crate::prelude::Droppable { id: "inner", items: inner }
                }
            }
        } }
    }
    let capture = Rc::new(RefCell::new(OwnershipCapture::default()));
    let mut dom = VirtualDom::new(app).with_root_context(capture.clone());
    dom.rebuild_in_place();
    dom.render_immediate_to_vec();
    let group = capture.borrow().group.unwrap();
    assert_eq!(group.lists.peek().len(), 1);
    assert_eq!(
        group
            .transactions
            .registry
            .get(group.lists.peek()[0].owner)
            .unwrap()
            .id,
        "outer"
    );
}

type Case = Rc<RefCell<Option<Box<dyn FnOnce(Callback<u32, String>)>>>>;

fn run(case: impl FnOnce(Callback<u32, String>) + 'static) {
    fn app() -> Element {
        let case = use_context::<Case>();
        let keys = use_callback(|item: u32| item.to_string());
        use_hook(move || case.borrow_mut().take().unwrap()(keys));
        rsx! {}
    }
    let case: Case = Rc::new(RefCell::new(Some(Box::new(case))));
    VirtualDom::new(app)
        .with_root_context(case)
        .rebuild_in_place();
}

#[derive(Clone)]
struct TestList {
    owner: ScopeId,
    id: String,
    items: crate::context::list::ItemAccess<u32>,
    snapshot: ListSnapshot<u32>,
}

fn list(owner: usize, id: &str, items: Signal<Vec<u32>>) -> TestList {
    let x = owner.saturating_sub(1) as f64 * 220.0;
    TestList {
        owner: ScopeId(owner),
        id: id.into(),
        items: items.into(),
        snapshot: ListSnapshot {
            values: items.peek().clone(),
            rect: Some(rect(x, 0.0, 200.0, 300.0)),
            item_rects: items
                .peek()
                .iter()
                .enumerate()
                .map(|(i, _)| Some(rect(x, i as f64 * 80.0, 200.0, 80.0)))
                .collect(),
            ready: true,
            measured_revision: 0,
            axis: DragAxis::Vertical,
        },
    }
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> PixelsRect {
    PixelsRect::new(
        PixelsVector2D::new(x, y).to_point(),
        PixelsSize::new(width, height),
    )
}

fn point(x: f64, y: f64) -> DragPosition {
    DragPosition {
        mouse: ClientPoint::new(x, y),
        rect: None,
    }
}

fn board_zones() -> Vec<DropZone> {
    vec![
        DropZone {
            rect: Some(rect(0.0, 0.0, 200.0, 300.0)),
            ready: true,
        },
        DropZone {
            rect: Some(rect(220.0, 0.0, 200.0, 300.0)),
            ready: true,
        },
    ]
}

fn options() -> Options<u32> {
    Options {
        on_drop: None,
        can_drop: None,
        on_drag_start: None,
        on_drag_cancel: None,
    }
}

/// Uses the same required dependencies and transaction path as the provider.
struct Group {
    context: ConnectedListsContext<u32>,
    dnd: DndContext<u32>,
    session: DragSession<u32>,
    transactions: DndTransactionContext<u32>,
}

impl Group {
    fn new(keys: Callback<u32, String>, lists: Vec<TestList>) -> Self {
        let (mut dnd, session) = drag_context(keys);
        dnd.active.set(None);
        let transactions =
            DndTransactionContext::new(keys, options(), crate::context::DragCoordinator::new());
        let context = ConnectedListsContext::new(dnd, session, transactions);
        let mut group = Self {
            context,
            dnd,
            session,
            transactions,
        };
        for list in lists {
            group.register(list);
        }
        group
    }
    fn register(&mut self, list: TestList) {
        if self.transactions.register(ListAccess {
            owner: list.owner,
            id: list.id,
            items: list.items,
            group: Some(self.context.id()),
        }) {
            self.context.register(ListEntry {
                owner: list.owner,
                snapshot: list.snapshot,
            });
        }
    }
    fn begin(&mut self, item: u32) {
        assert!(
            self.transactions
                .begin(item.to_string(), *self.session.generation.peek())
        );
        let lists = self.context.lists.peek().clone();
        let (source, _) = self.context.find_source(&lists, &item.to_string()).unwrap();
        let bounds = lists[source].snapshot.rect.unwrap();
        self.dnd.mouse_pos.set(ClientPoint::new(
            bounds.min_x() + 10.0,
            bounds.min_y() + 10.0,
        ));
        self.dnd.active.set(Some(item));
        self.dnd.is_floating_ready.set(true);
        self.context.update(None);
    }
    fn release(&mut self, position: DragPosition, cancelled: bool) {
        self.dnd.mouse_pos.set(position.mouse);
        self.dnd.start_pos.set(position.mouse);
        self.dnd.active_rect.set(position.rect);
        self.dnd.finish_drag(self.session, cancelled);
        self.context.update(None);
    }
    fn unregister(&mut self, owner: ScopeId) {
        self.context.unregister(owner);
        self.transactions.unregister(owner);
    }
}

#[test]
fn hover_and_release_accept_the_same_overlap_with_the_handle_outside() {
    run(|keys| {
        let source = Signal::new(vec![1, 2]);
        let target = Signal::new(vec![4]);
        let mut group = Group::new(keys, vec![list(1, "todo", source), list(2, "done", target)]);
        group.begin(2);
        let position = DragPosition {
            mouse: ClientPoint::new(210.0, 100.0),
            rect: Some(rect(190.0, 80.0, 200.0, 80.0)),
        };
        assert_eq!(group.context.preview_at("2", position, 0), Ok(ScopeId(2)));
        assert_eq!(*source.peek(), vec![1]);
        assert_eq!(*target.peek(), vec![4, 2]);
        let mut entry = group.context.lists.peek()[1].clone();
        entry.snapshot.ready = false;
        group.context.publish(entry.owner, entry.snapshot);
        assert_eq!(group.context.preview_at("2", position, 0), Ok(ScopeId(2)));
        group.release(position, false);
        assert_eq!(*source.peek(), vec![1]);
        assert_eq!(*target.peek(), vec![4, 2]);
        assert!(group.context.preview.peek().is_none());
    });
}

#[test]
fn leaving_all_visible_targets_clears_the_preview_and_rolls_back_on_release() {
    run(|keys| {
        let mut source = Signal::new(vec![1, 2]);
        let mut target = Signal::new(vec![4]);
        let mut group = Group::new(keys, vec![list(1, "todo", source), list(2, "done", target)]);
        group.begin(2);
        let position = DragPosition {
            mouse: ClientPoint::new(210.0, 100.0),
            rect: Some(rect(190.0, 80.0, 200.0, 80.0)),
        };
        assert_eq!(group.context.preview_at("2", position, 0), Ok(ScopeId(2)));
        source.with_mut(|items| items.push(9));
        target.with_mut(|items| items.push(8));
        let outside = DragPosition {
            mouse: ClientPoint::new(600.0, 100.0),
            rect: Some(rect(580.0, 80.0, 200.0, 80.0)),
        };
        assert_eq!(
            group.context.preview_at("2", outside, 0),
            Err(DragCancelReason::Outside)
        );
        assert!(group.context.preview.peek().is_none());
        assert_eq!(*target.peek(), vec![4, 2, 8]);
        group.release(outside, false);
        assert_eq!(*source.peek(), vec![1, 2, 9]);
        assert_eq!(*target.peek(), vec![4, 8]);
    });
}

#[test]
fn pointer_priority_and_overlap_ranking_choose_one_target_without_edge_flicker() {
    run(|_| {
        let zones = board_zones();
        let mut position = DragPosition {
            mouse: ClientPoint::new(190.0, 100.0),
            rect: Some(rect(170.0, 80.0, 250.0, 80.0)),
        };
        assert_eq!(
            target_zone(&zones, position, None),
            Some(0),
            "pointer wins over larger overlap"
        );
        position.mouse.x = 210.0;
        assert_eq!(
            target_zone(&zones, position, None),
            Some(1),
            "largest overlap wins in the gap"
        );
        position.rect = Some(rect(110.0, 80.0, 200.0, 80.0));
        assert_eq!(
            target_zone(&zones, position, Some(1)),
            Some(1),
            "equal overlap preserves the current target"
        );
        position.rect = Some(rect(21.0, 80.0, 200.0, 80.0));
        assert_eq!(
            target_zone(&zones[1..], position, None),
            None,
            "one pixel is insufficient"
        );
        position.rect = Some(rect(20.0, 80.0, 200.0, 80.0));
        assert_eq!(
            target_zone(&zones[1..], position, None),
            None,
            "edge contact is insufficient"
        );
        position.rect = Some(rect(70.0, 80.0, 200.0, 80.0));
        assert_eq!(
            target_zone(&zones[1..], position, None),
            Some(0),
            "one quarter is sufficient"
        );
    });
}

#[test]
fn overlap_respects_clipping_and_cannot_select_a_new_unmeasured_list() {
    run(|_| {
        let mut zones = board_zones();
        let position = DragPosition {
            mouse: ClientPoint::new(210.0, 100.0),
            rect: Some(rect(190.0, 80.0, 200.0, 80.0)),
        };
        zones[1].ready = false;
        assert_eq!(target_zone(&zones, position, Some(0)), None);
        assert_eq!(
            target_zone(&zones, position, Some(1)),
            Some(1),
            "current list can await remeasurement"
        );
        zones[1].ready = true;
        zones[1].rect = Some(rect(220.0, 0.0, 200.0, 50.0));
        assert_eq!(
            target_zone(&zones, position, Some(1)),
            None,
            "hidden portion cannot receive a card"
        );
        zones[1].rect = None;
        assert_eq!(target_zone(&zones, position, Some(1)), None);
    });
}

#[test]
fn release_captures_its_rectangle_before_root_cleanup_and_a_new_pickup() {
    run(|keys| {
        let source = Signal::new(vec![1, 2]);
        let target = Signal::new(vec![4]);
        let mut group = Group::new(keys, vec![list(1, "todo", source), list(2, "done", target)]);
        group.begin(2);
        group.dnd.active_rect.set(Some(rect(0.0, 0.0, 200.0, 80.0)));
        group.dnd.start_pos.set(ClientPoint::new(20.0, 20.0));
        group.dnd.mouse_pos.set(ClientPoint::new(210.0, 100.0));
        group.dnd.finish_drag(group.session, false);
        let end = group.session.last_end.peek().unwrap();
        assert_eq!(end.position.rect, Some(rect(190.0, 80.0, 200.0, 80.0)));
        assert!(group.dnd.active_rect.peek().is_none());
        group.dnd.start_pos.set(ClientPoint::new(500.0, 500.0));
        group
            .dnd
            .active_rect
            .set(Some(rect(480.0, 480.0, 100.0, 200.0)));
        group.dnd.active.set(Some(1));
        group.context.update(None);
        assert_eq!(*source.peek(), vec![1]);
        assert_eq!(*target.peek(), vec![4, 2]);
        assert_eq!(*group.dnd.active.peek(), Some(1));
    });
}

#[test]
fn placeholder_visibility_tracks_acceptance_in_both_layouts_without_ending_the_drag() {
    use crate::{
        DragLayout,
        draggable::Draggable,
        droppable::{DragLayoutContext, DroppableContext},
    };
    use dioxus::core::{AttributeValue, Mutation};
    type Capture = Rc<RefCell<Option<(ConnectedListsContext<u32>, DndContext<u32>, ScopeId)>>>;

    fn app() -> Element {
        let capture = use_context::<Capture>();
        let initial_layout = use_context::<DragLayout>();
        let layout = use_signal(move || initial_layout);
        let keys = use_callback(|item: u32| item.to_string());
        let placeholder = use_callback(|_: (f64, f64)| rsx! { div { "Accepted drop" } });
        let context = use_context_provider(move || {
            let (mut context, _) = drag_context(keys);
            context.active_rect.set(Some(rect(0.0, 0.0, 200.0, 80.0)));
            context.placeholder_render = Some(placeholder);
            context
        });
        let mut group = use_context_provider(move || {
            let transactions =
                DndTransactionContext::new(keys, options(), crate::context::DragCoordinator::new());
            ConnectedListsContext::new(context, DragSession::new(), transactions)
        });
        let owner = dioxus::core::current_scope_id();
        let id = use_signal(|| "list".to_string());
        use_context_provider(move || ListIdentity {
            owner,
            id: id.into(),
        });
        use_context_provider(DroppableContext::<u32>::default);
        use_context_provider(move || DragLayoutContext(layout.into()));
        use_hook(move || {
            group.preview.set(Some(Preview {
                owner,
                key: "2".into(),
            }));
            *capture.borrow_mut() = Some((group, context, owner));
        });
        rsx! { Draggable { index: 0, item: 2_u32, class: "original" } }
    }

    for layout in [DragLayout::Relative, DragLayout::Absolute] {
        let capture: Capture = Rc::new(RefCell::new(None));
        let mut dom = VirtualDom::new(app)
            .with_root_context(capture.clone())
            .with_root_context(layout);
        let initial = dom.rebuild_to_vec();
        let footprint = initial
            .edits
            .iter()
            .find_map(|edit| match edit {
                Mutation::SetAttribute {
                    id,
                    name: "visibility",
                    ns: Some("style"),
                    value: AttributeValue::Text(value),
                } if value == "visible" => Some(*id),
                _ => None,
            })
            .expect("accepted placeholder is visible");
        let (mut group, context, owner) = capture.borrow().unwrap();
        for preview in [
            None,
            Some(Preview {
                owner,
                key: "2".into(),
            }),
            Some(Preview {
                owner,
                key: "other".into(),
            }),
        ] {
            let expected = if preview.as_ref().is_some_and(|preview| preview.key == "2") {
                "visible"
            } else {
                "hidden"
            };
            dom.in_scope(ScopeId::APP, || group.preview.set(preview));
            let updates = dom.render_immediate_to_vec();
            assert!(updates.edits.iter().any(|edit| matches!(edit,
                Mutation::SetAttribute { id, name: "visibility", ns: Some("style"), value: AttributeValue::Text(value) }
                    if *id == footprint && *value == expected
            )), "placeholder must become {expected} in {layout:?}");
            assert!(context.active.peek().is_some());
            assert!(*context.is_floating_ready.peek());
            // Hiding a rejected preview must not collapse the reserved slot.
            assert!(!updates.edits.iter().any(|edit| matches!(edit,
                Mutation::SetAttribute { id, name: "height", .. } if *id == footprint
            )));
        }
    }
}

#[test]
fn transfers_once_then_returns_to_the_original_list() {
    run(|keys| {
        let source = Signal::new(vec![1, 2, 3]);
        let target = Signal::new(vec![4, 5]);
        assert!(move_item(source, target, "2", 1, keys));
        assert_eq!(*source.peek(), vec![1, 3]);
        assert_eq!(*target.peek(), vec![4, 2, 5]);
        assert!(!move_item(source, target, "2", 1, keys));
        assert!(move_item(target, source, "2", 1, keys));
        assert_eq!(*source.peek(), vec![1, 2, 3]);
        assert_eq!(*target.peek(), vec![4, 5]);
    });
}

#[test]
fn a_failed_destination_write_preserves_both_lists() {
    run(|keys| {
        let source = Signal::new(vec![1, 2]);
        let target = Signal::new(vec![3]);
        let borrowed_target = target.peek();
        assert!(!move_item(source, target, "2", 0, keys));
        assert_eq!(*source.peek(), vec![1, 2]);
        assert_eq!(*borrowed_target, vec![3]);
    });
}

#[test]
fn a_transfer_invalidates_published_geometry_until_the_children_publish_again() {
    run(|keys| {
        let source = Signal::new(vec![1, 2]);
        let target = Signal::new(vec![]);
        let lists = vec![list(1, "todo", source), list(2, "done", target)];
        let group = Group::new(keys, lists.clone());
        assert!(
            group
                .context
                .drop_zones(&group.context.lists.peek(), 0)
                .iter()
                .all(|zone| zone.ready)
        );
        assert!(move_item(lists[0].items, lists[1].items, "2", 0, keys));
        assert!(
            group
                .context
                .drop_zones(&group.context.lists.peek(), 0)
                .iter()
                .all(|zone| !zone.ready)
        );
    });
}

#[test]
fn accepts_empty_destinations_and_clamps_the_insertion_index() {
    run(|keys| {
        let source = Signal::new(vec![1]);
        let target = Signal::new(Vec::new());
        assert!(move_item(source, target, "1", 99, keys));
        assert!(source.peek().is_empty());
        assert_eq!(*target.peek(), vec![1]);
        assert_eq!(insertion_index(&[], 50.0, DragAxis::Vertical), Some(0));
    });
}

#[test]
fn rejects_aliases_duplicates_and_missing_items_without_mutating_data() {
    run(|keys| {
        let source = Signal::new(vec![1, 2]);
        let duplicate = Signal::new(vec![2, 3]);
        assert!(!move_item(source, source, "1", 0, keys));
        assert!(!move_item(source, duplicate, "2", 0, keys));
        assert!(!move_item(source, duplicate, "99", 0, keys));
        assert_eq!(*source.peek(), vec![1, 2]);
        assert_eq!(*duplicate.peek(), vec![2, 3]);
    });
}

#[test]
fn cancellation_restores_position_without_overwriting_external_edits() {
    run(|keys| {
        let mut source = Signal::new(vec![1, 2, 3]);
        let mut target = Signal::new(vec![4]);
        let mut group = Group::new(keys, vec![list(1, "todo", source), list(2, "done", target)]);
        group.begin(2);
        assert_eq!(
            group.context.preview_at("2", point(250.0, 10.0), 0),
            Ok(ScopeId(2))
        );
        source.with_mut(|items| items.push(9));
        target.with_mut(|items| items.push(8));
        group.release(point(250.0, 10.0), true);
        assert_eq!(*source.peek(), vec![1, 2, 3, 9]);
        assert_eq!(*target.peek(), vec![4, 8]);

        group.begin(2);
        source.set(vec![2, 1, 3, 9]);
        group.transactions.mark_moved();
        group.release(point(20.0, 50.0), true);
        assert_eq!(*source.peek(), vec![1, 2, 3, 9]);
    });
}

#[test]
fn ambiguous_item_keys_do_not_select_a_source() {
    run(|keys| {
        let lists = vec![
            list(1, "one", Signal::new(vec![1])),
            list(2, "two", Signal::new(vec![1])),
        ];
        let group = Group::new(keys, lists.clone());
        assert_eq!(
            group.context.find_source(&group.context.lists.peek(), "1"),
            None
        );
    });
}

#[test]
fn registries_isolate_groups_and_reject_conflicting_list_ids_or_signals() {
    run(|keys| {
        let mut first = Group::new(keys, vec![]);
        let mut second = Group::new(keys, vec![]);
        let items = Signal::new(vec![1]);
        first.register(list(1, "todo", items));
        first.register(list(2, "todo", Signal::new(vec![2])));
        first.register(list(3, "other", items));
        second.register(list(4, "todo", Signal::new(vec![3])));
        assert_eq!(first.context.lists.peek().len(), 1);
        assert_eq!(second.context.lists.peek().len(), 1);
    });
}

#[test]
fn hit_testing_uses_container_bounds_and_ignores_unready_zones() {
    let mut zones = board_zones();
    assert_eq!(target_zone(&zones, point(250.0, 100.0), None), Some(1));
    assert_eq!(target_zone(&zones, point(210.0, 100.0), None), None);
    assert_eq!(target_zone(&zones, point(250.0, 350.0), None), None);
    zones[1].ready = false;
    assert_eq!(target_zone(&zones, point(250.0, 100.0), None), None);
}

#[test]
fn insertion_handles_mixed_heights_padding_gaps_and_missing_measurements() {
    let rects = [
        Some(rect(0.0, 10.0, 100.0, 60.0)),
        Some(rect(0.0, 90.0, 100.0, 160.0)),
    ];
    assert_eq!(insertion_index(&rects, 0.0, DragAxis::Vertical), Some(0));
    assert_eq!(insertion_index(&rects, 80.0, DragAxis::Vertical), Some(1));
    assert_eq!(insertion_index(&rects, 180.0, DragAxis::Vertical), Some(2));
    assert_eq!(
        insertion_index(&[rects[0], None], 80.0, DragAxis::Vertical),
        None
    );
}

#[test]
fn transfers_into_horizontal_lists_use_the_destination_x_axis() {
    run(|keys| {
        let source = Signal::new(vec![1, 2]);
        let target = Signal::new(vec![3, 4]);
        let mut destination = list(2, "done", target);
        destination.snapshot.axis = DragAxis::Horizontal;
        destination.snapshot.item_rects = vec![
            Some(rect(220.0, 0.0, 90.0, 300.0)),
            Some(rect(320.0, 0.0, 90.0, 300.0)),
        ];
        let mut group = Group::new(keys, vec![list(1, "todo", source), destination]);
        group.begin(2);
        assert_eq!(
            group.context.preview_at("2", point(300.0, 250.0), 0),
            Ok(ScopeId(2))
        );
        assert_eq!(*target.peek(), vec![3, 2, 4]);
    });
}

fn drag_context(keys: Callback<u32, String>) -> (DndContext<u32>, DragSession<u32>) {
    let context = DndContext {
        render: Callback::new(|_: u32| rsx! {}),
        placeholder_render: None,
        active: Signal::new(Some(2)),
        active_rect: Signal::new(None),
        active_mounted: Signal::new(None),
        is_floating_ready: Signal::new(true),
        mouse_pos: Signal::new(ClientPoint::zero()),
        start_pos: Signal::new(ClientPoint::zero()),
        grab_offset: Signal::new(Default::default()),
        key_gen: keys,
        recalculate_rects: Signal::new(0),
    };
    let mut session = DragSession::<u32>::new();
    session.generation.set(1);
    (context, session)
}

#[test]
fn unmounting_the_destination_rolls_back_before_unregistering() {
    run(|keys| {
        let source = Signal::new(vec![1, 2, 3]);
        let target = Signal::new(vec![4]);
        let mut group = Group::new(keys, vec![list(1, "todo", source), list(2, "done", target)]);
        group.begin(2);
        assert_eq!(
            group.context.preview_at("2", point(250.0, 10.0), 0),
            Ok(ScopeId(2))
        );
        group.unregister(ScopeId(2));
        assert_eq!(*source.peek(), vec![1, 2, 3]);
        assert_eq!(*target.peek(), vec![4]);
        assert!(group.dnd.active.peek().is_none());
        assert!(*group.session.cancelled.peek());
        assert!(group.context.drag.peek().is_none());
        assert!(group.context.preview.peek().is_none());
        assert_eq!(group.context.lists.peek().len(), 1);
    });
}

#[test]
fn unmounting_an_unrelated_list_keeps_the_drag_active() {
    run(|keys| {
        let source = Signal::new(vec![1, 2]);
        let mut group = Group::new(
            keys,
            vec![
                list(1, "todo", source),
                list(3, "other", Signal::new(vec![])),
            ],
        );
        group.begin(2);
        group.unregister(ScopeId(3));
        assert_eq!(*group.dnd.active.peek(), Some(2));
        assert!(group.context.drag.peek().is_some());
        assert!(!*group.session.cancelled.peek());
    });
}

#[test]
fn disposing_the_group_restores_the_item_and_releases_the_drag_before_child_cleanup() {
    run(|keys| {
        let source = Signal::new(vec![1, 2, 3]);
        let target = Signal::new(vec![4]);
        let mut group = Group::new(keys, vec![list(1, "todo", source), list(2, "done", target)]);
        group.begin(2);
        assert_eq!(
            group.context.preview_at("2", point(250.0, 10.0), 0),
            Ok(ScopeId(2))
        );
        group.context.dispose();
        assert_eq!(*source.peek(), vec![1, 2, 3]);
        assert_eq!(*target.peek(), vec![4]);
        assert!(group.dnd.active.peek().is_none());
        assert!(group.context.drag.peek().is_none());
        assert!(group.context.preview.peek().is_none());
        assert!(!group.transactions.state.is_busy());
        group.unregister(ScopeId(1));
        group.unregister(ScopeId(2));
        assert!(group.context.lists.peek().is_empty());
    });
}

#[test]
fn a_denied_candidate_hides_the_preview_and_never_changes_the_lists() {
    run(|keys| {
        let source = Signal::new(vec![1, 2]);
        let target = Signal::new(vec![4]);
        let mut group = Group::new(keys, vec![list(1, "todo", source), list(2, "done", target)]);
        let mut policy = options();
        policy.can_drop = Some(Callback::new(|change: crate::DropChange<u32>| {
            change.to.list_id != "done"
        }));
        group.transactions.configure(policy);
        group.begin(2);
        assert!(group.context.preview.peek().is_some());
        assert_eq!(
            group.context.preview_at("2", point(250.0, 10.0), 0),
            Err(DragCancelReason::NotAllowed)
        );
        assert!(group.context.preview.peek().is_none());
        group.release(point(250.0, 10.0), false);
        assert_eq!(*source.peek(), vec![1, 2]);
        assert_eq!(*target.peek(), vec![4]);
        assert!(!group.transactions.state.is_busy());
    });
}

#[test]
fn explicit_cancel_rolls_back_even_when_pointer_is_inside_a_valid_destination() {
    run(|keys| {
        let source = Signal::new(vec![1, 2, 3]);
        let target = Signal::new(vec![4]);
        let mut group = Group::new(keys, vec![list(1, "todo", source), list(2, "done", target)]);
        group.begin(2);
        assert_eq!(
            group.context.preview_at("2", point(250.0, 10.0), 0),
            Ok(ScopeId(2))
        );
        group.release(point(250.0, 100.0), true);
        assert_eq!(*source.peek(), vec![1, 2, 3]);
        assert_eq!(*target.peek(), vec![4]);
    });
}

#[test]
fn quick_release_keeps_a_transfer_while_destination_remeasurement_is_pending() {
    run(|keys| {
        let source = Signal::new(vec![1, 2, 3]);
        let target = Signal::new(vec![4]);
        let mut group = Group::new(keys, vec![list(1, "todo", source), list(2, "done", target)]);
        group.begin(2);
        assert_eq!(
            group.context.preview_at("2", point(250.0, 10.0), 0),
            Ok(ScopeId(2))
        );
        let lists = group.context.lists.peek().clone();
        for mut entry in lists {
            entry.snapshot.ready = false;
            group.context.publish(entry.owner, entry.snapshot);
        }
        group.release(point(250.0, 100.0), false);
        assert_eq!(*source.peek(), vec![1, 3]);
        assert_eq!(*target.peek(), vec![2, 4]);
    });
}

#[test]
fn late_mouseup_and_new_pickup_cannot_undo_previous_cancellation() {
    run(|keys| {
        let source = Signal::new(vec![1, 2, 3]);
        let target = Signal::new(vec![4]);
        let mut group = Group::new(keys, vec![list(1, "todo", source), list(2, "done", target)]);
        group.begin(2);
        assert_eq!(
            group.context.preview_at("2", point(250.0, 10.0), 0),
            Ok(ScopeId(2))
        );
        group.dnd.mouse_pos.set(ClientPoint::new(250.0, 100.0));
        group.dnd.finish_drag(group.session, true);
        group.dnd.finish_drag(group.session, false);
        let end = group.session.last_end.peek().unwrap();
        assert_eq!(end.generation, 1);
        assert!(end.cancelled);

        group
            .session
            .generation
            .with_mut(|generation| *generation += 1);
        group.session.cancelled.set(false);
        group.dnd.active.set(Some(3));
        group.dnd.mouse_pos.set(ClientPoint::new(20.0, 50.0));
        group.context.update(None);
        assert_eq!(*source.peek(), vec![1, 2, 3]);
        assert_eq!(*target.peek(), vec![4]);
        assert_eq!(*group.dnd.active.peek(), Some(3));
    });
}

#[test]
fn a_new_measurement_revision_rejects_previously_ready_geometry() {
    run(|keys| {
        let entry = list(1, "todo", Signal::new(vec![1]));
        let group = Group::new(keys, vec![entry.clone()]);
        let mut entry = ListEntry {
            owner: entry.owner,
            snapshot: entry.snapshot,
        };
        let stale = group.context.drop_zones(&[entry.clone()], 1);
        assert_eq!(target_zone(&stale, point(50.0, 50.0), None), None);
        entry.snapshot.measured_revision = 1;
        let fresh = group.context.drop_zones(&[entry], 1);
        assert_eq!(target_zone(&fresh, point(50.0, 50.0), None), Some(0));
    });
}
