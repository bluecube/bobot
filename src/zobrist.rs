use std::{
    fmt::Debug,
    hash::Hash,
    ops::{BitXor, BitXorAssign},
};

use crate::{Color, Position, util::Splitmix64};

/// Maximum position range that can be hashed.
/// The value is a futureproofing to support
/// larger boards than just 13x13 or 19x19.
/// It costs almost nothing...
const MAX_SIZE: usize = 32;

const HASHES_SEED: u64 = 0xb0b0_4a54_5eed;
static HASHES: [ZobristHash; MAX_SIZE * MAX_SIZE * 2] = {
    let mut hashes = [ZobristHash(0); _];
    let mut rng = Splitmix64::with_seed(HASHES_SEED);
    let mut i = 0;

    while i < (MAX_SIZE * MAX_SIZE * 2) {
        hashes[i] = ZobristHash(rng.next());
        i += 1;
    }

    hashes
};

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ZobristHash(u64);

impl ZobristHash {
    pub const fn stone(pos: Position, color: Color) -> ZobristHash {
        debug_assert!(pos.row < MAX_SIZE);
        debug_assert!(pos.col < MAX_SIZE);
        HASHES[(color as usize) + 2 * (pos.col + MAX_SIZE * pos.row)]
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

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn all_unique() {
        let mut set: HashSet<u64> = HashSet::new();

        for r in 0..MAX_SIZE {
            for c in 0..MAX_SIZE {
                for color in [Color::Black, Color::White] {
                    set.insert(ZobristHash::stone(Position { row: r, col: c }, color).into());
                }
            }
        }

        assert_eq!(set.len(), MAX_SIZE * MAX_SIZE * 2);
    }
}
