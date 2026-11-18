# Changelog

Notable changes to `dioxus-dnd-kit` are documented here, newest first.

## [0.2.0] — 2026-11-18

### Added

- `DragLayout::Relative` and the optional `layout` prop on `Droppable` for
  sortable items with natural, dynamic heights.
- A standalone `relative_list` example with expandable cards, width changes,
  nested scrolling, an optional custom placeholder, resizing during drag, and
  a switch between relative and absolute layouts.
- Regression tests for repeated dragging, layout changes, stable item identity,
  asynchronous measurements, and sorting items of different heights.

### Changed

- Relative items stay mounted in normal flow while the fixed overlay is dragged.
  Custom placeholders cover the reserved space without adding another row height.
- Item identity and DOM registration use the stable keys from `key_gen` internally,
  so editing item data does not lose its registration.
- Geometry refreshes on item resize, viewport resize, and scrolling during drag.
- Layout measurements wait for an animation frame; obsolete measurement tasks are
  cancelled after a layout change.

### Fixed

- Repeatedly dragging the same relative item after dropping it: temporary
  `pointer-events: none` is now removed correctly.
- Leftover positioning, visibility, and opacity styles when switching layouts.
- Stale asynchronous rectangle measurements being applied to a different DOM order.
- Items of different heights swapping back after geometry refresh while the pointer
  continues in the same direction.
- Pickup measurements activating a drag after the mouse button was already released.
- Overlay movement being shifted or flipped at container boundaries instead of
  preserving the grab offset.

### Compatibility

- `DragLayout::Absolute` remains the default. Existing lists need no new props.
- The public `DndItem` bounds and existing `DndContext` and `DroppableContext`
  fields are unchanged.
- Sorting remains vertical and within a single list. Cross-list transfers are
  planned separately and are not part of this release.

## [0.1.3] — 2026-05-16

### Added

- Refreshing active collision geometry through `DndContext::recalculate_rects`.
- The `resizable_list` example for expanding and collapsing items and explicitly
  invalidating cached geometry.

## [0.1.2]

### Changed

- Updated the `dioxus-floating` dependency from `0.2` to `0.3`.

## [0.1.1] — 2026-04-14

### Fixed

- Idle placeholders now follow the list width instead of retaining a measured
  pixel width, allowing the layout to respond to container and viewport changes.
- Custom placeholder content is rendered only while the item is lifted.

## [0.1.0] — 2026-01-30

### Added

- Initial single-list sorting for Dioxus 0.7 using absolute item positioning.
- `DraggableView`, `Droppable`, `Draggable`, `DraggableHandler`, and a floating
  drag overlay.
- Custom item and placeholder rendering, stable DOM keys, and the `simple_list`
  example.

[0.2.0]: https://github.com/Blarx/dioxus-dnd-kit/compare/v0.1.3...v0.2.0
[0.1.3]: https://github.com/Blarx/dioxus-dnd-kit/compare/8fec246...v0.1.3
[0.1.2]: https://github.com/Blarx/dioxus-dnd-kit/compare/v0.1.1...8fec246
[0.1.1]: https://github.com/Blarx/dioxus-dnd-kit/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/Blarx/dioxus-dnd-kit/tree/v0.1.0
