use dioxus::html::geometry::{PixelsRect, PixelsVector2D};

use crate::{DragAxis, context::DragPosition};

/// Geometry eligible for this frame, without registration or item-data copies.
#[derive(Clone, Copy)]
pub(super) struct DropZone {
    pub rect: Option<PixelsRect>,
    pub ready: bool,
}

pub(super) fn target_zone(
    zones: &[DropZone],
    position: DragPosition,
    current: Option<usize>,
) -> Option<usize> {
    let point = PixelsVector2D::new(position.mouse.x, position.mouse.y).to_point();
    let eligible = |index: usize, zone: &DropZone| zone.ready || Some(index) == current;
    let area = |rect: PixelsRect| rect.width() * rect.height();
    // Prefer the pointer, including the current container while its item DOM is
    // being remeasured after a live transfer. Never admit other unready lists.
    let pointed = zones
        .iter()
        .enumerate()
        .filter(|(index, zone)| {
            eligible(*index, zone) && zone.rect.is_some_and(|rect| rect.contains(point))
        })
        .min_by(|(_, a), (_, b)| area(a.rect.unwrap()).total_cmp(&area(b.rect.unwrap())))
        .map(|(index, _)| index);
    if pointed.is_some() {
        return pointed;
    }

    let dragged = position.rect?;
    zones
        .iter()
        .enumerate()
        .filter(|(index, zone)| eligible(*index, zone))
        .filter_map(|(index, zone)| {
            let rect = zone.rect?;
            let overlap = area(dragged.intersection(&rect)?);
            let smaller_area = area(dragged).min(area(rect));
            // Use visible, clipped container bounds. An edge touch must not
            // select a neighbor; a quarter of the smaller surface must overlap.
            (smaller_area > 0.0 && overlap >= smaller_area * 0.25).then_some((index, overlap))
        })
        .max_by(|(a, overlap_a), (b, overlap_b)| {
            overlap_a
                .total_cmp(overlap_b)
                .then_with(|| (Some(*a) == current).cmp(&(Some(*b) == current)))
                .then_with(|| b.cmp(a))
        })
        .map(|(index, _)| index)
}

pub(super) fn insertion_index(
    rects: &[Option<PixelsRect>],
    coordinate: f64,
    axis: DragAxis,
) -> Option<usize> {
    if rects.iter().any(Option::is_none) {
        return None;
    }
    Some(
        rects
            .iter()
            .position(|rect| {
                let rect = rect.unwrap();
                coordinate < axis.midpoint(rect)
            })
            .unwrap_or(rects.len()),
    )
}
