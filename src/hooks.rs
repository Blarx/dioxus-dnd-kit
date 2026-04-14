use dioxus::{
    document::{Eval, eval},
    logger::tracing,
    prelude::*,
};

use crate::{droppable::DroppableContext, prelude::*};

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
                let r = await dioxus.recv(); 
                console.error("use_body_on_mouse_up r res", r);
            } catch (e) {
                // Канал закрыт со стороны Rust
                console.error("use_body_on_mouse_up chan closed", e);
            } finally {
                console.log("use_body_on_mouse_up destroying via AbortController...");
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
            loop {
                match eval.recv::<String>().await {
                    Ok(value) => {
                        if value.as_str() == "resize" {
                            callback.call(());
                        }
                    }
                    Err(_) => break,
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
