use std::rc::Rc;

use dioxus::{core::Task, html::geometry::PixelsRect, prelude::*};
use dioxus_floating::{FloatingOptions, OffsetOptions, use_placement_on_point};

use crate::{
    DndItem, DragLayout,
    context::DragSession,
    droppable::DragLayoutContext,
    hooks::{use_dnd_context, use_droppable_context, wait_for_layout},
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
/// In absolute mode a measured placeholder reserves space. In relative mode the
/// original stays in flow and is hidden only after the fixed overlay is ready.
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
    let mut context = use_dnd_context::<T>();
    let mut droppable_context = use_droppable_context::<T>();
    let layout_context = try_use_context::<DragLayoutContext>();
    let layout = use_memo(move || layout_context.map(|ctx| (ctx.0)()).unwrap_or_default());
    let mut draggable_context = use_context_provider(DraggableContext::default);
    let mut measurement_task = use_signal(|| None::<Task>);
    let trigger_key = context.key_gen.call(item.clone());
    let is_active = use_memo(move || {
        context
            .active
            .read()
            .clone()
            .map(|active_item| context.key_gen.call(active_item) == trigger_key)
            .unwrap_or(false)
    });
    let is_ready_to_hide = use_memo(move || is_active() && (context.is_floating_ready)());

    let register_item = item.clone();
    use_effect(move || {
        if let Some(draggable) = (draggable_context.draggable_ref)() {
            droppable_context.register_element_by_key(
                register_item.clone(),
                draggable,
                context.key_gen,
            );
        }
    });

    use_effect(move || {
        // subscribe to recalculation logic.
        let _ = (context.recalculate_rects)();

        if let Some(task) = measurement_task.take() {
            task.cancel();
        }

        if let Some(draggable) = (draggable_context.draggable_ref)() {
            measurement_task.set(Some(spawn(async move {
                wait_for_layout().await;
                if let Ok(rect) = draggable.get_client_rect().await {
                    // Keep the previous height until its replacement is available.
                    draggable_context.draggable_rect.set(Some(rect));
                    draggable_context.draggable_container_rect.set(Some(rect));
                }
            })));
        } else {
            draggable_context.draggable_rect.set(None);
            draggable_context.draggable_container_rect.set(None);
        }
    });

    // Use individual CSS attributes for stateful styles. Dioxus preserves omitted
    // properties when replacing a style string; namespaced attributes can remove
    // them on drop and when switching layouts (notably pointer-events: none).
    rsx! {
        div {
            class: "relative",
            position: "relative",
            display: (layout() == DragLayout::Relative).then_some("flow-root"),
            onmounted: move |evt: MountedEvent| {
                draggable_context.draggable_container_ref.set(Some(evt.data()));
            },
            div {
                position: (layout() == DragLayout::Relative).then_some("absolute"),
                top: (layout() == DragLayout::Relative).then_some("0"),
                right: (layout() == DragLayout::Relative).then_some("0"),
                bottom: (layout() == DragLayout::Relative).then_some("0"),
                left: (layout() == DragLayout::Relative).then_some("0"),
                visibility: if is_ready_to_hide() { "visible" } else { "hidden" },
                pointer_events: "none",
                width: if layout() == DragLayout::Relative { "auto" } else { "100%" },
                height: if layout() == DragLayout::Absolute && is_ready_to_hide() {
                    format!("{}px", draggable_context.rect().height())
                } else {
                    "auto".to_string()
                },
                min_height: if layout() == DragLayout::Absolute && !is_ready_to_hide() {
                    format!("{}px", draggable_context.rect().height())
                } else {
                    "0px".to_string()
                },
                if is_ready_to_hide() {
                    if let Some(placeholder_render) = context.placeholder_render {
                        {placeholder_render((
                            draggable_context.rect().height(),
                            draggable_context.rect().width()
                        ))}
                    }
                }
            }
            div {
                class: "{class}",
                position: if layout() == DragLayout::Relative { "relative" } else { "absolute" },
                top: (layout() == DragLayout::Absolute).then_some("0"),
                left: (layout() == DragLayout::Absolute).then_some("0"),
                right: (layout() == DragLayout::Absolute).then_some("0"),
                opacity: (layout() == DragLayout::Absolute).then_some(if is_ready_to_hide() { "0" } else { "1" }),
                visibility: (layout() == DragLayout::Relative).then_some(if is_ready_to_hide() { "hidden" } else { "visible" }),
                pointer_events: (layout() == DragLayout::Relative && is_ready_to_hide()).then_some("none"),
                onmounted: move |evt: MountedEvent| {
                    draggable_context.draggable_ref.set(Some(evt.data()));
                },
                onresize: move |_| {
                    context.recalculate_rects.with_mut(|value| *value += 1);
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
    let mut session = use_context::<DragSession>();

    rsx! {
        div { class: "{class}",
            onmousedown: move |evt: MouseEvent| {
                if evt.trigger_button() != Some(dioxus::html::input_data::MouseButton::Primary)
                    || context.active.peek().is_some()
                {
                    return;
                }
                let click_pos = evt.client_coordinates();

                if let Some(mut draggable_ctx) = draggable_context
                    && let Some(mounted) = (draggable_ctx.draggable_container_ref)() {
                    evt.prevent_default();
                    session.generation.with_mut(|value| *value += 1);
                    let generation = *session.generation.peek();
                    context.start_pos.set(click_pos);
                    context.mouse_pos.set(click_pos);
                    let item = item.clone();
                    spawn(async move {
                        if let Ok(rect) = mounted.get_client_rect().await
                            && *session.generation.peek() == generation
                        {
                            draggable_ctx.draggable_rect.set(Some(rect));
                            context.is_floating_ready.set(false);
                            context.active_mounted.set(Some(mounted));
                            context.active_rect.set(Some(rect));

                            let off_x = rect.min_x() - click_pos.x;
                            let off_y = rect.min_y() - click_pos.y - 1.0;

                            context.grab_offset.set(OffsetOptions::new(off_x, off_y));
                            // Mount the overlay only after its size and grab offset are known.
                            context.active.set(Some(item));
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
            // A dragged item must follow the grab point even outside the list bounds.
            middleware: Vec::new(),
            offset: context.grab_offset.read().clone(),
            ..Default::default()
        },
    );

    let is_ready = use_memo(move || placement().is_ready);
    use_effect(move || {
        if context.active.peek().is_some() {
            context.is_floating_ready.set(is_ready());
        }
    });

    let draggable_style = use_memo(move || {
        let rect = context.active_rect.read().unwrap_or(PixelsRect::zero());

        placement.with(|pos| {
            if pos.is_ready {
                format!(
                    r#"
                    position: fixed;
                    inset: 0px auto auto 0px;
                    margin: 0px;
                    box-sizing: border-box;
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
                    pointer-events: none;
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
