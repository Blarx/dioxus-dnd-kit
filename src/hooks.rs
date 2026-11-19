use dioxus::{
    document::{Eval, eval},
    html::geometry::{ClientPoint, PixelsRect, PixelsSize, PixelsVector2D},
    logger::tracing,
    prelude::*,
};

use crate::{droppable::DroppableContext, prelude::*};

/// Let pending renderer mutations reach the DOM before measuring layout.
pub(crate) async fn wait_for_layout() {
    let _ = eval("await new Promise(requestAnimationFrame);").await;
}

/// A connected drop target must be hit only through its visible area. A raw
/// bounding rect also includes portions clipped by horizontal/nested scrollers.
pub(crate) async fn visible_container_rect(id: String) -> Option<PixelsRect> {
    let mut measure = eval(concat!(
        include_str!("scroll_geometry.js"),
        r#"
        const element = document.getElementById(await dioxus.recv());
        if (!element) { dioxus.send(null); return; }
        let bounds = clipBounds(element.getBoundingClientRect(), {
            left: 0, top: 0, right: window.innerWidth, bottom: window.innerHeight
        }, true, true);
        for (let parent = element.parentElement; parent; parent = parent.parentElement) {
            const [clipX, clipY] = clippingAxes(getComputedStyle(parent));
            if (clipX || clipY) bounds = clipBounds(bounds, clientBounds(parent), clipX, clipY);
        }
        const { left, top, right, bottom } = bounds;
        dioxus.send(right > left && bottom > top ? [left, top, right - left, bottom - top] : null);
    "#
    ));
    measure.send(id).ok()?;
    let [x, y, width, height] = measure.recv::<Option<[f64; 4]>>().await.ok()??;
    Some(PixelsRect::new(
        PixelsVector2D::new(x, y).to_point(),
        PixelsSize::new(width, height),
    ))
}

/// Global drag input, including the final release position outside the view.
/// Mouseup stays enabled during asynchronous pickup; moves and cancellation
/// events are forwarded only while a drag is active.
pub(crate) fn use_window_drag_events(
    view_id: usize,
    active: Memo<bool>,
    auto_scroll: Memo<bool>,
    mouse: Signal<ClientPoint>,
    callback: Callback<[f64; 3]>,
) {
    let mut listener = use_signal(|| None::<Eval>);
    use_effect(move || {
        let mut bridge = eval(concat!(
            include_str!("scroll_geometry.js"),
            include_str!("auto_scroll.js"),
            r#"
            const viewId = await dioxus.recv();
            const scroller = createAutoScroll('[data-dnd-view="' + viewId + '"]');
            const controller = new AbortController();
            const options = { capture: true, signal: controller.signal };
            let active = false;
            let scrolling = false;
            const stopScrolling = () => {
                scrolling = false;
                scroller.stop();
            };
            window.addEventListener("mousemove", e => {
                if (scrolling) scroller.move(e.clientX, e.clientY);
                if (active) dioxus.send([0, e.clientX, e.clientY]);
            }, options);
            window.addEventListener("mouseup", e => {
                if (e.button === 0) {
                    stopScrolling();
                    dioxus.send([1, e.clientX, e.clientY]);
                }
            }, options);
            window.addEventListener("keydown", e => {
                if (active && e.key === "Escape") {
                    stopScrolling();
                    e.preventDefault();
                    dioxus.send([2, 0, 0]);
                }
            }, options);
            window.addEventListener("blur", () => {
                stopScrolling();
                if (active) dioxus.send([2, 0, 0]);
            }, options);
            try {
                while (true) {
                    const command = await dioxus.recv();
                    if (command === "abort") break;
                    active = command !== "idle";
                    if (Array.isArray(command)) {
                        scrolling = true;
                        scroller.move(...command);
                    } else {
                        stopScrolling();
                    }
                }
            } finally {
                stopScrolling();
                controller.abort();
            }
        "#
        ));
        let _ = bridge.send(view_id);
        spawn(async move {
            while let Ok(event) = bridge.recv::<[f64; 3]>().await {
                callback.call(event);
            }
        });
        listener.set(Some(bridge));
    });
    use_effect(move || {
        let active = active();
        let auto_scroll = auto_scroll();
        if let Some(bridge) = listener() {
            if active && auto_scroll {
                let mouse = mouse.peek();
                let _ = bridge.send([mouse.x, mouse.y]);
            } else {
                let _ = bridge.send(if active { "active" } else { "idle" });
            }
        }
    });
    use_drop(move || {
        if let Some(bridge) = *listener.peek() {
            let _ = bridge.send("abort");
        }
    });
}

/// Scroll does not bubble, so listen in the capture phase, including nested scrollers.
pub(crate) fn use_window_on_scroll(callback: Callback<()>) {
    let mut hook_eval = use_signal(|| None::<Eval>);
    use_effect(move || {
        let mut listener = eval(
            r#"
            const controller = new AbortController();
            window.addEventListener("scroll", () => dioxus.send(true), {
                capture: true, passive: true, signal: controller.signal
            });
            await dioxus.recv();
            controller.abort();
            "#,
        );
        spawn(async move {
            while listener.recv::<bool>().await.is_ok() {
                callback.call(());
            }
        });
        if let Some(previous) = hook_eval.replace(Some(listener)) {
            let _ = previous.send("abort");
        }
    });
    use_drop(move || {
        if let Some(listener) = *hook_eval.peek() {
            let _ = listener.send("abort");
        }
    });
}

/// Returns the global drag-and-drop context.
///
/// Provides access to the current active item, mouse position, and global drag state.
/// Must be called within a [`DraggableView`].
pub fn use_dnd_context<T: DndItem>() -> DndContext<T> {
    use_context::<DndContext<T>>()
}

/// Returns the nearest droppable zone context.
///
/// Used by [`Draggable`] items to register themselves and report their geometry.
pub fn use_droppable_context<T: DndItem>() -> DroppableContext<T> {
    use_context::<DroppableContext<T>>()
}

/// Attaches a global `mouseup` listener to the browser `window`.
///
/// This is crucial for D-n-D to detect when a user releases the item
/// outside the application window or over non-interactive elements.
///
/// ### Implementation Details:
/// Uses a JavaScript `AbortController` via `eval` to ensure the event listener
/// is properly removed when the component is unmounted or the effect is re-run.
pub fn use_body_on_mouse_up(callback: Callback<()>) {
    let mut hook_eval = use_signal(|| Option::<Eval>::None);

    use_effect(move || {
        let mut eval = eval(
            r#"
            const controller = new AbortController();
            const handler = (event) => {
                dioxus.send("mouseup");
            };
            
            window.addEventListener("mouseup", handler, {
                capture: true, 
                signal: controller.signal 
            });
            
            try {
                await dioxus.recv();
            } catch (_) {
            } finally {
                controller.abort();
            }
            "#,
        );
        if let Some(old_eval) = *hook_eval.peek() {
            let _ = old_eval.send("abort");
        }
        hook_eval.set(Some(eval));
        spawn(async move {
            loop {
                match eval.recv::<String>().await {
                    Ok(value) => {
                        if value.as_str() == "mouseup" {
                            tracing::debug!("use_body_on_mouse_up: upped");
                            callback.call(());
                        }
                    }
                    Err(err) => {
                        tracing::error!("use_body_on_mouse_up: {err}");
                        break;
                    }
                }
            }
        });
    });
    use_drop(move || {
        if let Some(old_eval) = hook_eval() {
            let _ = old_eval.send("abort");
        }
    });
}

/// Attaches a global `resize` listener to the browser `window`.
///
/// Useful to invalidate cached geometry after viewport/container responsive changes.
pub fn use_window_on_resize(callback: Callback<()>) {
    let mut hook_eval = use_signal(|| Option::<Eval>::None);

    use_effect(move || {
        let mut eval = eval(
            r#"
            const controller = new AbortController();
            const handler = () => {
                dioxus.send("resize");
            };
            
            window.addEventListener("resize", handler, {
                signal: controller.signal
            });
            
            try {
                await dioxus.recv();
            } catch (_) {
            } finally {
                controller.abort();
            }
            "#,
        );
        if let Some(old_eval) = *hook_eval.peek() {
            let _ = old_eval.send("abort");
        }
        hook_eval.set(Some(eval));
        spawn(async move {
            while let Ok(value) = eval.recv::<String>().await {
                if value.as_str() == "resize" {
                    callback.call(());
                }
            }
        });
    });
    use_drop(move || {
        if let Some(old_eval) = hook_eval() {
            let _ = old_eval.send("abort");
        }
    });
}
