//! Multi-level column headers.
//!
//! A column says which groups it belongs to, outermost first
//! ([`ColumnSpec::group`](crate::ColumnSpec::group)). The header above the
//! columns is derived from those paths rather than described separately, so it
//! cannot fall out of step with the columns: reordering, hiding or pinning a
//! column rearranges the header with it.

use crate::ColumnSpec;

/// One cell of a group header row: a label over a run of columns.
///
/// A run of columns that share a group at this level, and the same groups above
/// it, becomes one cell. A column whose path is shorter than the header is deep
/// gets a cell of its own with no label, which is what fills the space above it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupSpan {
    /// The group's label, or `None` above a column that has no group here.
    pub label: Option<String>,
    /// Position of the first column it covers, among the visible columns.
    pub start: usize,
    /// How many columns it covers. Never zero.
    pub span: usize,
}

impl GroupSpan {
    /// The columns this cell covers.
    #[must_use]
    pub const fn columns(&self) -> std::ops::Range<usize> {
        self.start..self.start + self.span
    }

    /// Whether a column falls under this cell.
    #[must_use]
    pub const fn covers(&self, column: usize) -> bool {
        column >= self.start && column < self.start + self.span
    }
}

/// How deep the group header is: the longest group path among the columns.
///
/// Zero when no column is grouped, in which case the header is the one row of
/// column headers and nothing else.
#[must_use]
pub fn group_levels<T>(columns: &[&ColumnSpec<T>]) -> usize {
    columns
        .iter()
        .map(|column| column.group_path.len())
        .max()
        .unwrap_or(0)
}

/// The group header rows above the columns, outermost first.
///
/// Each row covers every column exactly once, so the rows line up with the
/// columns below them and `aria-colindex` stays true.
#[must_use]
pub fn group_header_rows<T>(columns: &[&ColumnSpec<T>]) -> Vec<Vec<GroupSpan>> {
    (0..group_levels(columns))
        .map(|level| spans_at(columns, level))
        .collect()
}

/// The spans of one group header row.
fn spans_at<T>(columns: &[&ColumnSpec<T>], level: usize) -> Vec<GroupSpan> {
    let mut spans: Vec<GroupSpan> = Vec::new();

    for (index, column) in columns.iter().enumerate() {
        let label = column.group_path.get(level);
        // The path down to and including this level decides what may merge: two
        // groups of the same name under different parents are two groups.
        let prefix = column.group_path.get(..=level);

        let merges = match (label, spans.last(), prefix) {
            // Only a labelled group merges, and only with the run just before
            // it, and only when that run came from the same path.
            (Some(label), Some(last), Some(prefix)) => {
                last.label.as_deref() == Some(label.as_str())
                    && columns
                        .get(last.start)
                        .and_then(|first| first.group_path.get(..=level))
                        == Some(prefix)
            }
            _ => false,
        };

        if merges {
            if let Some(last) = spans.last_mut() {
                last.span += 1;
            }
        } else {
            spans.push(GroupSpan {
                label: label.cloned(),
                start: index,
                span: 1,
            });
        }
    }

    spans
}
