//! Virtualization arithmetic.

use std::ops::Range;

/// Which rows a virtualized body should render.
///
/// Returns the rows intersecting the viewport, widened by `overscan` rows on
/// each side so that scrolling does not expose unrendered space.
///
/// The returned range always satisfies `start <= end <= total_rows`, whatever
/// the inputs — scroll positions arriving from a real browser can be negative
/// (rubber-band overscroll on iOS) or, during teardown, not finite at all.
///
/// A `row_height` that is not finite and positive yields an empty range: there
/// is no meaningful answer, and guessing one would render the wrong rows.
///
/// # Examples
///
/// ```
/// use datagrid_core::visible_range;
///
/// // 20px rows, scrolled to 100px, 50px tall viewport, no overscan:
/// // rows 5..8 intersect the viewport.
/// assert_eq!(visible_range(100.0, 50.0, 20.0, 1000, 0), 5..8);
///
/// // With overscan the range grows on both sides.
/// assert_eq!(visible_range(100.0, 50.0, 20.0, 1000, 2), 3..10);
///
/// // Overscroll cannot push the range out of bounds.
/// assert_eq!(visible_range(-40.0, 50.0, 20.0, 1000, 0), 0..3);
/// ```
#[must_use]
pub fn visible_range(
    scroll_top: f64,
    viewport_height: f64,
    row_height: f64,
    total_rows: usize,
    overscan: usize,
) -> Range<usize> {
    if !row_height.is_finite() || row_height <= 0.0 || total_rows == 0 {
        return 0..0;
    }

    // Clamp the incoming geometry before doing any arithmetic with it, so that
    // NaN and negative overscroll cannot propagate into the result.
    let scroll_top = if scroll_top.is_finite() {
        scroll_top.max(0.0)
    } else {
        0.0
    };
    let viewport_height = if viewport_height.is_finite() {
        viewport_height.max(0.0)
    } else {
        0.0
    };

    let first_visible = to_index(scroll_top / row_height, total_rows);
    let last_visible = to_index(
        ((scroll_top + viewport_height) / row_height).ceil(),
        total_rows,
    );

    let start = first_visible.saturating_sub(overscan);
    let end = last_visible.saturating_add(overscan).min(total_rows);

    start..end.max(start)
}

/// Converts a row coordinate to an index, clamped into `0..=total_rows`.
///
/// `as` conversions from float to integer saturate rather than wrap, so this
/// cannot overflow; the explicit clamp keeps the value inside the row count.
fn to_index(value: f64, total_rows: usize) -> usize {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let index = value.floor() as usize;
    index.min(total_rows)
}

/// The total scrollable height of `total_rows` rows, in CSS pixels.
///
/// The spacer above and below the rendered window must add up to this, or the
/// scrollbar will not match the data.
#[must_use]
pub fn total_height(total_rows: usize, row_height: f64) -> f64 {
    if !row_height.is_finite() || row_height <= 0.0 {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)]
    let rows = total_rows as f64;
    rows * row_height
}

/// The offset of the first rendered row, in CSS pixels.
///
/// This is what the rendered window must be translated by so the rows land
/// where the scrollbar says they are.
#[must_use]
pub fn offset_of(index: usize, row_height: f64) -> f64 {
    total_height(index, row_height)
}

/// How many whole rows fit in the viewport; what `PageUp` and `PageDown` move by.
///
/// Never less than one, so paging always makes progress even in a viewport
/// shorter than a row.
#[must_use]
pub fn rows_per_viewport(viewport_height: f64, row_height: f64) -> usize {
    if !row_height.is_finite() || row_height <= 0.0 || !viewport_height.is_finite() {
        return 1;
    }
    to_index((viewport_height / row_height).floor(), usize::MAX).max(1)
}

/// The scroll position that brings row `index` fully into view, or `None` when
/// it already is.
///
/// Scrolls as little as possible: a row above the viewport is aligned to the
/// top, a row below it to the bottom. That is what keyboard navigation wants —
/// moving down one row past the edge should nudge the view by one row, not jump
/// the focused row to the top.
///
/// All values are in the body's coordinate space. With a sticky header, pass the
/// viewport height minus the header height; the header covers that part of the
/// viewport and rows beneath it are not visible.
///
/// # Examples
///
/// ```
/// use datagrid_core::reveal_scroll_top;
///
/// // 20px rows, 100px viewport scrolled to 200px: rows 10..15 are visible.
/// assert_eq!(reveal_scroll_top(12, 20.0, 100.0, 200.0), None);
/// // Row 15 is just below: scroll so it becomes the last visible row.
/// assert_eq!(reveal_scroll_top(15, 20.0, 100.0, 200.0), Some(220.0));
/// // Row 3 is above: scroll so it becomes the first visible row.
/// assert_eq!(reveal_scroll_top(3, 20.0, 100.0, 200.0), Some(60.0));
/// ```
#[must_use]
pub fn reveal_scroll_top(
    index: usize,
    row_height: f64,
    viewport_height: f64,
    scroll_top: f64,
) -> Option<f64> {
    if !row_height.is_finite() || row_height <= 0.0 {
        return None;
    }
    let scroll_top = if scroll_top.is_finite() {
        scroll_top.max(0.0)
    } else {
        0.0
    };
    let viewport_height = if viewport_height.is_finite() {
        viewport_height.max(0.0)
    } else {
        0.0
    };

    let top = offset_of(index, row_height);
    let bottom = top + row_height;

    if top < scroll_top {
        Some(top)
    } else if bottom > scroll_top + viewport_height {
        // In a viewport shorter than one row, showing the row's top beats
        // showing its bottom.
        Some((bottom - viewport_height).max(0.0).min(top))
    } else {
        None
    }
}
