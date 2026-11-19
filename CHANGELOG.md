# Changelog

Notable changes to `dioxus-dnd-kit` are documented here, newest first.

## [Unreleased] — 0.3.0

### Added

- Horizontal sorting via `DragAxis` and `Droppable::axis` (vertical by default).
  Connected insertion follows the destination's axis.
- Nested column/card sorting in the board example, with separate handles,
  validation requests and Undo. `DraggableView::overlay_render` draws a passive
  column snapshot without registering duplicate lists or HTML IDs.
- Typed drag-session isolation and a shared lease across nested views, including
  API validation. `DndState::is_busy()` exposes tree-wide activity. Nested views
  cannot accidentally join an ancestor view's connected group.
- Transactional `DraggableView::on_drop` for same-list sorting and connected
  transfers. `DropEvent` can accept/reject after an await and undo an accepted move.
- `DropChange`/`DropLocation` identify the item, original/final list and index,
  and neighboring keys. View-level `can_drop`, `on_drag_start`, `on_drag_cancel`,
  and `use_dnd_state` expose local eligibility and operation state.
- Pending validation blocks new pickups in the view. Rollback preserves unrelated
  edits; conflict checks and unmount invalidation prevent stale decisions from
  overwriting authoritative data. The board example simulates delayed API responses.
- `ConnectedLists<T>` connects `Droppable` containers with explicit, stable `id`
  values and separate item signals. Independent lists keep their original behavior.
- Live cross-list transfers, including insertion into empty lists and positions
  selected from the destination items' geometry.
- Cancellation restores the dragged item to its original list and index on Escape,
  window blur, or release outside the connected containers.
- Optional `ConnectedLists::on_drag_start` callback.
- A Trello-style `connected_lists` example with three columns, mixed card heights,
  persistent item data, an empty destination, both layout modes, and an independent list.
- Crate-level rustdoc guide, compilable quick starts, layout and identity guidance,
  and API links. The README quick start now includes a drag handle.
- Public event and state rustdoc covers candidate versus released positions,
  decision results, shared event handles, Undo lifetime, and nested validation state.

### Changed

- Base DnD transactions now live in `DndTransactionContext`, with one shared list
  registry and preview mutation path for local sorting and connected transfers.
  Validation state reads the transaction directly instead of mirroring it.
- Clearing the public drag context's `active` signal completes independent
  transactions and releases their lock; changed drops still await validation.
- Global drag input tracks mouse movement and the final release position outside
  the view, allowing the overlay and cancellation logic to follow the pointer.
- Connected containers refresh their geometry even before they contain the active
  item. A newly mounted active absolute item immediately reserves its pickup height.
- Internal transfer bookkeeping and regression tests cover duplicate prevention,
  rollback without overwriting unrelated edits, empty lists, and group isolation.
- Connected drop targets respect viewport bounds and clipping by scrollable or
  overflow-hidden ancestors, so hidden portions cannot receive a drop.
- Connected-list coordination uses published geometry snapshots and scoped item
  callbacks, avoiding child-owned signal access and Dioxus warning spam during drags.
- Connected hover and release share pointer-first, visible-card-overlap hit testing.
  An accepted placeholder commits on release even with the handle outside the list;
  moving away from all targets hides its highlight and cancels on release.

### Compatibility

- Absolute layout remains the default, and lists connect only through an explicit
  `ConnectedLists` wrapper plus non-empty container IDs.
- Existing component calls, public context fields, and `DndItem` bounds remain valid.
- Item content remounts across container boundaries; store persistent UI state in
  item data or in a parent keyed by item ID.

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

[Unreleased]: https://github.com/Blarx/dioxus-dnd-kit/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/Blarx/dioxus-dnd-kit/compare/v0.1.3...v0.2.0
[0.1.3]: https://github.com/Blarx/dioxus-dnd-kit/compare/8fec246...v0.1.3
[0.1.2]: https://github.com/Blarx/dioxus-dnd-kit/compare/v0.1.1...8fec246
[0.1.1]: https://github.com/Blarx/dioxus-dnd-kit/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/Blarx/dioxus-dnd-kit/tree/v0.1.0
