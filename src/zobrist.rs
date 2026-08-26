use std::{
    fmt::Debug,
    hash::Hash,
    ops::{BitXor, BitXorAssign},
};

use rand::RngExt;

use crate::{Color, Position, color::ColorMap};

#[derive(Clone)]
pub struct ZobristHasher {
    hashes: ColorMap<Box<[ZobristHash]>>,
    board_size: usize,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ZobristHash(u64);

impl ZobristHasher {
    pub fn new(board_size: usize, rng: &mut impl rand::Rng) -> ZobristHasher {
        ZobristHasher {
            hashes: [
                Self::make_grid(board_size, rng),
                Self::make_grid(board_size, rng),
            ]
            .into(),
            board_size,
        }
    }

    pub fn stone(&self, pos: Position, color: Color) -> ZobristHash {
        self.hashes[color][self.index(pos)]
    }

    pub fn board_size(&self) -> usize {
        self.board_size
    }

    fn make_grid(board_size: usize, rng: &mut impl rand::Rng) -> Box<[ZobristHash]> {
        (0..(board_size * board_size))
            .map(|_| ZobristHash(rng.random()))
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }

    fn index(&self, pos: Position) -> usize {
        debug_assert!(pos.row < self.board_size);
        debug_assert!(pos.col < self.board_size);
        pos.row * self.board_size + pos.col
    }
}

impl BitXor for ZobristHash {
    type Output = ZobristHash;

    fn bitxor(self, rhs: Self) -> Self::Output {
        ZobristHash(self.0 ^ rhs.0)
    }
}

impl BitXorAssign for ZobristHash {
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

impl From<ZobristHash> for u64 {
    fn from(value: ZobristHash) -> Self {
        value.0
    }
}

impl Debug for ZobristHasher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ZobristHasher")
            .field("board_size", &self.board_size)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
pub fn zobrist_hasher_strategy(
    size: impl proptest::strategy::Strategy<Value = usize>,
) -> impl proptest::strategy::Strategy<Value = ZobristHasher> {
    use proptest::strategy::Strategy as _;
    use rand::SeedableRng as _;

    (size, proptest::num::u64::ANY.no_shrink()).prop_map(|(board_size, seed)| {
        ZobristHasher::new(board_size, &mut rand::rngs::SmallRng::seed_from_u64(seed))
    })
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    use proptest::property_test;

    use super::*;

    #[property_test]
    fn all_unique(#[strategy = zobrist_hasher_strategy(0usize..=19usize)] hasher: ZobristHasher) {
        let mut set: HashSet<u64> = HashSet::new();
        let board_size = hasher.board_size();

        for r in 0..board_size {
            for c in 0..board_size {
                for color in [Color::Black, Color::White] {
                    set.insert(hasher.stone(Position { row: r, col: c }, color).into());
                }
            }
        }

        assert_eq!(set.len(), board_size * board_size * 2);
    }
}
