mod context;
mod draggable;
mod droppable;
mod hooks;

pub trait DndItem: PartialEq + Clone + 'static {}

impl<T: PartialEq + Clone + 'static> DndItem for T {}

pub mod prelude {
    pub use crate::DndItem;
    pub use crate::context::DndContext;
    pub use crate::context::DraggableView;
    pub use crate::draggable::{Draggable, DraggableHandler, DraggableOverlay};
    pub use crate::droppable::Droppable;
    pub use crate::hooks::{use_body_on_mouse_up, use_dnd_context, use_droppable_context};
}
