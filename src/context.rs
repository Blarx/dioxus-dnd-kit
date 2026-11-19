//! Base DnD owns physical input, list registration and reversible transactions.
//! ConnectedLists adds group geometry and proposes destinations to this base.

pub(crate) mod list;
pub(crate) mod transaction;

use std::{marker::PhantomData, rc::Rc};

use dioxus::{
    html::geometry::{ClientPoint, PixelsRect, PixelsVector2D},
    prelude::*,
};
use dioxus_floating::{OffsetOptions, ScrollableView};

use crate::{
    DndItem,
    context::transaction::{DndTransactionContext, Options},
    hooks::{use_window_drag_events, use_window_on_resize, use_window_on_scroll},
    prelude::DraggableOverlay,
    {DragCancel, DragStart, DropChange, DropEvent},
};

/// Invalidates in-flight pickup measurements when the button is released or a new drag starts.
/// Kept separate from the public context so existing context consumers remain compatible.
pub(crate) struct DragSession<T: DndItem> {
    pub generation: Signal<u64>,
    pub cancelled: Signal<bool>,
    pub last_end: Signal<Option<DragEnd>>,
    marker: PhantomData<fn() -> T>,
}

impl<T: DndItem> Copy for DragSession<T> {}
impl<T: DndItem> Clone for DragSession<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: DndItem> DragSession<T> {
    pub(crate) fn new() -> Self {
        Self {
            generation: Signal::new(0),
            cancelled: Signal::new(false),
            last_end: Signal::new(None),
            marker: PhantomData,
        }
    }
}

/// One lease for the entire nested view tree, including pending validation.
#[derive(Clone, Copy)]
pub(crate) struct DragCoordinator {
    owner: Signal<Option<ScopeId>>,
}
impl DragCoordinator {
    pub(crate) fn new() -> Self {
        Self {
            owner: Signal::new(None),
        }
    }
    pub(crate) fn available(self) -> bool {
        self.owner.try_peek().is_ok_and(|owner| owner.is_none())
    }
    pub(crate) fn acquire(mut self, scope: ScopeId) -> bool {
        if !self.available() {
            return false;
        }
        self.owner.set(Some(scope));
        true
    }
    pub(crate) fn release(mut self, scope: ScopeId) {
        if self
            .owner
            .try_peek()
            .is_ok_and(|owner| *owner == Some(scope))
        {
            self.owner.set(None);
        }
    }
    pub(crate) fn is_busy(self) -> bool {
        self.owner.read().is_some()
    }
}

pub(crate) struct OverlayRenderer<T: DndItem>(pub Memo<Callback<T, Element>>);
impl<T: DndItem> Copy for OverlayRenderer<T> {}
impl<T: DndItem> Clone for OverlayRenderer<T> {
    fn clone(&self) -> Self {
        *self
    }
}

#[derive(Clone, Copy)]
pub(crate) struct DragPosition {
    pub mouse: ClientPoint,
    pub rect: Option<PixelsRect>,
}

#[derive(Clone, Copy)]
pub(crate) struct DragEnd {
    pub generation: u64,
    pub cancelled: bool,
    pub position: DragPosition,
}

/// State management for a Drag-and-Drop session.
#[derive(Debug, Clone, PartialEq)]
pub struct DndContext<T: DndItem> {
    /// How to render the item in the list and overlay.
    pub render: Callback<T, Element>,
    /// Optional placeholder renderer used when an item is lifted.
    pub placeholder_render: Option<Callback<(f64, f64), Element>>,
    /// Current active (dragged) item data.
    /// Clearing it ends the drag; changed list moves still go through `on_drop`.
    pub active: Signal<Option<T>>,
    /// Rect of the active item before it was lifted.
    pub active_rect: Signal<Option<PixelsRect>>,
    /// Internal: Reference to the mounted data of the active element.
    pub active_mounted: Signal<Option<Rc<MountedData>>>,
    /// Flag indicating that the floating overlay is ready for display.
    pub is_floating_ready: Signal<bool>,
    /// Current mouse coordinates relative to the viewport.
    pub mouse_pos: Signal<ClientPoint>,
    /// Mouse coordinates at the start of the drag operation.
    pub start_pos: Signal<ClientPoint>,
    /// Distance between the grab point and the element's top-left corner.
    pub grab_offset: Signal<OffsetOptions>,
    /// Key generator for DOM stability.
    pub key_gen: Callback<T, String>,
    /// Global trigger to force recalculation of element rectangles.
    pub recalculate_rects: Signal<u64>,
}

impl<T: DndItem> Copy for DndContext<T> {}

impl<T: DndItem> DndContext<T> {
    pub(crate) fn set_mouse_position(&mut self, position: ClientPoint) {
        if *self.mouse_pos.peek() != position {
            self.mouse_pos.set(position);
        }
    }

    pub(crate) fn drag_position(&self, mouse: ClientPoint) -> DragPosition {
        let start = *self.start_pos.peek();
        DragPosition {
            mouse,
            rect: (*self.active_rect.peek()).map(|rect| {
                rect.translate(PixelsVector2D::new(mouse.x - start.x, mouse.y - start.y))
            }),
        }
    }

    pub(crate) fn finish_drag(&mut self, mut session: DragSession<T>, cancelled: bool) {
        let generation = *session.generation.peek();
        session.generation.with_mut(|value| *value += 1);
        // Keep cancellation latched until the next pickup. A mouseup following
        // Escape must not turn that same drag into a successful drop.
        if cancelled {
            session.cancelled.set(true);
        }
        if self.active.peek().is_some() {
            session.last_end.set(Some(DragEnd {
                generation,
                cancelled: *session.cancelled.peek(),
                position: self.drag_position(*self.mouse_pos.peek()),
            }));
        }
        self.active.set(None);
        self.is_floating_ready.set(false);
        self.active_mounted.set(None);
        self.active_rect.set(None);
        self.start_pos.set(ClientPoint::zero());
    }

    fn new(
        key_gen: Callback<T, String>,
        render: Callback<T, Element>,
        placeholder_render: Option<Callback<(f64, f64), Element>>,
    ) -> Self {
        Self {
            key_gen,
            render,
            placeholder_render,
            active: Signal::new(None),
            active_mounted: Signal::new(None),
            active_rect: Signal::new(None),
            is_floating_ready: Signal::new(false),
            mouse_pos: Signal::new(ClientPoint::zero()),
            start_pos: Signal::new(ClientPoint::zero()),
            grab_offset: Signal::new(OffsetOptions::default()),
            recalculate_rects: Signal::new(0),
        }
    }
}

/// The root container and manager for the Drag-and-Drop area.
///
/// `DraggableView` initializes the [`DndContext`], handles mouse events (move/up),
/// and manages both the visual drag and its reversible list transaction.
/// [`Droppable`](crate::prelude::Droppable) calculates local sort positions;
/// [`ConnectedLists`](crate::prelude::ConnectedLists) extends the same transaction
/// API to transfers between lists. Their item signals change live during previews.
///
/// Use `on_drop` for asynchronous persistence and [`use_dnd_state`](crate::use_dnd_state)
/// in a descendant component for reactive validation UI. While a decision is
/// pending, the whole nested view tree blocks new pickups. Without `on_drop`,
/// changed drops accept immediately. See [`DropEvent`] for decisions and Undo.
///
/// ### Features:
/// - **Selection Lock**: Automatically toggles `user-select: none` during dragging to prevent text highlighting.
/// - **Overlay Management**: Renders the [`DraggableOverlay`] automatically when an item is picked up.
/// - **Stable Landing**: Restores the original and removes the overlay in the same update.
#[component]
pub fn DraggableView<T: DndItem>(
    /// CSS classes for the outer container.
    #[props(default)]
    class: ReadSignal<String>,

    /// Content containing [`Draggable`](crate::prelude::Draggable) items and
    /// [`Droppable`](crate::prelude::Droppable) zones.
    children: Element,

    /// A callback to generate a stable, unique key for each item.
    key_gen: Callback<T, String>,

    /// The primary rendering callback used for both the list items and the dragging ghost.
    render: Callback<T, Element>,

    /// An optional callback to render a placeholder that stays in the list while an item is being dragged.
    /// Receives `(height, width)` in pixels.
    placeholder_render: Option<Callback<(f64, f64), Element>>,
    /// Optional passive overlay renderer for items containing nested lists.
    /// Defaults to `render`; avoid mounting live Droppables in the overlay copy.
    overlay_render: Option<Callback<T, Element>>,
    /// Called once after release at a changed, eligible list position.
    ///
    /// The optimistic move is already in the item signals. Accept or reject the
    /// [`DropEvent`], including after an await. Releasing its last unresolved clone
    /// attempts rejection automatically; retaining one keeps validation pending.
    /// Omitting this callback accepts changes immediately. Hover updates,
    /// unchanged drops, and cancelled drags do not call it.
    on_drop: Option<Callback<DropEvent<T>>>,
    /// Fast, synchronous eligibility predicate for a proposed position.
    ///
    /// Receives a [`DropChange`] with the pickup source and proposed destination.
    /// It can run repeatedly during preview and again at release. Return `false`
    /// to deny the candidate; keep this predicate free of mutations and network
    /// requests. Asynchronous business validation belongs in `on_drop`.
    can_drop: Option<Callback<DropChange<T>, bool>>,
    /// Called once at pickup from a registered list, before any preview changes.
    /// Receives [`DragStart`] with the original item, key, and list position.
    on_drag_start: Option<Callback<DragStart<T>>>,
    /// Called when a drag is cancelled before creating a drop request, after
    /// attempting restoration. Inspect [`DragCancel::rollback`] for its result.
    /// Rejection of a pending request is reported through [`DropOutcome`](crate::DropOutcome).
    on_drag_cancel: Option<Callback<DragCancel<T>>>,
) -> Element {
    let mut context =
        use_context_provider(move || DndContext::new(key_gen, render, placeholder_render));
    let session = use_context_provider(DragSession::<T>::new);
    let ancestor_coordinator = try_use_context::<DragCoordinator>();
    let coordinator =
        use_context_provider(move || ancestor_coordinator.unwrap_or_else(DragCoordinator::new));
    let overlay_renderer = use_memo(use_reactive(
        (&overlay_render, &render),
        |(overlay_render, render)| overlay_render.unwrap_or(render),
    ));
    use_context_provider(move || OverlayRenderer(overlay_renderer));
    let mut transactions = use_context_provider(move || {
        DndTransactionContext::new(
            key_gen,
            Options {
                on_drop,
                can_drop,
                on_drag_start,
                on_drag_cancel,
            },
            coordinator,
        )
    });
    use_effect(use_reactive(
        (&on_drop, &can_drop, &on_drag_start, &on_drag_cancel),
        move |(on_drop, can_drop, on_drag_start, on_drag_cancel)| {
            transactions.configure(Options {
                on_drop,
                can_drop,
                on_drag_start,
                on_drag_cancel,
            });
        },
    ));
    use_effect(move || {
        let active = (context.active)();
        let end = (session.last_end)();
        let mut session = session;
        if active.is_some() && transactions.can_start() {
            session.cancelled.set(false);
        }
        transactions.observe_drag(
            active,
            *session.generation.peek(),
            end,
            (session.cancelled)(),
        );
    });
    use_drop(move || transactions.shutdown());

    let dragging = use_memo(move || context.active.read().is_some());
    use_window_drag_events(
        dragging,
        use_callback(move |event: [f64; 3]| match event[0] as u8 {
            0 => context.set_mouse_position(ClientPoint::new(event[1], event[2])),
            1 => {
                context.set_mouse_position(ClientPoint::new(event[1], event[2]));
                context.finish_drag(session, false);
            }
            _ => context.finish_drag(session, true),
        }),
    );
    use_window_on_resize(use_callback(move |_| {
        context.recalculate_rects.with_mut(|r| *r += 1);
    }));
    use_window_on_scroll(use_callback(move |_| {
        if context.active.peek().is_some() {
            context.recalculate_rects.with_mut(|r| *r += 1);
        }
    }));

    rsx! {
        ScrollableView { class,
            style: if context.active.read().is_some() {
                "user-select: none; -webkit-user-select: none;"
            } else {
                "user-select: auto; -webkit-user-select: auto;"
            },
            on_mouse_move: move |e: MouseEvent| {
                context.set_mouse_position(e.client_coordinates());
            },
            {children}
            if let Some(active_item) = context.active.read().as_ref() {
                // A new pickup must get fresh placement hooks even if a very
                // quick release/regrab coalesces into a single VDOM render.
                DraggableOverlay::<T> { key: "{(session.generation)()}", item: active_item.clone() }
            }
        }
    }
}

#[cfg(test)]
mod tests;
