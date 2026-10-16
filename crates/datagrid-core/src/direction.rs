//! The direction text runs in, and what it mirrors.

use crate::NavKey;

/// Which way a grid reads: the direction its first column is at, and its arrow
/// keys point to.
///
/// Left to right is what a grid does unless it is told otherwise. In right to
/// left text the first column sits at the right edge, `ArrowRight` goes back to
/// the column before, and a column is widened by dragging its edge to the
/// left. This type says so, because a grid on a page has no way to read it from
/// the page (`docs/DECISIONS.md` ADR-0044).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Direction {
    /// Left to right: the first column is at the left.
    #[default]
    Ltr,
    /// Right to left: the first column is at the right.
    Rtl,
}

impl Direction {
    /// The value of the HTML `dir` attribute: `"ltr"` or `"rtl"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ltr => "ltr",
            Self::Rtl => "rtl",
        }
    }

    /// Whether this is right to left.
    #[must_use]
    pub const fn is_rtl(self) -> bool {
        matches!(self, Self::Rtl)
    }

    /// `1.0` going to the right on screen from the start edge, `-1.0` in right
    /// to left text: what a horizontal distance from the start edge is
    /// multiplied by to become a screen direction, and what `scrollLeft` is in
    /// right to left text (a negative number, `docs/VERIFICATION.md` §23).
    #[must_use]
    pub const fn sign(self) -> f64 {
        match self {
            Self::Ltr => 1.0,
            Self::Rtl => -1.0,
        }
    }
}

impl NavKey {
    /// The key as it moves through the columns in `direction`.
    ///
    /// [`Left`](NavKey::Left) and [`Right`](NavKey::Right) say where a key points
    /// on screen; [`navigate`](crate::navigate) moves to the column before or
    /// after. In right to left text those are the other way round.
    ///
    /// ```
    /// use datagrid_core::{Direction, NavKey};
    ///
    /// assert_eq!(NavKey::Left.in_direction(Direction::Ltr), NavKey::Left);
    /// assert_eq!(NavKey::Left.in_direction(Direction::Rtl), NavKey::Right);
    /// assert_eq!(NavKey::Down.in_direction(Direction::Rtl), NavKey::Down);
    /// ```
    #[must_use]
    pub const fn in_direction(self, direction: Direction) -> Self {
        match (self, direction) {
            (Self::Left, Direction::Rtl) => Self::Right,
            (Self::Right, Direction::Rtl) => Self::Left,
            _ => self,
        }
    }
}
