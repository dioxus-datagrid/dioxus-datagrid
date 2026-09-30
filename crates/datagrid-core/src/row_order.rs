//! Moving a row to another place in the rows it belongs to.
//!
//! The grid never owns the rows, so it never moves one: it says which row went
//! where, and the application does this to its own `Vec`.

/// Moves the row at `from` so that it ends up at `to`, and says whether
/// anything moved.
///
/// `to` is where the row sits **afterwards**, counted in the same list — not
/// where it would be inserted before the row was taken out. That is the way a
/// user reads a move ("third from the top"), and it is the reading that makes
/// moving down by one mean what it says.
///
/// An index past the end lands at the end; an index equal to `from`, or a list
/// too short to hold either, changes nothing.
///
/// ```
/// use datagrid_core::move_row;
///
/// let mut rows = vec!["a", "b", "c", "d"];
/// assert!(move_row(&mut rows, 0, 2));
/// assert_eq!(rows, ["b", "c", "a", "d"]);
///
/// // And back again: a move and its reverse leave the list as it was.
/// assert!(move_row(&mut rows, 2, 0));
/// assert_eq!(rows, ["a", "b", "c", "d"]);
/// ```
pub fn move_row<T>(rows: &mut Vec<T>, from: usize, to: usize) -> bool {
    if from >= rows.len() {
        return false;
    }
    let to = to.min(rows.len() - 1);
    if from == to {
        return false;
    }
    let row = rows.remove(from);
    rows.insert(to, row);
    true
}
