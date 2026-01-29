use std::rc::Rc;

use dioxus::{
    html::geometry::{ClientPoint, PixelsRect},
    prelude::*,
};
use dioxus_floating::{OffsetOptions, ScrollableView};

use crate::{DndItem, hooks::use_body_on_mouse_up, prelude::DraggableOverlay};

/// State management for a Drag-and-Drop session.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DndContext<T: DndItem> {
    /// How to render the item in the list and overlay.
    pub render: Callback<T, Element>,
    /// Optional placeholder renderer used when an item is lifted.
    pub placeholder_render: Option<Callback<(f64, f64), Element>>,
    /// Current active (dragged) item data.
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

impl<T: DndItem> DndContext<T> {
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
/// `DraggableView` initializes the [`DndContext`], handles global mouse events (move/up),
/// and manages the visual lifecycle of the dragged item.
///
/// ### Features:
/// - **Selection Lock**: Automatically toggles `user-select: none` during dragging to prevent text highlighting.
/// - **Overlay Management**: Renders the [`DraggableOverlay`] automatically when an item is picked up.
/// - **Smooth Landing**: Implements a short delay before clearing the active state to prevent layout flickering.
#[component]
pub fn DraggableView<T: DndItem>(
    /// CSS classes for the outer container.
    #[props(default)]
    class: ReadSignal<String>,

    /// Content containing [`Draggable`] items and [`Droppable`] zones.
    children: Element,

    /// A callback to generate a stable, unique key for each item.
    key_gen: Callback<T, String>,

    /// The primary rendering callback used for both the list items and the dragging ghost.
    render: Callback<T, Element>,

    /// An optional callback to render a placeholder that stays in the list while an item is being dragged.
    /// Receives `(height, width)` in pixels.
    placeholder_render: Option<Callback<(f64, f64), Element>>,
) -> Element {
    let mut context =
        use_context_provider(move || DndContext::new(key_gen, render, placeholder_render));

    let mouseup = use_callback(move |_| {
        context.is_floating_ready.set(false);
        spawn(async move {
            gloo_timers::future::TimeoutFuture::new(50).await;
            context.active.set(None);
            context.active_mounted.set(None);
            context.active_rect.set(None);
            context.start_pos.set(ClientPoint::zero());
        });
    });
    use_body_on_mouse_up(mouseup);

    rsx! {
        ScrollableView { class,
            style: if context.active.read().is_some() {
                "user-select: none; -webkit-user-select: none;"
            } else {
                "user-select: auto; -webkit-user-select: auto;"
            },
            on_mouse_move: move |e: MouseEvent| {
                context.mouse_pos.set(e.client_coordinates());
            },
            // on_mouse_up: move |_| {
            //     context.active.set(None);
            //     context.start_pos.set(ClientPoint::zero());
            // },
            {children}
            if let Some(active_item) = context.active.read().as_ref() {
                DraggableOverlay::<T> { item: active_item.clone() }
            }
        }
    }
}
