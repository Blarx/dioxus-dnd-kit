use std::rc::Rc;

use dioxus::{html::geometry::PixelsRect, prelude::*};
use dioxus_floating::{FloatingOptions, OffsetOptions, use_placement_on_point};

use crate::{
    DndItem,
    hooks::{use_dnd_context, use_droppable_context},
};

/// Local state for an individual draggable element.
///
/// Manages references and dimensions for both the item itself
/// and its relative container to ensure stable layout transitions.
#[derive(Debug, Clone, Copy, Default)]
pub struct DraggableContext {
    /// Reference to the actual draggable element node.
    pub draggable_ref: Signal<Option<Rc<MountedData>>>,
    /// Last measured bounding box of the element.
    pub draggable_rect: Signal<Option<PixelsRect>>,
    /// Reference to the outer relative container (used for positioning stability).
    pub draggable_container_ref: Signal<Option<Rc<MountedData>>>,
    /// Bounding box of the container.
    pub draggable_container_rect: Signal<Option<PixelsRect>>,
}

impl DraggableContext {
    /// Get the draggable rect
    pub fn rect(&self) -> PixelsRect {
        (*self.draggable_rect.read()).unwrap_or(PixelsRect::zero())
    }
}

/// A wrapper component that makes its content draggable.
///
/// It handles layout stability by providing a placeholder and switching the
/// original element to `position: absolute` when it's being dragged.
#[component]
pub fn Draggable<T: DndItem>(
    /// Index of the item within the list (useful for sorting logic).
    index: usize,
    /// The data associated with this draggable item.
    item: T,
    /// Optional signals for CSS classes.
    #[props(default)]
    class: ReadSignal<String>,
    /// Content that will be rendered inside the draggable container.
    children: Element,
) -> Element {
    let context = use_dnd_context::<T>();
    let mut droppable_context = use_droppable_context::<T>();
    let mut draggable_context = use_context_provider(DraggableContext::default);
    let trigger_item = item.clone();
    let is_active = use_memo(move || {
        context
            .active
            .read()
            .clone()
            .map(|active_item| active_item.eq(&trigger_item))
            .unwrap_or(false)
    });
    let is_ready_to_hide = use_memo(move || is_active() && (context.is_floating_ready)());

    let register_item = item.clone();
    use_effect(move || {
        if let Some(draggable) = (draggable_context.draggable_ref)() {
            droppable_context.register_element_mounted(register_item.clone(), draggable.clone());
        }
    });

    use_effect(move || {
        // subscribe to recalculation logic.
        let _ = (context.recalculate_rects)();

        // Clear draggable rect when recalculate.
        if draggable_context.draggable_rect.peek().is_some() {
            draggable_context.draggable_rect.set(None);
        }

        if let Some(draggable) = (draggable_context.draggable_ref)() {
            spawn(async move {
                gloo_timers::future::TimeoutFuture::new(10).await;
                let rect = draggable
                    .get_client_rect()
                    .await
                    .unwrap_or(PixelsRect::zero());
                draggable_context.draggable_rect.set(Some(rect));
                draggable_context.draggable_container_rect.set(Some(rect));
            });
        } else {
            draggable_context.draggable_rect.set(None);
            draggable_context.draggable_container_rect.set(None);
        }
    });

    // Idle: span full droppable width so layout tracks window/container resize. While
    // dragging: fixed hole matching the grabbed element so the overlay lines up.
    let placeholder_style = use_memo(move || {
        if let Some(rect) = (draggable_context.draggable_rect)() {
            if is_ready_to_hide() {
                format!(
                    "box-sizing: border-box; visibility: visible; pointer-events: none; width: {}px; height: {}px; flex-shrink: 0;",
                    rect.width(),
                    rect.height()
                )
            } else {
                format!(
                    "box-sizing: border-box; visibility: hidden; pointer-events: none; width: 100%; min-height: {}px; height: auto;",
                    rect.height()
                )
            }
        } else {
            format!(
                "box-sizing: border-box; visibility: hidden; pointer-events: none; width: 100%; height: auto;",
            )
        }
    });

    let draggable_style = use_memo(move || {
        format!(
            "position: absolute; top: 0; left: 0; right: 0; opacity: {};",
            if is_ready_to_hide() { 0 } else { 1 }
        )
    });

    rsx! {
        div { class: "relative",
            onmounted: move |evt: MountedEvent| {
                draggable_context.draggable_container_ref.set(Some(evt.data()));
            },
            div { style: "{placeholder_style}",
                if is_ready_to_hide() {
                    if let Some(placeholder_render) = context.placeholder_render {
                    {placeholder_render((
                        draggable_context.rect().height(),
                        draggable_context.rect().width()
                    ))}
                    }
                }
            }
            div { class: "{class}", style: "{draggable_style}",
                onmounted: move |evt: MountedEvent| {
                    draggable_context.draggable_ref.set(Some(evt.data()));
                },
                {context.render.call(item.clone())}
                {children}
            }
        }
    }
}

/// A handle for initiating the drag-and-drop operation.
///
/// This component captures the `onmousedown` event. Use this if you don't
/// want the entire [`Draggable`] item to be a grab target (e.g., a "grip" icon).
#[component]
pub fn DraggableHandler<T: DndItem>(
    /// CSS classes for the handler element.
    #[props(default)]
    class: ReadSignal<String>,
    /// The item data linked to this handler.
    item: T,
    /// Usually an icon or a specific "grab" area.
    children: Element,
) -> Element {
    let mut context = use_dnd_context::<T>();
    let draggable_context = try_use_context::<DraggableContext>();

    rsx! {
        div { class: "{class}",
            onmousedown: move |evt: MouseEvent| {
                let click_pos = evt.client_coordinates();

                context.start_pos.set(click_pos);
                // is not overlay
                if let Some(draggable_ctx) = draggable_context
                    && let Some(mounted) = (draggable_ctx.draggable_container_ref)() {
                    // set active after calculations
                    context.active.set(Some(item.clone()));
                    spawn(async move {
                        // Замеряем ЗДЕСЬ И СЕЙЧАС, чтобы скролл был учтен актуально
                        if let Ok(rect) = mounted.get_client_rect().await {

                            context.active_mounted.set(Some(mounted));
                            context.active_rect.set(Some(rect));

                            let off_x = rect.min_x() - click_pos.x;
                            let off_y = rect.min_y() - click_pos.y - 1.0;

                            context.grab_offset.set(OffsetOptions::new(off_x, off_y));
                        }
                    });
                }
            },
            {children}
        }
    }
}

/// A visual "ghost" element that follows the mouse cursor during dragging.
///
/// Uses [`dioxus-floating`] to handle precise positioning relative to the
/// initial grab point, ensuring the element doesn't "jump" when picked up.
#[component]
pub fn DraggableOverlay<T: DndItem>(
    /// Optional CSS classes for the overlay.
    #[props(default)]
    class: ReadSignal<String>,
    /// Data of the currently dragged item to be rendered.
    item: T,
) -> Element {
    let mut context = use_dnd_context::<T>();
    let mut overlay_ref = use_signal(|| Option::<Rc<MountedData>>::None);

    let trigger_point = use_memo(move || Some((context.mouse_pos)()));
    let placement = use_placement_on_point(
        overlay_ref,
        trigger_point,
        FloatingOptions {
            offset: context.grab_offset.read().clone(),
            ..Default::default()
        },
    );

    let draggable_style = use_memo(move || {
        let rect = context.active_rect.read().unwrap_or(PixelsRect::zero());

        placement.with(|pos| {
            if pos.is_ready {
                context.is_floating_ready.set(true);
                format!(
                    r#"
                    position: fixed;
                    inset: 0px auto auto 0px;
                    margin: 0px;
                    transform: translate3d({}px, {}px, 0px);
                    height: {}px;
                    width: {}px;
                    pointer-events: none;
                    z-index: 9999;
                    left: 0;
                    top: 0;
                    visibility: visible;
                    "#,
                    pos.x,
                    pos.y,
                    rect.height(),
                    rect.width()
                )
            } else {
                format!(
                    r#"
                    position: absolute;
                    transform: none;
                    height: {}px;
                    width: {}px;
                    pointer-events: auto;
                    z-index: auto;
                    left: 0;
                    top: 0;
                    visibility: hidden;
                    "#,
                    rect.height(),
                    rect.width()
                )
            }
        })
    });

    rsx! {
        div { class, style: "{draggable_style}",
            onmounted: move |evt: MountedEvent| {
                overlay_ref.set(Some(evt.data()));
            },
            {context.render.call(item)}
        }
    }
}
