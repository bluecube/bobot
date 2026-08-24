use std::{
    array,
    ops::{BitAnd, BitOr, Not},
};

use bit_iter::BitIter;
use serde::{Deserialize, Serialize};
use wide::{u16x16, u64x4};

/// Bitboard for represetnting positions on a board.
/// This implementation only handles for board sizes <= 16 (9x9 and 13x13).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Bitboard16 {
    bits: u16x16,
}

/// Position on the board
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub row: usize,
    pub col: usize,
}

impl Position {
    pub fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }
}

impl From<(usize, usize)> for Position {
    fn from(value: (usize, usize)) -> Self {
        Position {
            row: value.0,
            col: value.1,
        }
    }
}

impl Default for Bitboard16 {
    fn default() -> Self {
        Self::new()
    }
}

impl Bitboard16 {
    /// Creates a bitboard with no positions set.
    pub fn new() -> Self {
        Bitboard16 {
            bits: u16x16::splat(0),
        }
    }

    /// Creates a bitboard with a single position set.
    pub fn single(pos: Position) -> Self {
        let mut bb = Self::new();
        bb.set(pos, true);
        bb
    }

    /// Creates a mask where the first board_size rows and columns are set, rest is zero
    /// Generates full 16x16 masks for out of bounds `board_size`.
    pub fn board_mask(board_size: usize) -> Self {
        let row_bits = 1u16.unbounded_shl(board_size as u32).wrapping_sub(1);
        let array = std::array::from_fn(|i| if i < board_size { row_bits } else { 0 });
        Bitboard16 {
            bits: u16x16::new(array),
        }
    }

    /// Returns a value of a single bit. Slow.
    /// Panics on out of range index.
    pub fn get(&self, pos: Position) -> bool {
        assert!(pos.row < 16);
        assert!(pos.col < 16);
        (self.bits.as_array()[pos.row] >> pos.col) & 1 != 0
    }

    /// Sets a value of a single bit. Slow.
    /// Panics on out of range index.
    pub fn set(&mut self, pos: Position, value: bool) {
        assert!(pos.row < 16);
        assert!(pos.col < 16);
        let row = &mut self.bits.as_mut_array()[pos.row];
        if value {
            *row |= 1 << pos.col;
        } else {
            *row &= !(1 << pos.col);
        }
    }

    /// Builds a bitboard from ascii representation, compatible with `Self::format_ascii`.
    /// Each line in text is a row in the bitboard, 'x' marks set position,
    /// any other char marks an unset position.
    /// Panics if any set position is outside the 16x16 boundary.
    pub fn from_ascii(s: &str) -> Bitboard16 {
        let mut bb = Bitboard16::new();
        for (row_number, line) in s.lines().enumerate() {
            for (col_number, c) in line.chars().enumerate() {
                if c == 'x' {
                    bb.set(Position::new(row_number, col_number), true);
                }
            }
        }

        bb
    }

    /// Generates a viewable string represetnation of the board, compatible with
    /// `Self::from_ascii`.
    /// Empty positions are drawn as `'.'` if within a `pad_to` by `pad_to` square,
    /// otherwise only if followed by a set position.
    /// Set cells are drawn as `'x'`.
    /// Panics on out of range `pad_to`.
    pub fn format_ascii(&self, pad_to: usize) -> String {
        format_ascii_helper(self.iter_rows(), |x| x.then_some('x'), pad_to)
    }

    /// Calculates the number of set bits in the bitboard
    pub fn popcnt(&self) -> usize {
        // Converting to u64x4 first compiles to better assembly
        let as64: u64x4 = bytemuck::cast(self.bits);
        as64.as_array()
            .iter()
            .map(|x| x.count_ones() as usize)
            .sum()
    }

    /// Returns true if all positions are unset.
    pub fn is_empty(&self) -> bool {
        self.bits == u16x16::splat(0)
    }

    /// Returns iterator over positions (`(row, col)`) of set bits.
    pub fn iter_positions(self) -> impl Iterator<Item = Position> {
        self.bits
            .to_array()
            .into_iter()
            .enumerate()
            .flat_map(|(row, row_bits)| {
                BitIter::from(row_bits).map(move |col| Position::new(row, col))
            })
    }

    /// Returns an iterator over maximal disjoint 4-connected subsets of `self`.
    /// Union of all groups is always equal to `self`, ordering is unspecified.
    pub fn iter_groups(self) -> impl Iterator<Item = Bitboard16> {
        let mut bb = self;

        std::iter::from_fn(move || {
            if bb.is_empty() {
                None
            } else {
                let group = bb.arbitrary_set_bit().flood_fill(bb);
                bb = bb & !group;
                Some(group)
            }
        })
    }

    pub fn iter_rows(self) -> impl Iterator<Item = RowIterator> {
        self.bits
            .to_array()
            .into_iter()
            .map(|row_bits| RowIterator {
                bits: row_bits,
                pos: 0,
            })
    }

    /// Extracts a bitboard that is a subset of `self` and has a single bit set
    /// (unless `self` is empty, then result is empty too).
    /// Should be fast.
    pub fn arbitrary_set_bit(&self) -> Bitboard16 {
        let as64: u64x4 = bytemuck::cast(self.bits);

        /// Shifts the content of vector by n lanes up.
        fn shift_lanes(v: u64x4, n: usize) -> u64x4 {
            assert!(n < 4);
            let input_array = v.as_array();

            let output_array = array::from_fn(|i| {
                if i < (input_array.len() - n) {
                    input_array[i + n]
                } else {
                    0
                }
            });

            u64x4::new(output_array)
        }

        let lanes_above_squashed =
            shift_lanes(as64, 1) | shift_lanes(as64, 2) | shift_lanes(as64, 3);
        let mask = lanes_above_squashed.simd_eq(0);

        // has at most one bit set in every 64bit lane
        let single_bits = as64 & (-as64);

        Bitboard16 {
            bits: bytemuck::cast(single_bits & mask),
        }
    }
}

/// Dilation and flood fill
impl Bitboard16 {
    /// Expands the bitboard four ways (left, right, up, down) by one position.
    pub fn dilate(&self) -> Bitboard16 {
        self | self.shift_left(1) | self.shift_right(1) | self.shift_up(1) | self.shift_down(1)
    }

    /// Expands the bitboard in self to fill contiguous area within `mask`.
    /// Clips the seed to be inside mask first.
    pub fn flood_fill(&self, mask: Bitboard16) -> Bitboard16 {
        let mut current = self & mask;
        loop {
            let next = current.fill_left(mask) | current.fill_right(mask);
            let next = next.fill_up(mask) | next.fill_down(mask);
            if next == current {
                return current;
            }
            current = next;
        }
    }

    /// Runs a segment of flood filling, expanding only left
    fn fill_left(&self, mask: Bitboard16) -> Bitboard16 {
        let mut current = *self;
        let mut mask = mask;

        current = current | current.shift_left(1) & mask;
        mask = mask & mask.shift_left(1);
        current = current | current.shift_left(2) & mask;
        mask = mask & mask.shift_left(2);
        current = current | current.shift_left(4) & mask;
        mask = mask & mask.shift_left(4);
        current = current | current.shift_left(8) & mask;

        current
    }

    /// Runs a segment of flood filling, expanding only right
    fn fill_right(&self, mask: Bitboard16) -> Bitboard16 {
        let mut current = *self;
        let mut mask = mask;

        current = current | current.shift_right(1) & mask;
        mask = mask & mask.shift_right(1);
        current = current | current.shift_right(2) & mask;
        mask = mask & mask.shift_right(2);
        current = current | current.shift_right(4) & mask;
        mask = mask & mask.shift_right(4);
        current = current | current.shift_right(8) & mask;

        current
    }

    /// Runs a segment of flood filling, expanding only up
    fn fill_up(&self, mask: Bitboard16) -> Bitboard16 {
        let mut current = *self;
        let mut mask = mask;

        current = current | current.shift_up(1) & mask;
        mask = mask & mask.shift_up(1);
        current = current | current.shift_up(2) & mask;
        mask = mask & mask.shift_up(2);
        current = current | current.shift_up(4) & mask;
        mask = mask & mask.shift_up(4);
        current = current | current.shift_up(8) & mask;

        current
    }

    /// Runs a segment of flood filling, expanding only down
    fn fill_down(&self, mask: Bitboard16) -> Bitboard16 {
        let mut current = *self;
        let mut mask = mask;

        current = current | current.shift_down(1) & mask;
        mask = mask & mask.shift_down(1);
        current = current | current.shift_down(2) & mask;
        mask = mask & mask.shift_down(2);
        current = current | current.shift_down(4) & mask;
        mask = mask & mask.shift_down(4);
        current = current | current.shift_down(8) & mask;

        current
    }
}

/// Shifting
impl Bitboard16 {
    /// Shifts the content of the bitboard to the left (decreasing column indices of set bits).
    fn shift_left(&self, n: usize) -> Bitboard16 {
        Bitboard16 {
            bits: self.bits.unbounded_shr_scalar(n as u32),
        }
    }

    /// Shifts the content of the bitboard to the right (increasing column indices of set bits).
    fn shift_right(&self, n: usize) -> Bitboard16 {
        Bitboard16 {
            bits: self.bits.unbounded_shl_scalar(n as u32),
        }
    }

    /// Shifts the content of the bitboard up (decreasing row indices of set bits).
    fn shift_up(&self, n: usize) -> Bitboard16 {
        assert!(n < 16);
        let input_array = self.bits.as_array();

        let output_array = array::from_fn(|i| {
            if i < (input_array.len() - n) {
                input_array[i + n]
            } else {
                0
            }
        });

        Bitboard16 {
            bits: u16x16::new(output_array),
        }
    }

    /// Shifts the content of the bitboard down (increasing row indices of set bits).
    fn shift_down(&self, n: usize) -> Bitboard16 {
        assert!(n < 16);
        let input_array = self.bits.as_array();

        let output_array = array::from_fn(|i| if i >= n { input_array[i - n] } else { 0 });

        Bitboard16 {
            bits: u16x16::new(output_array),
        }
    }
}

impl BitOr for Bitboard16 {
    type Output = Bitboard16;

    fn bitor(self, rhs: Bitboard16) -> Bitboard16 {
        Bitboard16 {
            bits: self.bits | rhs.bits,
        }
    }
}

impl BitOr<&Bitboard16> for Bitboard16 {
    type Output = Bitboard16;

    fn bitor(self, rhs: &Bitboard16) -> Bitboard16 {
        Bitboard16 {
            bits: self.bits | rhs.bits,
        }
    }
}

impl BitOr for &Bitboard16 {
    type Output = Bitboard16;

    fn bitor(self, rhs: &Bitboard16) -> Bitboard16 {
        Bitboard16 {
            bits: self.bits | rhs.bits,
        }
    }
}

impl BitOr<Bitboard16> for &Bitboard16 {
    type Output = Bitboard16;

    fn bitor(self, rhs: Bitboard16) -> Bitboard16 {
        Bitboard16 {
            bits: self.bits | rhs.bits,
        }
    }
}

impl BitAnd for Bitboard16 {
    type Output = Bitboard16;

    fn bitand(self, rhs: Bitboard16) -> Bitboard16 {
        Bitboard16 {
            bits: self.bits & rhs.bits,
        }
    }
}

impl BitAnd<&Bitboard16> for Bitboard16 {
    type Output = Bitboard16;

    fn bitand(self, rhs: &Bitboard16) -> Bitboard16 {
        Bitboard16 {
            bits: self.bits & rhs.bits,
        }
    }
}

impl BitAnd for &Bitboard16 {
    type Output = Bitboard16;

    fn bitand(self, rhs: &Bitboard16) -> Bitboard16 {
        Bitboard16 {
            bits: self.bits & rhs.bits,
        }
    }
}

impl BitAnd<Bitboard16> for &Bitboard16 {
    type Output = Bitboard16;

    fn bitand(self, rhs: Bitboard16) -> Bitboard16 {
        Bitboard16 {
            bits: self.bits & rhs.bits,
        }
    }
}

impl Not for Bitboard16 {
    type Output = Bitboard16;

    fn not(self) -> Self::Output {
        Bitboard16 { bits: !self.bits }
    }
}

impl FromIterator<Position> for Bitboard16 {
    fn from_iter<T: IntoIterator<Item = Position>>(iter: T) -> Self {
        let mut bb = Bitboard16::new();
        for pos in iter {
            bb.set(pos, true);
        }
        bb
    }
}

/// Iterates over bits of a row, returning true for set positions and false for unset ones.
#[derive(Copy, Clone, Debug)]
pub struct RowIterator {
    bits: u16,
    pos: u16,
}

impl Iterator for RowIterator {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < 16 {
            let v = (self.bits >> self.pos) & 1 != 0;
            self.pos += 1;
            Some(v)
        } else {
            None
        }
    }
}

/// Helper to format a bitboard to ascii.
/// Empty positions are drawn as `'.'` if within a `pad_to` by `pad_to` square,
/// otherwise only if followed by a set position.
/// - `rows_iterator`: Iterator of rows, each of those is an iterator of cells.
/// - `cell_fn`: Applied to every cell in the iterator output to get the character, or None for empty cell;
///
/// Panics on out of range (> 16) `pad_to` or if rows_iterator or any of the first `pad_to`
/// cell iterators has less entries than `pad_to`.
pub(crate) fn format_ascii_helper<RowsIterator, CellFn>(
    rows_iterator: RowsIterator,
    mut cell_fn: CellFn,
    pad_to: usize,
) -> String
where
    RowsIterator: IntoIterator,
    RowsIterator::Item: IntoIterator,
    CellFn: FnMut(<RowsIterator::Item as IntoIterator>::Item) -> Option<char>,
{
    assert!(pad_to <= 16);

    let mut result = String::with_capacity(pad_to * (pad_to + 1));
    let mut pending_rows = 0;
    let mut n_rows = 0;

    for (row, row_data) in rows_iterator.into_iter().enumerate() {
        n_rows += 1;

        let mut pending_cols = 0;
        let mut is_row_empty = true;
        let mut n_cols = 0;
        for (col, cell) in row_data.into_iter().enumerate() {
            n_cols += 1;
            match cell_fn(cell) {
                Some(c) => {
                    for _ in 0..pending_rows {
                        result.push('\n');
                    }
                    pending_rows = 0;
                    for _ in 0..pending_cols {
                        result.push('.');
                    }
                    pending_cols = 0;
                    result.push(c);
                    is_row_empty = false;
                }
                None if (row < pad_to) & (col < pad_to) => {
                    assert_eq!(pending_rows, 0);
                    assert_eq!(pending_cols, 0);
                    result.push('.');
                    is_row_empty = false;
                }
                _ => {
                    pending_cols += 1;
                }
            }
        }

        if row < pad_to {
            assert!(n_cols >= pad_to);
        }

        if is_row_empty {
            assert!(row >= pad_to);
            pending_rows += 1;
        } else {
            result.push('\n');
        }
    }

    assert!(n_rows >= pad_to);

    result
}

#[cfg(test)]
impl proptest::arbitrary::Arbitrary for Bitboard16 {
    type Parameters = ();
    type Strategy = proptest::strategy::BoxedStrategy<Bitboard16>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        use proptest::prelude::Strategy;

        let with_weights = (0f64..=1f64)
            .prop_flat_map(|weight| {
                proptest::collection::vec(proptest::bool::weighted(weight), 256)
            })
            .prop_map(|v| {
                let mut bb = Bitboard16::new();
                for (i, set) in v.iter().enumerate() {
                    if *set {
                        bb.set(Position::new(i / 16, i % 16), true);
                    }
                }
                bb
            });

        let sparse =
            proptest::collection::hash_set((0usize..16usize, 0usize..16usize), 0usize..64usize)
                .prop_map(|set| {
                    let mut bb = Bitboard16::new();
                    for pos in set {
                        bb.set(pos.into(), true);
                    }

                    bb
                });

        let dense = sparse.clone().prop_map(|bb| !bb);

        proptest::prop_oneof![sparse, dense, with_weights].boxed()
    }
}

#[cfg(test)]
mod test {
    use crate::bitboard::{Bitboard16, Position};
    use proptest::property_test;

    fn transpose(bb: Bitboard16) -> Bitboard16 {
        let mut output = Bitboard16::new();
        for pos in bb.iter_positions() {
            output.set(Position::new(pos.col, pos.row), true);
        }
        output
    }

    #[test]
    fn new_is_empty() {
        assert!(Bitboard16::new().is_empty())
    }

    #[test]
    fn new_has_no_bits_set() {
        let bb = Bitboard16::new();

        for row in 0..16 {
            for col in 0..16 {
                assert!(!bb.get(Position::new(row, col)));
            }
        }
    }

    #[test]
    fn set_five() {
        let mut bb = Bitboard16::new();

        bb.set(Position::new(0, 0), true);
        bb.set(Position::new(3, 7), true);
        bb.set(Position::new(1, 15), true);
        bb.set(Position::new(15, 14), true);
        bb.set(Position::new(8, 4), true);

        assert_eq!(bb.popcnt(), 5);
    }

    #[test]
    fn full_board_mask_negated_is_empty() {
        assert!((!Bitboard16::board_mask(16)).is_empty())
    }

    #[property_test]
    fn board_mask_bits(#[strategy = 0usize..=16usize] board_size: usize) {
        let bb = Bitboard16::board_mask(board_size);

        for row in 0usize..16usize {
            for col in 0usize..16usize {
                if (row < board_size) & (col < board_size) {
                    assert!(bb.get(Position::new(row, col)));
                } else {
                    assert!(!bb.get(Position::new(row, col)));
                }
            }
        }
    }

    #[property_test]
    fn iterator_bits_are_set(bb: Bitboard16) {
        for pos in bb.iter_positions() {
            assert!(bb.get(pos));
        }
    }

    #[property_test]
    fn copy_using_set_is_equal(bb: Bitboard16) {
        let mut copy = Bitboard16::new();

        for pos in bb.iter_positions() {
            copy.set(pos, true);
        }

        assert_eq!(copy, bb);
    }

    #[property_test]
    fn clear_using_set_is_empty(bb: Bitboard16) {
        let mut copy = bb;

        for pos in bb.iter_positions() {
            copy.set(pos, false);
        }

        assert!(copy.is_empty());
    }

    mod ascii {
        use super::*;

        #[property_test]
        fn roundtrip(bb: Bitboard16, #[strategy = 0usize..=16usize] pad_to: usize) {
            let s = bb.format_ascii(pad_to);

            assert_eq!(Bitboard16::from_ascii(&s), bb);
        }

        #[property_test]
        fn padding(bb: Bitboard16, #[strategy = 0usize..=16usize] pad_to: usize) {
            let s = bb.format_ascii(pad_to);

            assert!(s.lines().count() >= pad_to);
            assert!(s.lines().take(pad_to).all(|l| l.len() >= pad_to))
        }

        #[property_test]
        fn compacting(bb: Bitboard16) {
            let s = bb.format_ascii(0);

            assert!(s.lines().all(|l| l.is_empty() || l.ends_with('x')));
            assert!(s.lines().last().is_none_or(|l| !l.is_empty()));
        }

        mod examples {
            use super::*;

            #[test]
            fn from_ascii() {
                let bb = Bitboard16::from_ascii(
                    ".........\n\
                     x........\n\
                     .....x...\n\
                     ....x....\n",
                );

                assert!(bb.get(Position::new(1, 0)));
                assert!(bb.get(Position::new(2, 5)));
                assert!(bb.get(Position::new(3, 4)));
            }

            #[test]
            fn padding_empty() {
                let bb = Bitboard16::new();
                assert_eq!(bb.format_ascii(4), "....\n....\n....\n....\n")
            }

            #[test]
            fn padding_over() {
                let bb = Bitboard16::single(Position::new(10, 10));
                assert_eq!(
                    bb.format_ascii(4),
                    "....\n....\n....\n....\n\n\n\n\n\n\n..........x\n"
                )
            }
        }
    }

    mod iter_groups {
        use super::*;

        #[test]
        fn empty() {
            assert_eq!(Bitboard16::new().iter_groups().count(), 0);
        }

        #[property_test]
        fn cover_everything(bb: Bitboard16) {
            assert_eq!(bb.iter_groups().fold(Bitboard16::new(), |a, b| a | b), bb);
        }

        #[property_test]
        fn are_disjoint(bb: Bitboard16) {
            use itertools::Itertools as _;
            let groups: Vec<_> = bb.iter_groups().collect();

            for ((i, a), (j, b)) in groups
                .iter()
                .enumerate()
                .cartesian_product(groups.iter().enumerate())
            {
                if i == j {
                    // If we compared boards and not indices, this would allow duplicate groups
                    continue;
                }
                assert!((a & b).is_empty());
            }
        }

        #[property_test]
        fn are_maximum_shape(bb: Bitboard16) {
            for group in bb.iter_groups() {
                assert_eq!(group.dilate() & bb, group);
            }
        }

        mod examples {
            use super::*;

            #[test]
            fn two_diagonal() {
                let bb = Bitboard16::from_ascii("x\n.x");
                for group in bb.iter_groups() {
                    assert_eq!(group.popcnt(), 1);
                }
            }

            #[test]
            fn checkerboard() {
                let mut bb = Bitboard16::new();
                for r in 0..16 {
                    for i in 0..8 {
                        let c = 2 * i + r % 2;

                        bb.set(Position::new(r, c), true);
                    }
                }

                for group in bb.iter_groups() {
                    assert_eq!(group.popcnt(), 1);
                }
            }
        }
    }

    mod iter_rows {
        use super::*;

        #[property_test]
        fn popcnt(bb: Bitboard16) {
            assert_eq!(bb.iter_rows().flatten().filter(|b| *b).count(), bb.popcnt());
        }

        #[property_test]
        fn row_count(bb: Bitboard16) {
            assert_eq!(bb.iter_rows().count(), 16);
        }

        #[property_test]
        fn col_count(bb: Bitboard16) {
            assert!(bb.iter_rows().all(|row| row.count() == 16));
        }

        #[property_test]
        fn get_bits(bb: Bitboard16) {
            assert!(
                bb.iter_rows()
                    .enumerate()
                    .flat_map(|(r, r_iter)| r_iter.enumerate().map(move |(c, bit)| (r, c, bit)))
                    .all(|(r, c, b)| bb.get(Position::new(r, c)) == b)
            )
        }
    }

    mod shift {
        use super::*;

        mod reference {
            use super::*;

            /// Implements a general shift by using single bit access to act as a baseline
            fn reference(bb: Bitboard16, dr: isize, dc: isize) -> Bitboard16 {
                let mut result = Bitboard16::new();
                for pos in bb.iter_positions() {
                    let r = pos.row as isize + dr;
                    let c = pos.col as isize + dc;

                    if !(0isize..16isize).contains(&r) {
                        continue;
                    }

                    if !(0isize..16isize).contains(&c) {
                        continue;
                    }

                    result.set(Position::new(r as usize, c as usize), true);
                }

                result
            }

            #[property_test]
            fn left(bb: Bitboard16, #[strategy = 0usize..16usize] n: usize) {
                assert_eq!(bb.shift_left(n), reference(bb, 0, -(n as isize)))
            }

            #[property_test]
            fn right(bb: Bitboard16, #[strategy = 0usize..16usize] n: usize) {
                assert_eq!(bb.shift_right(n), reference(bb, 0, n as isize))
            }

            #[property_test]
            fn up(bb: Bitboard16, #[strategy = 0usize..16usize] n: usize) {
                assert_eq!(bb.shift_up(n), reference(bb, -(n as isize), 0))
            }

            #[property_test]
            fn down(bb: Bitboard16, #[strategy = 0usize..16usize] n: usize) {
                assert_eq!(bb.shift_down(n), reference(bb, n as isize, 0))
            }
        }

        mod out_of_range {
            use super::*;

            #[test]
            fn left() {
                assert!(
                    Bitboard16::single(Position::new(5, 0))
                        .shift_left(1)
                        .is_empty()
                );
            }

            #[test]
            fn right() {
                assert!(
                    Bitboard16::single(Position::new(5, 15))
                        .shift_right(1)
                        .is_empty()
                );
            }

            #[test]
            fn up() {
                assert!(
                    Bitboard16::single(Position::new(0, 5))
                        .shift_up(1)
                        .is_empty()
                );
            }

            #[test]
            fn down() {
                assert!(
                    Bitboard16::single(Position::new(15, 5))
                        .shift_down(1)
                        .is_empty()
                );
            }
        }

        mod transpose_symmetry {
            use super::*;

            #[property_test]
            fn left_up(bb: Bitboard16, #[strategy = 0usize..16usize] n: usize) {
                assert_eq!(transpose(bb.shift_left(n)), transpose(bb).shift_up(n))
            }

            #[property_test]
            fn right_down(bb: Bitboard16, #[strategy = 0usize..16usize] n: usize) {
                assert_eq!(transpose(bb.shift_right(n)), transpose(bb).shift_down(n))
            }
        }

        #[test]
        fn up_accross_half() {
            let bb = Bitboard16::single(Position::new(8, 5)).shift_up(1);

            assert_eq!(bb.popcnt(), 1);
            assert!(bb.get(Position::new(7, 5)))
        }

        #[test]
        fn down_accross_half() {
            let bb = Bitboard16::single(Position::new(7, 5)).shift_down(1);

            assert_eq!(bb.popcnt(), 1);
            assert!(bb.get(Position::new(8, 5)))
        }
    }

    mod dilate {
        use super::*;

        #[property_test]
        fn single_position_dilates_to_five(
            #[strategy = 0usize..16usize] row: usize,
            #[strategy = 0usize..16usize] col: usize,
        ) {
            let bb = Bitboard16::single(Position::new(row, col));

            let expected_count = 1
                + if (row == 0) | (row == 15) { 1 } else { 2 }
                + if (col == 0) | (col == 15) { 1 } else { 2 };

            assert_eq!(bb.dilate().popcnt(), expected_count);
        }

        #[property_test]
        fn is_distributive(bb1: Bitboard16, bb2: Bitboard16) {
            assert_eq!((bb1 | bb2).dilate(), bb1.dilate() | bb2.dilate());
        }

        #[property_test]
        fn never_clears(bb: Bitboard16) {
            assert_eq!(bb.dilate() & bb, bb);
        }

        #[test]
        fn empty() {
            assert!(Bitboard16::new().dilate().is_empty());
        }

        #[test]
        fn full() {
            let full = !Bitboard16::new();
            assert_eq!(full.dilate(), full);
        }

        #[test]
        fn near_half_vector_boundary() {
            // This would be more important if we were hand-rolling the
            // vectorized code, but it's still a good test.

            let mut bb = Bitboard16::new();
            bb.set(Position::new(7, 2), true);
            bb.set(Position::new(8, 6), true);

            let bb = bb.dilate();

            assert!(bb.get(Position::new(6, 2)));
            assert!(bb.get(Position::new(7, 1)));
            assert!(bb.get(Position::new(7, 2)));
            assert!(bb.get(Position::new(7, 3)));
            assert!(bb.get(Position::new(8, 2)));

            assert!(bb.get(Position::new(7, 6)));
            assert!(bb.get(Position::new(8, 5)));
            assert!(bb.get(Position::new(8, 6)));
            assert!(bb.get(Position::new(8, 7)));
            assert!(bb.get(Position::new(9, 6)));

            assert_eq!(bb.popcnt(), 10);
        }

        #[test]
        fn corner_zero() {
            let bb = Bitboard16::single(Position::new(0, 0));

            assert_eq!(bb.dilate().popcnt(), 3);
        }

        #[test]
        fn corner_max() {
            let bb = Bitboard16::single(Position::new(15, 15));

            assert_eq!(bb.dilate().popcnt(), 3);
        }

        fn dilate_reference(bb: Bitboard16) -> Bitboard16 {
            let mut output = bb;
            for Position { row: r, col: c } in bb.iter_positions() {
                if r > 0 {
                    output.set(Position::new(r - 1, c), true);
                }
                if r < 15 {
                    output.set(Position::new(r + 1, c), true);
                }
                if c > 0 {
                    output.set(Position::new(r, c - 1), true);
                }
                if c < 15 {
                    output.set(Position::new(r, c + 1), true);
                }
            }
            output
        }

        #[property_test]
        fn reference(bb: Bitboard16) {
            assert_eq!(bb.dilate(), dilate_reference(bb));
        }

        #[property_test]
        fn transpose_symmetry(bb: Bitboard16) {
            assert_eq!(transpose(bb.dilate()), transpose(bb).dilate());
        }
    }

    mod flood_fill {
        use super::*;

        #[property_test]
        fn subset_of_mask(seed: Bitboard16, mask: Bitboard16) {
            let ff = seed.flood_fill(mask);
            assert_eq!(ff, ff & mask);
        }

        #[property_test]
        fn contains_clipped_seed(seed: Bitboard16, mask: Bitboard16) {
            let ff = seed.flood_fill(mask);
            let clipped_seed = seed & mask;
            assert_eq!(ff & clipped_seed, clipped_seed);
        }

        #[property_test]
        fn clips_seed(seed: Bitboard16, mask: Bitboard16) {
            // This property is part of the function contract from the docstring
            assert_eq!(seed.flood_fill(mask), (seed & mask).flood_fill(mask));
        }

        #[property_test]
        fn idempotence(seed: Bitboard16, mask: Bitboard16) {
            assert_eq!(
                seed.flood_fill(mask).flood_fill(mask),
                seed.flood_fill(mask)
            );
        }

        #[property_test]
        fn distributive_over_seed(seed1: Bitboard16, seed2: Bitboard16, mask: Bitboard16) {
            assert_eq!(
                (seed1 | seed2).flood_fill(mask),
                seed1.flood_fill(mask) | seed2.flood_fill(mask)
            );
        }

        #[property_test]
        fn transpose_symmetry(seed: Bitboard16, mask: Bitboard16) {
            assert_eq!(
                transpose(seed).flood_fill(transpose(mask)),
                transpose(seed.flood_fill(mask))
            );
        }

        #[property_test]
        fn empty_seed(mask: Bitboard16) {
            assert!(Bitboard16::new().flood_fill(mask).is_empty());
        }

        mod examples {
            use super::*;

            #[test]
            fn snake1() {
                let mask = Bitboard16::from_ascii(
                    "xxxxxxxxxxxxxxxx\n\
                     ...............x\n\
                     xxxxxxxxxxxxxxxx\n\
                     x...............\n\
                     xxxxxxxxxxxxxxxx\n\
                     ...............x\n\
                     xxxxxxxxxxxxxxxx\n\
                     x...............\n\
                     xxxxxxxxxxxxxxxx\n\
                     ...............x\n\
                     xxxxxxxxxxxxxxxx\n\
                     x...............\n\
                     xxxxxxxxxxxxxxxx\n\
                     ...............x\n\
                     xxxxxxxxxxxxxxxx\n\
                     x...............",
                );
                let seed = Bitboard16::single(Position::new(0, 0));

                assert_eq!(seed.flood_fill(mask), mask);
                assert_eq!(seed.flood_fill(transpose(mask)), transpose(mask));
            }

            #[test]
            fn snake2() {
                let mask = Bitboard16::from_ascii(
                    "xxxxxxxxxxxxxxxx\n\
                     ...............x\n\
                     xxxxxxxxxxxxxx.x\n\
                     x............x.x\n\
                     x.xxxxxxxxxx.x.x\n\
                     x.x........x.x.x\n\
                     x.x.xxxxxx.x.x.x\n\
                     x.x.x....x.x.x.x\n\
                     x.x.x.x..x.x.x.x\n\
                     x.x.x.xxxx.x.x.x\n\
                     x.x.x......x.x.x\n\
                     x.x.xxxxxxxx.x.x\n\
                     x.x..........x.x\n\
                     x.xxxxxxxxxxxx.x\n\
                     x..............x\n\
                     xxxxxxxxxxxxxxxx",
                );
                let seed = Bitboard16::single(Position::new(0, 0));

                assert_eq!(seed.flood_fill(mask), mask);
                assert_eq!(seed.flood_fill(transpose(mask)), transpose(mask));
            }

            #[test]
            fn full() {
                let mask = Bitboard16::board_mask(16);
                let seed = Bitboard16::single(Position::new(0, 0));

                assert_eq!(seed.flood_fill(mask), mask);
            }

            #[test]
            fn two_islands() {
                let mask = Bitboard16::from_ascii(
                    "................\n\
                     ................\n\
                     ....xxxxxxxx....\n\
                     .....xxxxxxxxx..\n\
                     xxxxxxxxxxxx....\n\
                     .xx.....x.......\n\
                     ..xx........xx..\n\
                     ..xxxx.....xx...\n\
                     ..xxxx....xxxx..\n\
                     ...xx.....x..x..\n\
                     ...xx....xxxxx..",
                );

                let seed1 = Bitboard16::single(Position::new(5, 2));
                let seed2 = Bitboard16::single(Position::new(9, 13));

                let ff1 = seed1.flood_fill(mask);
                let ff2 = seed2.flood_fill(mask);

                assert!((ff1 & ff2).is_empty());
                assert_eq!(ff1 | ff2, mask);
            }

            #[test]
            fn staircase() {
                let mask = Bitboard16::from_ascii(
                    "x\n\
                     .x\n\
                     ..x\n\
                     ...x\n\
                     ....x\n\
                     .....x\n\
                     ......x\n\
                     .......x\n\
                     ........x\n\
                     .........x\n\
                     ..........x\n\
                     ...........x\n\
                     ............x\n\
                     .............x\n\
                     ..............x\n\
                     ...............x",
                );
                let seed = Bitboard16::single(Position::new(8, 8));

                assert_eq!(seed.flood_fill(mask), seed);
            }

            #[test]
            fn not_skipping_a_gap() {
                let mask = Bitboard16::from_ascii("xxx.xx");
                let seed = Bitboard16::single(Position::new(0, 0));

                assert_eq!(seed.flood_fill(mask).popcnt(), 3);
            }
        }
    }

    mod arbitrary_set_bit {
        use proptest::prop_assume;

        use super::*;

        #[test]
        fn empty() {
            assert!(Bitboard16::new().arbitrary_set_bit().is_empty());
        }

        #[property_test]
        fn nonempty_is_single_bit(bb: Bitboard16) {
            prop_assume!(!bb.is_empty());

            assert_eq!(bb.arbitrary_set_bit().popcnt(), 1);
        }

        #[property_test]
        fn is_subset(bb: Bitboard16) {
            let bit = bb.arbitrary_set_bit();
            assert_eq!(bit & bb, bit);
        }

        #[property_test]
        fn idempotence(bb: Bitboard16) {
            let bit = bb.arbitrary_set_bit();
            assert_eq!(bit.arbitrary_set_bit(), bit);
        }

        #[property_test]
        fn removal_loop(bb: Bitboard16) {
            let mut count = 0;
            let mut current = bb;

            while !current.is_empty() {
                let bit = current.arbitrary_set_bit();
                assert!(!(bit.is_empty()));
                current = current & (!bit);
                count += 1;
            }

            assert_eq!(count, bb.popcnt());
        }
    }
}
