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

/// Which columns a column-virtualized body should render.
///
/// `widths` are the columns' widths in layout order, in CSS pixels, and
/// `scroll_left` is how far the container is scrolled from the start edge. The
/// result is the columns intersecting the viewport, widened by `overscan`
/// columns on each side. It always satisfies `start <= end <= widths.len()`,
/// whatever the inputs.
///
/// A width that is not finite, or is negative, counts as zero: a column that
/// has none takes no room, and guessing one would render the wrong columns.
///
/// This is the horizontal twin of [`visible_range`], except that columns, unlike
/// fixed-height rows, differ in width and the position of a column is a sum.
///
/// # Examples
///
/// ```
/// use datagrid_core::visible_columns;
///
/// // Ten columns of 100px, scrolled to 250px, a 300px viewport: columns 2..6
/// // (x = 200..600) intersect 250..550.
/// let widths = [100.0; 10];
/// assert_eq!(visible_columns(&widths, 250.0, 300.0, 0), 2..6);
/// assert_eq!(visible_columns(&widths, 250.0, 300.0, 1), 1..7);
///
/// // Widths may differ.
/// assert_eq!(visible_columns(&[50.0, 400.0, 50.0], 100.0, 100.0, 0), 1..2);
///
/// // Overscroll cannot push the range out of bounds.
/// assert_eq!(visible_columns(&widths, -40.0, 300.0, 0), 0..3);
/// ```
#[must_use]
pub fn visible_columns(
    widths: &[f64],
    scroll_left: f64,
    viewport_width: f64,
    overscan: usize,
) -> Range<usize> {
    if widths.is_empty() {
        return 0..0;
    }
    let scroll_left = clamp_coordinate(scroll_left);
    let viewport_width = clamp_coordinate(viewport_width);
    let right = scroll_left + viewport_width;

    // A column is in view if it ends after the left edge and starts before the
    // right one. Walk once, remembering where the first and last such column are.
    let mut x = 0.0;
    let mut first = None;
    let mut last = None;
    for (index, width) in widths
        .iter()
        .map(|width| clamp_coordinate(*width))
        .enumerate()
    {
        let end = x + width;
        if first.is_none() && end > scroll_left {
            first = Some(index);
        }
        if x < right {
            last = Some(index + 1);
        }
        x = end;
    }

    // Scrolled past the last column (or nothing in view): show the last ones
    // rather than nothing, as a browser clamps the scroll position.
    let first = first.unwrap_or(widths.len().saturating_sub(1));
    let last = last.unwrap_or(0).max(first);

    let start = first.saturating_sub(overscan);
    let end = last.saturating_add(overscan).min(widths.len());
    start..end.max(start)
}

/// A pixel quantity made safe to add up: non-finite and negative become zero.
fn clamp_coordinate(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

/// How far from the start edge column `index` begins: the widths before it.
///
/// An `index` past the end is the total width.
///
/// # Examples
///
/// ```
/// use datagrid_core::column_offset;
///
/// assert_eq!(column_offset(&[100.0, 50.0, 80.0], 2), 150.0);
/// assert_eq!(column_offset(&[100.0, 50.0, 80.0], 9), 230.0);
/// ```
#[must_use]
pub fn column_offset(widths: &[f64], index: usize) -> f64 {
    widths
        .iter()
        .take(index)
        .map(|width| clamp_coordinate(*width))
        .sum()
}

/// The horizontal scroll position that brings a column fully into view, or
/// `None` when it already is.
///
/// Scrolls as little as possible, like [`reveal_scroll_top`]. `inset_start` and
/// `inset_end` are the widths of the pinned blocks at either edge: they stay
/// where they are and cover that much of the viewport, so a scrolling column
/// has to clear them. A column wider than what is left shows its start.
///
/// # Examples
///
/// ```
/// use datagrid_core::reveal_scroll_left;
///
/// // A 100px column at x = 500 in a 300px viewport scrolled to 0: scroll so
/// // that it ends at the right edge.
/// assert_eq!(reveal_scroll_left(500.0, 100.0, 300.0, 0.0, 0.0, 0.0), Some(300.0));
/// // Already in view.
/// assert_eq!(reveal_scroll_left(500.0, 100.0, 300.0, 400.0, 0.0, 0.0), None);
/// // A 60px pinned block at the start covers the first 60px of the viewport.
/// assert_eq!(reveal_scroll_left(500.0, 100.0, 300.0, 460.0, 60.0, 0.0), Some(440.0));
/// ```
#[must_use]
pub fn reveal_scroll_left(
    start: f64,
    width: f64,
    viewport_width: f64,
    scroll_left: f64,
    inset_start: f64,
    inset_end: f64,
) -> Option<f64> {
    if !start.is_finite() || !width.is_finite() {
        return None;
    }
    let scroll_left = clamp_coordinate(scroll_left);
    let inset_start = clamp_coordinate(inset_start);
    let inset_end = clamp_coordinate(inset_end);
    let width = width.max(0.0);
    // What the scrolling columns can use of the viewport.
    let usable = (clamp_coordinate(viewport_width) - inset_start - inset_end).max(0.0);

    let left = start - inset_start;
    let right = start + width - inset_start;
    // `left` and `right` are measured from where the scrolling area begins.
    if left < scroll_left {
        Some(left.max(0.0))
    } else if right > scroll_left + usable {
        Some((right - usable).max(0.0).min(left.max(0.0)))
    } else {
        None
    }
}
