mod context;
mod draggable;
mod droppable;
mod hooks;

/// How list items participate in document layout. The drag overlay is always fixed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DragLayout {
    /// Preserve the original absolute content and measured placeholder layout.
    #[default]
    Absolute,
    /// Keep content in normal flow, including while its overlay is being dragged.
    Relative,
}

pub trait DndItem: PartialEq + Clone + 'static {}

impl<T: PartialEq + Clone + 'static> DndItem for T {}

pub mod prelude {
    pub use crate::context::DndContext;
    pub use crate::context::DraggableView;
    pub use crate::draggable::{Draggable, DraggableHandler, DraggableOverlay};
    pub use crate::droppable::Droppable;
    pub use crate::hooks::{
        use_body_on_mouse_up, use_dnd_context, use_droppable_context, use_window_on_resize,
    };
    pub use crate::{DndItem, DragLayout};
}
