use std::rc::Rc;

use dioxus::{
    html::geometry::{ClientPoint, PixelsRect},
    prelude::*,
};
use dioxus_floating::{OffsetOptions, ScrollableView};

use crate::{
    DndItem,
    hooks::{use_body_on_mouse_up, use_window_on_resize, use_window_on_scroll},
    prelude::DraggableOverlay,
};

/// Invalidates in-flight pickup measurements when the button is released or a new drag starts.
/// Kept separate from the public context so existing context consumers remain compatible.
#[derive(Clone, Copy)]
pub(crate) struct DragSession {
    pub generation: Signal<u64>,
}

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
/// `DraggableView` initializes the [`DndContext`], handles mouse events (move/up),
/// and manages the visual lifecycle of the dragged item.
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
) -> Element {
    let mut context =
        use_context_provider(move || DndContext::new(key_gen, render, placeholder_render));
    let mut session = use_context_provider(|| DragSession {
        generation: Signal::new(0),
    });

    let mouseup = use_callback(move |_| {
        session.generation.with_mut(|value| *value += 1);
        context.active.set(None);
        context.is_floating_ready.set(false);
        context.active_mounted.set(None);
        context.active_rect.set(None);
        context.start_pos.set(ClientPoint::zero());
    });
    use_body_on_mouse_up(mouseup);
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
                context.mouse_pos.set(e.client_coordinates());
            },
            {children}
            if let Some(active_item) = context.active.read().as_ref() {
                DraggableOverlay::<T> { item: active_item.clone() }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DragLayout,
        draggable::Draggable,
        droppable::{DragLayoutContext, DroppableContext},
    };
    use dioxus::core::{AttributeValue, ElementId, Mutation};
    use std::cell::RefCell;

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

    fn has_style(
        edits: &[Mutation],
        node: ElementId,
        property: &str,
        expected: Option<&str>,
    ) -> bool {
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
}
