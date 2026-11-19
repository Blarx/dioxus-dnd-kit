use super::*;
use std::cell::RefCell;

#[derive(Clone)]
struct Board {
    transactions: DndTransactionContext<u32>,
    source: Signal<Vec<u32>>,
    target: Signal<Vec<u32>>,
    events: Rc<RefCell<Vec<DropEvent<u32>>>>,
    cancellations: Rc<RefCell<Vec<DragCancel<u32>>>>,
}

impl Board {
    fn new() -> Self {
        let events = Rc::new(RefCell::new(Vec::new()));
        let cancellations = Rc::new(RefCell::new(Vec::new()));
        let mut transactions = DndTransactionContext::new(
            Callback::new(|item: u32| item.to_string()),
            Options {
                on_drop: Some(Callback::new({
                    let events = events.clone();
                    move |event| events.borrow_mut().push(event)
                })),
                can_drop: None,
                on_drag_start: None,
                on_drag_cancel: Some(Callback::new({
                    let cancellations = cancellations.clone();
                    move |event| cancellations.borrow_mut().push(event)
                })),
            },
            DragCoordinator::new(),
        );
        let source = Signal::new(vec![1, 2, 3]);
        let target = Signal::new(vec![4]);
        transactions.register(ListAccess {
            owner: ScopeId(1),
            id: "source".into(),
            items: source.into(),
            group: None,
        });
        transactions.register(ListAccess {
            owner: ScopeId(2),
            id: "target".into(),
            items: target.into(),
            group: None,
        });
        Self {
            transactions,
            source,
            target,
            events,
            cancellations,
        }
    }

    fn transfer(&mut self, generation: u64) -> DropEvent<u32> {
        assert!(self.transactions.begin("2".into(), generation));
        assert!(move_item(
            self.source,
            self.target,
            "2",
            1,
            self.transactions.key_gen
        ));
        self.transactions.mark_moved();
        self.transactions.finish(generation, None);
        self.events.borrow().last().unwrap().clone()
    }
}

type Case = Rc<RefCell<Option<Box<dyn FnOnce(Board)>>>>;
fn run(case: impl FnOnce(Board) + 'static) {
    fn app() -> Element {
        let board = use_hook(Board::new);
        let mut transactions = board.transactions;
        use_drop(move || transactions.shutdown());
        let case = use_context::<Case>();
        use_hook(move || case.borrow_mut().take().unwrap()(board));
        rsx! {}
    }
    let case: Case = Rc::new(RefCell::new(Some(Box::new(case))));
    VirtualDom::new(app)
        .with_root_context(case)
        .rebuild_in_place();
}

#[test]
fn local_sort_and_transfer_use_one_preview_transaction_and_one_drop_event() {
    run(|mut board| {
        assert!(board.transactions.begin("2".into(), 1));
        assert_eq!(board.transactions.preview_move(ScopeId(1), 2), Ok(true));
        assert_eq!(*board.source.peek(), vec![1, 3, 2]);
        assert_eq!(board.transactions.preview_move(ScopeId(1), 2), Ok(false));
        assert_eq!(board.transactions.preview_move(ScopeId(2), 0), Ok(true));
        assert_eq!(*board.source.peek(), vec![1, 3]);
        assert_eq!(*board.target.peek(), vec![2, 4]);
        board.transactions.finish(1, None);
        assert_eq!(board.events.borrow().len(), 1);
        let event = board.events.borrow()[0].clone();
        assert_eq!((event.change.from.index, event.change.to.index), (1, 0));
        assert_eq!(event.reject(), DropResult::Applied);
        assert_eq!(*board.source.peek(), vec![1, 2, 3]);
        assert_eq!(*board.target.peek(), vec![4]);
    });
}

#[test]
fn denied_or_failed_preview_writes_do_not_change_the_list() {
    run(|mut board| {
        let mut options = *board.transactions.options.peek();
        options.can_drop = Some(Callback::new(|_: DropChange<u32>| false));
        board.transactions.configure(options);
        assert!(board.transactions.begin("2".into(), 1));
        assert_eq!(
            board.transactions.preview_move(ScopeId(1), 2),
            Err(DragCancelReason::NotAllowed)
        );
        assert_eq!(*board.source.peek(), vec![1, 2, 3]);
        options.can_drop = None;
        board.transactions.configure(options);
        let borrowed = board.source.peek();
        assert_eq!(
            board.transactions.preview_move(ScopeId(1), 2),
            Err(DragCancelReason::Outside)
        );
        assert_eq!(*borrowed, vec![1, 2, 3]);
        drop(borrowed);
        board.transactions.finish(1, None);
        assert!(board.events.borrow().is_empty());
        assert!(board.transactions.can_start());
    });
}

#[test]
fn one_registry_isolates_groups_and_rejects_ambiguous_keys() {
    run(|board| {
        let mut registry = ListRegistry::new();
        let first = Signal::new(vec![1_u32]);
        let second = Signal::new(vec![1_u32]);
        for (owner, group, items) in [(10, 100, first), (20, 200, second)] {
            assert!(registry.register(ListAccess {
                owner: ScopeId(owner),
                id: "same-id".into(),
                items: items.into(),
                group: Some(ScopeId(group)),
            }));
        }
        assert!(
            registry
                .locate("1", board.transactions.key_gen, None)
                .is_none()
        );
        let entry = registry
            .locate("1", board.transactions.key_gen, Some(ScopeId(100)))
            .unwrap()
            .0;
        assert_eq!(entry.owner, ScopeId(10));
        assert!(!registry.register(ListAccess {
            owner: ScopeId(11),
            id: "alias".into(),
            items: entry.items,
            group: Some(ScopeId(100)),
        }));
        registry.unregister(ScopeId(10));
        assert_eq!(
            registry
                .locate("1", board.transactions.key_gen, None)
                .unwrap()
                .0
                .owner,
            ScopeId(20)
        );
    });
}

#[test]
fn pending_transfer_reports_original_and_final_locations_and_rejects_only_the_item() {
    run(|mut board| {
        let event = board.transfer(1);
        assert_eq!(event.change.key, "2");
        assert_eq!(event.change.from.list_id, "source");
        assert_eq!(event.change.from.index, 1);
        assert_eq!(event.change.from.previous_key.as_deref(), Some("1"));
        assert_eq!(event.change.from.next_key.as_deref(), Some("3"));
        assert_eq!(event.change.to.list_id, "target");
        assert_eq!(event.change.to.index, 1);
        assert!(board.transactions.state.is_pending());
        assert!(!board.transactions.can_start());
        board.source.with_mut(|items| items.insert(0, 9));
        board.target.with_mut(|items| items.push(8));
        assert_eq!(event.reject(), DropResult::Applied);
        assert_eq!(event.status(), DropStatus::Rejected);
        assert_eq!(*board.source.peek(), vec![9, 1, 2, 3]);
        assert_eq!(*board.target.peek(), vec![4, 8]);
        assert!(!board.transactions.state.is_pending());
        assert!(board.transactions.can_start());
        assert_eq!(event.accept(), DropResult::AlreadyResolved);
    });
}

#[test]
fn an_accepted_drop_can_be_undone_once() {
    run(|mut board| {
        let event = board.transfer(1);
        assert_eq!(event.accept(), DropResult::Applied);
        assert!(!board.transactions.state.is_pending());
        assert_eq!(event.undo(), DropResult::Applied);
        assert_eq!(event.undo(), DropResult::AlreadyResolved);
        assert_eq!(*board.source.peek(), vec![1, 2, 3]);
        assert_eq!(*board.target.peek(), vec![4]);
    });
}

#[test]
fn authoritative_relocation_is_never_overwritten_by_a_late_rejection() {
    run(|mut board| {
        let event = board.transfer(1);
        board.target.set(vec![2, 4]);
        assert_eq!(event.reject(), DropResult::Conflict);
        assert_eq!(event.status(), DropStatus::Conflict);
        assert_eq!(*board.source.peek(), vec![1, 3]);
        assert_eq!(*board.target.peek(), vec![2, 4]);
        assert!(!board.transactions.state.is_pending());
    });
}

#[test]
fn same_list_sort_is_one_request_and_rejection_restores_its_position() {
    run(|mut board| {
        assert!(board.transactions.begin("2".into(), 1));
        board.source.set(vec![1, 3, 2]);
        board.transactions.mark_moved();
        board.transactions.finish_independent(1, false);
        let event = board.events.borrow()[0].clone();
        assert_eq!(board.events.borrow().len(), 1);
        assert_eq!(event.change.from.list_id, "source");
        assert_eq!(event.change.to.list_id, "source");
        assert_eq!((event.change.from.index, event.change.to.index), (1, 2));
        assert_eq!(event.reject(), DropResult::Applied);
        assert_eq!(*board.source.peek(), vec![1, 2, 3]);
        assert_eq!(
            board.transactions.state.last_outcome().unwrap().status,
            DropStatus::Rejected
        );
    });
}

#[test]
fn noop_and_drag_cancellation_do_not_emit_drop_requests() {
    run(|mut board| {
        assert!(board.transactions.begin("2".into(), 1));
        board.transactions.mark_moved(); // moved away and returned to the pickup position
        board.transactions.finish_independent(1, false);
        assert!(board.events.borrow().is_empty());
        assert!(board.transactions.begin("2".into(), 2));
        board.source.set(vec![1, 3, 2]);
        board.transactions.mark_moved();
        board.transactions.finish_independent(2, true);
        assert_eq!(*board.source.peek(), vec![1, 2, 3]);
        assert!(board.events.borrow().is_empty());
        let cancelled = board.cancellations.borrow();
        assert_eq!(cancelled.len(), 1);
        assert_eq!(cancelled[0].reason, DragCancelReason::Cancelled);
        assert_eq!(cancelled[0].rollback, DropResult::Applied);
    });
}

#[test]
fn source_is_the_pickup_list_even_after_multiple_preview_transfers() {
    run(|mut board| {
        let third = Signal::new(vec![]);
        board.transactions.register(ListAccess {
            owner: ScopeId(3),
            id: "third".into(),
            items: third.into(),
            group: Some(ScopeId(99)),
        });
        assert!(board.transactions.begin("2".into(), 1));
        assert!(move_item(
            board.source,
            board.target,
            "2",
            1,
            board.transactions.key_gen
        ));
        assert!(move_item(
            board.target,
            third,
            "2",
            0,
            board.transactions.key_gen
        ));
        board.transactions.mark_moved();
        board.transactions.finish(1, None);
        let event = board.events.borrow()[0].clone();
        assert_eq!(event.change.from.list_id, "source");
        assert_eq!(event.change.to.list_id, "third");
        assert_eq!(event.reject(), DropResult::Applied);
        assert_eq!(*board.source.peek(), vec![1, 2, 3]);
        assert!(third.peek().is_empty());
    });
}

#[test]
fn can_drop_is_checked_for_candidates_and_again_before_the_request() {
    run(|mut board| {
        let mut allow = Signal::new(false);
        let mut options = *board.transactions.options.peek();
        options.can_drop = Some(Callback::new(move |change: DropChange<u32>| {
            assert_eq!(change.from.list_id, "source");
            assert_eq!(change.to.list_id, "target");
            allow()
        }));
        board.transactions.configure(options);
        assert!(board.transactions.begin("2".into(), 1));
        assert!(!board.transactions.allows(ScopeId(2), 1));
        allow.set(true);
        assert!(board.transactions.allows(ScopeId(2), 1));
        assert!(move_item(
            board.source,
            board.target,
            "2",
            1,
            board.transactions.key_gen
        ));
        board.transactions.mark_moved();
        allow.set(false);
        board.transactions.finish(1, None);
        assert!(board.events.borrow().is_empty());
        assert_eq!(*board.source.peek(), vec![1, 2, 3]);
        assert_eq!(
            board.cancellations.borrow()[0].reason,
            DragCancelReason::NotAllowed
        );
    });
}

#[test]
fn no_handler_preserves_immediate_acceptance() {
    run(|mut board| {
        let mut options = *board.transactions.options.peek();
        options.on_drop = None;
        board.transactions.configure(options);
        assert!(board.transactions.begin("2".into(), 1));
        board.source.set(vec![1, 3, 2]);
        board.transactions.mark_moved();
        board.transactions.finish_independent(1, false);
        assert!(!board.transactions.state.is_pending());
        assert_eq!(*board.source.peek(), vec![1, 3, 2]);
        assert_eq!(
            board.transactions.state.last_outcome().unwrap().status,
            DropStatus::Accepted
        );
    });
}

#[test]
fn dropping_the_last_pending_event_rejects_instead_of_leaving_the_view_locked() {
    run(|mut board| {
        let event = board.transfer(1);
        board.events.borrow_mut().clear();
        assert!(board.transactions.state.is_pending());
        drop(event);
        assert!(board.transactions.can_start());
        assert_eq!(*board.source.peek(), vec![1, 2, 3]);
        assert_eq!(*board.target.peek(), vec![4]);
    });
}

#[test]
fn newer_changes_invalidate_old_undo_and_pending_blocks_new_pickups() {
    run(|mut board| {
        let first = board.transfer(1);
        assert!(!board.transactions.begin("1".into(), 2));
        assert_eq!(first.accept(), DropResult::Applied);
        assert!(board.transactions.begin("1".into(), 3));
        assert_eq!(first.undo(), DropResult::Busy);
        board.source.set(vec![3, 1]);
        board.transactions.mark_moved();
        board.transactions.finish(3, None);
        assert_eq!(first.status(), DropStatus::Invalidated);
        assert_eq!(first.undo(), DropResult::Unavailable);
        let second = board.events.borrow()[1].clone();
        assert_eq!(second.accept(), DropResult::Applied);
        assert_eq!(first.reject(), DropResult::Unavailable);
        assert_eq!(*board.source.peek(), vec![3, 1]);
        assert_eq!(*board.target.peek(), vec![4, 2]);
    });
}

#[test]
fn removing_a_participating_list_rolls_back_and_invalidates_late_decisions() {
    run(|mut board| {
        let event = board.transfer(1);
        board.transactions.unregister(ScopeId(2));
        assert_eq!(*board.source.peek(), vec![1, 2, 3]);
        assert_eq!(*board.target.peek(), vec![4]);
        assert!(!board.transactions.state.is_pending());
        assert_eq!(event.accept(), DropResult::Unavailable);
        assert_eq!(event.reject(), DropResult::Unavailable);
    });
}

#[test]
fn an_authoritatively_deleted_item_is_not_resurrected() {
    run(|mut board| {
        let event = board.transfer(1);
        board.target.set(vec![4]);
        assert_eq!(event.reject(), DropResult::Conflict);
        assert_eq!(*board.source.peek(), vec![1, 3]);
        assert_eq!(*board.target.peek(), vec![4]);
        assert!(board.transactions.can_start());
    });
}

#[test]
fn rollback_keeps_current_item_data_when_its_stable_key_is_unchanged() {
    run(|mut board| {
        // The low digit is identity; the high digit represents editable data.
        board.transactions.key_gen = Callback::new(|item: u32| (item % 10).to_string());
        let event = board.transfer(1);
        board.target.set(vec![4, 12]);
        assert_eq!(event.reject(), DropResult::Applied);
        assert_eq!(*board.source.peek(), vec![1, 12, 3]);
        assert_eq!(*board.target.peek(), vec![4]);
    });
}

#[test]
fn a_handler_that_forgets_to_resolve_rejects_synchronously_without_deadlock() {
    run(|mut board| {
        let mut options = *board.transactions.options.peek();
        options.on_drop = Some(Callback::new(|_: DropEvent<u32>| {}));
        board.transactions.configure(options);
        assert!(board.transactions.begin("2".into(), 1));
        assert!(move_item(
            board.source,
            board.target,
            "2",
            1,
            board.transactions.key_gen
        ));
        board.transactions.mark_moved();
        board.transactions.finish(1, None);
        assert!(board.transactions.can_start());
        assert_eq!(*board.source.peek(), vec![1, 2, 3]);
        assert_eq!(*board.target.peek(), vec![4]);
        assert_eq!(
            board.transactions.state.last_outcome().unwrap().status,
            DropStatus::Rejected
        );
    });
}

#[test]
fn removing_an_unrelated_list_leaves_the_pending_decision_valid() {
    run(|mut board| {
        board.transactions.register(ListAccess {
            owner: ScopeId(3),
            id: "other".into(),
            items: Signal::new(vec![9]).into(),
            group: None,
        });
        let event = board.transfer(1);
        board.transactions.unregister(ScopeId(3));
        assert!(board.transactions.state.is_pending());
        assert_eq!(event.accept(), DropResult::Applied);
        assert_eq!(*board.target.peek(), vec![4, 2]);
    });
}

#[test]
fn shutdown_invalidates_handles_retained_outside_the_view() {
    let captured = Rc::new(RefCell::new(None));
    let saved = captured.clone();
    run(move |mut board| {
        let event = board.transfer(1);
        *saved.borrow_mut() = Some(event);
    });
    let event = captured.borrow_mut().take().unwrap();
    assert_eq!(event.status(), DropStatus::Invalidated);
    assert_eq!(event.accept(), DropResult::Unavailable);
    assert_eq!(event.reject(), DropResult::Unavailable);
}

#[test]
fn an_async_handler_keeps_the_drop_pending_until_the_api_result_arrives() {
    for accepted in [true, false] {
        fn app() -> Element {
            let board = use_hook(Board::new);
            let mut transactions = board.transactions;
            use_drop(move || transactions.shutdown());
            let capture = use_context::<Rc<RefCell<Option<Board>>>>();
            let receiver =
                use_context::<Rc<RefCell<Option<futures::channel::oneshot::Receiver<bool>>>>>();
            use_hook(move || {
                *capture.borrow_mut() = Some(board.clone());
                let mut options = *transactions.options.peek();
                options.on_drop = Some(Callback::new(move |event: DropEvent<u32>| {
                    let receiver = receiver.borrow_mut().take().unwrap();
                    async move {
                        if receiver.await.unwrap_or(false) {
                            event.accept();
                        } else {
                            event.reject();
                        }
                    }
                }));
                transactions.configure(options);
                assert!(transactions.begin("2".into(), 1));
                assert!(move_item(
                    board.source,
                    board.target,
                    "2",
                    1,
                    transactions.key_gen
                ));
                transactions.mark_moved();
                transactions.finish(1, None);
            });
            rsx! {}
        }
        let capture = Rc::new(RefCell::new(None::<Board>));
        let (sender, receiver) = futures::channel::oneshot::channel::<bool>();
        let receiver = Rc::new(RefCell::new(Some(receiver)));
        let mut dom = VirtualDom::new(app)
            .with_root_context(capture.clone())
            .with_root_context(receiver);
        dom.rebuild_in_place();
        dom.render_immediate_to_vec();
        let board = capture.borrow().clone().unwrap();
        assert!(board.transactions.state.is_pending());
        assert_eq!(*board.target.peek(), vec![4, 2]);
        sender.send(accepted).unwrap();
        dom.render_immediate_to_vec();
        assert!(!board.transactions.state.is_pending());
        assert_eq!(
            *board.target.peek(),
            if accepted { vec![4, 2] } else { vec![4] }
        );
        assert_eq!(
            board.transactions.state.last_outcome().unwrap().status,
            if accepted {
                DropStatus::Accepted
            } else {
                DropStatus::Rejected
            }
        );
    }
}
