use super::*;
use crate::{
    DragLayout,
    draggable::Draggable,
    droppable::{DragLayoutContext, DroppableContext},
};
use dioxus::core::{AttributeValue, ElementId, Mutation};
use std::cell::{Cell, RefCell};

#[derive(Clone, Copy)]
struct Controls {
    context: DndContext<u32>,
    layout: Signal<DragLayout>,
}

fn item_under_test() -> Element {
    let captured = use_context::<Rc<RefCell<Option<Controls>>>>();
    let key_gen = use_callback(|item: u32| item.to_string());
    let render = use_callback(|_: u32| rsx! { span { "Item" } });
    let context = use_context_provider(move || DndContext::new(key_gen, render, None));
    use_context_provider(DroppableContext::<u32>::default);
    let layout = use_signal(|| DragLayout::Relative);
    use_context_provider(move || DragLayoutContext(layout.into()));
    use_hook(move || *captured.borrow_mut() = Some(Controls { context, layout }));
    rsx! { Draggable { index: 0, item: 1_u32, class: "regression-item" } }
}

fn setup() -> (VirtualDom, Controls, ElementId) {
    let captured = Rc::new(RefCell::new(None::<Controls>));
    let mut dom = VirtualDom::new(item_under_test).with_root_context(captured.clone());
    let initial = dom.rebuild_to_vec();
    let content = initial
        .edits
        .iter()
        .find_map(|edit| match edit {
            Mutation::SetAttribute {
                name: "class",
                value: AttributeValue::Text(class),
                id,
                ..
            } if class == "regression-item" => Some(*id),
            _ => None,
        })
        .expect("the original draggable content must be mounted");
    let controls = captured.borrow().expect("capture drag controls");
    (dom, controls, content)
}

fn has_style(edits: &[Mutation], node: ElementId, property: &str, expected: Option<&str>) -> bool {
    edits.iter().any(|edit| match edit {
        Mutation::SetAttribute {
            name,
            ns: Some("style"),
            value,
            id,
        } if *id == node && *name == property => match (value, expected) {
            (AttributeValue::Text(actual), Some(expected)) => actual == expected,
            (AttributeValue::None, None) => true,
            _ => false,
        },
        _ => false,
    })
}

#[test]
fn restores_hit_testing_after_each_of_two_drags() {
    let (mut dom, mut controls, content) = setup();
    for _ in 0..2 {
        dom.in_scope(ScopeId::APP, || {
            controls.context.active.set(Some(1));
            controls.context.is_floating_ready.set(true);
        });
        let dragging = dom.render_immediate_to_vec();
        assert!(has_style(
            &dragging.edits,
            content,
            "pointer-events",
            Some("none")
        ));
        assert!(has_style(
            &dragging.edits,
            content,
            "visibility",
            Some("hidden")
        ));

        dom.in_scope(ScopeId::APP, || {
            controls.context.active.set(None);
            controls.context.is_floating_ready.set(false);
        });
        let dropped = dom.render_immediate_to_vec();
        // This must be a renderer-level CSS property removal. Omitting it
        // from a replacement style string leaves pointer-events: none behind.
        assert!(has_style(&dropped.edits, content, "pointer-events", None));
        assert!(has_style(
            &dropped.edits,
            content,
            "visibility",
            Some("visible")
        ));
        assert!(
            !dropped
                .edits
                .iter()
                .any(|edit| matches!(edit, Mutation::Remove { id } if *id == content))
        );
    }
}

#[test]
fn switching_layouts_clears_styles_owned_by_previous_mode() {
    let (mut dom, mut controls, content) = setup();
    dom.in_scope(ScopeId::APP, || {
        controls.context.active.set(Some(1));
        controls.context.is_floating_ready.set(true);
    });
    dom.render_immediate_to_vec();

    dom.in_scope(ScopeId::APP, || controls.layout.set(DragLayout::Absolute));
    let absolute = dom.render_immediate_to_vec();
    assert!(has_style(&absolute.edits, content, "pointer-events", None));
    assert!(has_style(&absolute.edits, content, "visibility", None));
    assert!(has_style(&absolute.edits, content, "opacity", Some("0")));

    dom.in_scope(ScopeId::APP, || controls.layout.set(DragLayout::Relative));
    let relative = dom.render_immediate_to_vec();
    for property in ["top", "left", "right", "opacity"] {
        assert!(
            has_style(&relative.edits, content, property, None),
            "clear {property}"
        );
    }
    assert!(has_style(
        &relative.edits,
        content,
        "position",
        Some("relative")
    ));
}

type DragCapture = Rc<RefCell<Option<(DndContext<u32>, DragSession<u32>)>>>;

type ResetCapture = Rc<RefCell<Option<(DndContext<u32>, crate::DndState<u32>)>>>;

#[component]
fn PublicResetProbe() -> Element {
    let capture = use_context::<ResetCapture>();
    let context = crate::prelude::use_dnd_context::<u32>();
    let state = crate::use_dnd_state::<u32>();
    use_hook(move || *capture.borrow_mut() = Some((context, state)));
    let items = use_signal(|| vec![1_u32, 2]);
    rsx! { crate::prelude::Droppable { items } }
}

#[test]
fn clearing_public_active_finishes_independent_and_untracked_drags() {
    fn app() -> Element {
        rsx! { DraggableView {
            key_gen: |item: u32| item.to_string(), render: |item: u32| rsx! { "{item}" },
            PublicResetProbe {}
        } }
    }
    let capture: ResetCapture = Rc::new(RefCell::new(None));
    let mut dom = VirtualDom::new(app).with_root_context(capture.clone());
    dom.rebuild_in_place();
    dom.render_immediate_to_vec();
    let (mut context, state) = capture.borrow().as_ref().copied().unwrap();
    let scope = context.active.origin_scope();
    for item in [1, 1, 90, 2] {
        dom.in_scope(scope, || context.active.set(Some(item)));
        for _ in 0..3 {
            dom.render_immediate_to_vec();
        }
        assert!(dom.in_scope(scope, || state.is_busy()));
        dom.in_scope(scope, || context.active.set(None));
        for _ in 0..3 {
            dom.render_immediate_to_vec();
        }
        assert!(!dom.in_scope(scope, || state.is_busy()));
        assert!(!dom.in_scope(scope, || state.is_pending()));
    }
}

type PublicChangeCapture = Rc<
    RefCell<
        Option<(
            DndContext<u32>,
            transaction::DndTransactionContext<u32>,
            Signal<Vec<u32>>,
        )>,
    >,
>;
type DropRequests = Rc<RefCell<Vec<crate::DropEvent<u32>>>>;

#[component]
fn PublicChangeProbe() -> Element {
    let capture = use_context::<PublicChangeCapture>();
    let context = crate::prelude::use_dnd_context::<u32>();
    let transactions = use_context::<transaction::DndTransactionContext<u32>>();
    let items = use_signal(|| vec![1_u32, 2, 3]);
    use_hook(move || *capture.borrow_mut() = Some((context, transactions, items)));
    rsx! { crate::prelude::Droppable { items } }
}

#[test]
fn clearing_public_active_emits_one_request_and_keeps_validation_pending() {
    fn app() -> Element {
        let requests = use_context::<DropRequests>();
        let starts = use_context::<Rc<Cell<usize>>>();
        rsx! { DraggableView {
            key_gen: |item: u32| item.to_string(), render: |item: u32| rsx! { "{item}" },
            on_drag_start: move |_: crate::DragStart<u32>| starts.set(starts.get() + 1),
            on_drop: move |event: crate::DropEvent<u32>| requests.borrow_mut().push(event),
            PublicChangeProbe {}
        } }
    }
    let capture: PublicChangeCapture = Rc::new(RefCell::new(None));
    let requests: DropRequests = Rc::new(RefCell::new(Vec::new()));
    let starts = Rc::new(Cell::new(0));
    let mut dom = VirtualDom::new(app)
        .with_root_context(capture.clone())
        .with_root_context(requests.clone())
        .with_root_context(starts.clone());
    dom.rebuild_in_place();
    for _ in 0..3 {
        dom.render_immediate_to_vec();
    }
    let (mut context, mut transactions, items) = capture.borrow().as_ref().copied().unwrap();
    let scope = context.active.origin_scope();
    for (attempt, accepted) in [true, false].into_iter().enumerate() {
        dom.in_scope(scope, || context.active.set(Some(2)));
        for _ in 0..3 {
            dom.render_immediate_to_vec();
        }
        dom.in_scope(scope, || {
            let owner = transactions
                .registry
                .locate("2", context.key_gen, None)
                .unwrap()
                .0
                .owner;
            assert_eq!(
                transactions.preview_move(owner, if accepted { 2 } else { 0 }),
                Ok(true)
            );
            context.active.set(None);
        });
        for _ in 0..3 {
            dom.render_immediate_to_vec();
        }
        assert_eq!(starts.get(), attempt + 1);
        assert_eq!(requests.borrow().len(), attempt + 1);
        let event = requests.borrow()[attempt].clone();
        dom.in_scope(scope, || {
            assert!(transactions.state.is_pending());
            assert!(transactions.state.is_busy());
            assert_eq!(transactions.state.pending(), Some(event.change.clone()));
            let result = if accepted {
                event.accept()
            } else {
                event.reject()
            };
            assert_eq!(result, crate::DropResult::Applied);
            assert!(!transactions.state.is_pending());
            assert!(!transactions.state.is_busy());
        });
        dom.in_scope(items.origin_scope(), || {
            assert_eq!(*items.peek(), vec![1, 3, 2])
        });
    }
}

#[component]
fn CaptureDrag() -> Element {
    let capture = use_context::<DragCapture>();
    let context = use_context::<DndContext<u32>>();
    let session = use_context::<DragSession<u32>>();
    use_hook(move || *capture.borrow_mut() = Some((context, session)));
    rsx! {}
}

#[component]
fn OverlayMountProbe() -> Element {
    let count = use_context::<Rc<Cell<usize>>>();
    use_hook(move || count.set(count.get() + 1));
    rsx! { span { "Overlay" } }
}

#[test]
fn rapid_regrab_of_the_same_item_remounts_overlay_placement_hooks() {
    fn app() -> Element {
        rsx! {
            DraggableView {
                key_gen: |item: u32| item.to_string(),
                render: |_: u32| rsx! { OverlayMountProbe {} },
                CaptureDrag {}
            }
        }
    }
    let capture: DragCapture = Rc::new(RefCell::new(None));
    let mounts = Rc::new(Cell::new(0_usize));
    let mut dom = VirtualDom::new(app)
        .with_root_context(capture.clone())
        .with_root_context(mounts.clone());
    dom.rebuild_in_place();
    let (mut context, mut session) = capture.borrow().unwrap();
    dom.in_scope(ScopeId::APP, || {
        session.generation.set(1);
        context.active.set(Some(1));
    });
    dom.render_immediate_to_vec();
    assert_eq!(mounts.get(), 1);

    // Release and pickup arrive before another render. The final active
    // value is still Some(1), but its overlay must get a fresh lifecycle.
    dom.in_scope(ScopeId::APP, || {
        context.finish_drag(session, false);
        session.generation.with_mut(|value| *value += 1);
        context.active.set(Some(1));
    });
    dom.render_immediate_to_vec();
    assert_eq!(mounts.get(), 2);
}

#[test]
fn a_passive_overlay_keeps_the_live_nested_content_mounted_once() {
    #[derive(Clone)]
    struct Counts {
        live: Rc<Cell<usize>>,
        overlay: Rc<Cell<usize>>,
    }
    #[component]
    fn LiveColumn(item: u32) -> Element {
        let counts = use_context::<Counts>();
        use_hook(move || counts.live.set(counts.live.get() + 1));
        rsx! { div { id: "live-list-{item}", "Live nested content" } }
    }
    #[component]
    fn PassiveColumn() -> Element {
        let counts = use_context::<Counts>();
        use_hook(move || counts.overlay.set(counts.overlay.get() + 1));
        rsx! { div { "Passive snapshot" } }
    }
    fn app() -> Element {
        let items = use_signal(|| vec![1_u32, 2]);
        let capture = use_context::<Rc<Cell<Option<Signal<Vec<u32>>>>>>();
        use_hook(move || capture.set(Some(items)));
        rsx! { DraggableView {
            key_gen: |id: u32| id.to_string(),
            render: |item: u32| rsx! { LiveColumn { item } },
            overlay_render: |_: u32| rsx! { PassiveColumn {} },
            CaptureDrag {}
            crate::prelude::Droppable { items, layout: DragLayout::Relative, axis: crate::DragAxis::Horizontal }
        } }
    }
    let counts = Counts {
        live: Rc::new(Cell::new(0)),
        overlay: Rc::new(Cell::new(0)),
    };
    let capture: DragCapture = Rc::new(RefCell::new(None));
    let item_capture = Rc::new(Cell::new(None::<Signal<Vec<u32>>>));
    let mut dom = VirtualDom::new(app)
        .with_root_context(counts.clone())
        .with_root_context(capture.clone())
        .with_root_context(item_capture.clone());
    dom.rebuild_in_place();
    dom.render_immediate_to_vec();
    let (mut context, session) = capture.borrow().unwrap();
    dom.in_scope(context.active.origin_scope(), || {
        context.active.set(Some(1))
    });
    dom.render_immediate_to_vec();
    assert_eq!(counts.live.get(), 2);
    assert_eq!(counts.overlay.get(), 1);
    let mut items = item_capture.get().unwrap();
    dom.in_scope(items.origin_scope(), || items.set(vec![2, 1]));
    dom.render_immediate_to_vec();
    assert_eq!(
        counts.live.get(),
        2,
        "moving keyed columns must preserve their live child scopes"
    );
    dom.in_scope(context.active.origin_scope(), || {
        context.finish_drag(session, false)
    });
    dom.render_immediate_to_vec();
    assert_eq!(counts.live.get(), 2);
}

#[test]
fn nested_views_isolate_sessions_and_share_the_drag_and_validation_lease() {
    use crate::{
        DropEvent, DropResult,
        context::{list::ListAccess, transaction::DndTransactionContext},
    };
    #[derive(Clone, Copy)]
    struct Nested {
        tasks: DndTransactionContext<u32>,
        columns: DndTransactionContext<String>,
        task_context: DndContext<u32>,
        column_context: DndContext<String>,
        task_session: DragSession<u32>,
        column_session: DragSession<String>,
        items: Signal<Vec<u32>>,
    }
    type Capture = Rc<RefCell<Option<Nested>>>;
    type Requests = Rc<RefCell<Vec<DropEvent<u32>>>>;
    #[component]
    fn Probe() -> Element {
        let mut tasks = use_context::<DndTransactionContext<u32>>();
        let mut columns = use_context::<DndTransactionContext<String>>();
        let task_context = use_context::<DndContext<u32>>();
        let column_context = use_context::<DndContext<String>>();
        let task_session = use_context::<DragSession<u32>>();
        let column_session = use_context::<DragSession<String>>();
        let items = use_signal(|| vec![1_u32, 2]);
        let lists = use_signal(|| vec!["a".to_string(), "b".to_string()]);
        let capture = use_context::<Capture>();
        use_hook(move || {
            tasks.register(ListAccess {
                owner: ScopeId(100),
                id: "tasks".into(),
                items: items.into(),
                group: None,
            });
            columns.register(ListAccess {
                owner: ScopeId(101),
                id: "columns".into(),
                items: lists.into(),
                group: None,
            });
            *capture.borrow_mut() = Some(Nested {
                tasks,
                columns,
                task_context,
                column_context,
                task_session,
                column_session,
                items,
            });
        });
        rsx! {}
    }
    fn app() -> Element {
        let requests = use_context::<Requests>();
        rsx! { DraggableView {
            key_gen: |id: u32| id.to_string(), render: |_: u32| rsx! {},
            on_drop: move |event: DropEvent<u32>| requests.borrow_mut().push(event),
            DraggableView {
                key_gen: |id: String| id, render: |_: String| rsx! {},
                Probe {}
            }
        } }
    }
    let capture: Capture = Rc::new(RefCell::new(None));
    let requests: Requests = Rc::new(RefCell::new(Vec::new()));
    let mut dom = VirtualDom::new(app)
        .with_root_context(capture.clone())
        .with_root_context(requests.clone());
    dom.rebuild_in_place();
    let mut nested = capture.borrow().unwrap();
    let task_scope = nested.task_context.active.origin_scope();
    let column_scope = nested.column_context.active.origin_scope();
    assert_ne!(
        nested.task_session.generation,
        nested.column_session.generation
    );
    dom.in_scope(task_scope, || {
        nested.task_session.generation.set(1);
        assert!(nested.tasks.begin("1".into(), 1));
    });
    dom.in_scope(column_scope, || {
        assert!(!nested.columns.can_start());
        nested
            .column_context
            .finish_drag(nested.column_session, false);
    });
    dom.in_scope(nested.items.origin_scope(), || nested.items.set(vec![2, 1]));
    dom.in_scope(task_scope, || {
        assert_eq!(*nested.task_session.generation.peek(), 1);
        nested.tasks.mark_moved();
        nested.tasks.finish_independent(1, false);
        assert!(nested.tasks.state.is_pending());
    });
    dom.in_scope(column_scope, || assert!(!nested.columns.can_start()));
    let event = requests.borrow()[0].clone();
    assert_eq!(event.accept(), DropResult::Applied);
    dom.in_scope(column_scope, || {
        assert!(nested.columns.begin("a".into(), 2))
    });
    dom.in_scope(task_scope, || assert!(!nested.tasks.can_start()));
    dom.in_scope(column_scope, || {
        nested.columns.cancel(crate::DragCancelReason::Cancelled)
    });
    dom.in_scope(task_scope, || assert!(nested.tasks.can_start()));
}
