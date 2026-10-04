#[cfg(test)]
use proptest::{
    arbitrary::Arbitrary,
    strategy::{BoxedStrategy, Strategy},
};
use serde::{Deserialize, Serialize};

/// Position on the board
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct Position {
    pub row: usize,
    pub col: usize,
}

impl Position {
    pub fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }

    pub fn apply_symmetry(&self, symmetry: Symmetry, board_size: usize) -> Position {
        assert!(self.row < board_size);
        assert!(self.col < board_size);

        let mut result = *self;

        if symmetry.hflip {
            result.col = board_size - result.col - 1;
        }

        if symmetry.vflip {
            result.row = board_size - result.row - 1;
        }

        if symmetry.transpose {
            (result.row, result.col) = (result.col, result.row);
        }

        result
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

/// Describes a D4 symmetry to be applied to the board.
/// The three elements are applied in order: hflip, then vflip, then transpose.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Symmetry {
    pub hflip: bool,
    pub vflip: bool,
    pub transpose: bool,
}

#[cfg(test)]
impl Arbitrary for Symmetry {
    type Parameters = ();
    type Strategy = BoxedStrategy<Symmetry>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        (
            proptest::bool::ANY,
            proptest::bool::ANY,
            proptest::bool::ANY,
        )
            .prop_map(|(hflip, vflip, transpose)| Symmetry {
                hflip,
                vflip,
                transpose,
            })
            .boxed()
    }
}
