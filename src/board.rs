use crate::{
    bitboard::{Bitboard16, Position, format_ascii_helper},
    color::{Color, ColorMap},
};
use rand::RngExt as _;

pub enum MoveError {
    NonEmptyPosition,
    Suicide,
    Ko,
}

#[derive(Default, Clone, PartialEq, Eq)]
pub struct Board {
    stones: ColorMap<Bitboard16>,
}

impl Board {
    pub const SIZE: usize = 13;

    pub fn new() -> Board {
        Board::default()
    }

    /// Returns a set of empty positions on the board.
    pub fn empty_positions(&self) -> Bitboard16 {
        let occupied = self.stones[Color::Black] | self.stones[Color::White];
        Bitboard16::board_mask(Self::SIZE) & !occupied
        // TODO: don't rebuild mask?
    }

    /// Plays a single stone.
    /// Returns next board state if the move was valid, Err if was invalid.
    /// Panics when playing outside 16x16.
    pub fn play_stone(&self, pos: Position, color: Color) -> Result<Board, MoveError> {
        let empty = self.empty_positions();
        let current_move = Bitboard16::single(pos);

        if (current_move & empty).is_empty() {
            return Err(MoveError::NonEmptyPosition);
        }

        let neighbors = current_move.dilate();
        let (own, opponents) = self.stones.clone().to_perspective(color);
        let own = own | current_move;
        let empty = empty & !current_move;

        let (opponents, empty) = if (neighbors & opponents).is_empty() {
            // If the current move doesn't neighbor with any opponent stones it couldn't
            // have captured any, so we skip the whole capture process
            (opponents, empty)
        } else {
            // Every live group neighbors with empty positions, flood filling from
            // these selects all opponents live stones.
            let live = (opponents & empty.dilate()).flood_fill(opponents);

            (live, empty | opponents & !live)
        };

        if (neighbors & empty).is_empty() {
            // Suicide can only happens if the newly placed stone is not neighboring an empty position
            // This is a quick check without much extra computation.

            let group = current_move.flood_fill(own);
            if (group.dilate() & empty).is_empty() {
                return Err(MoveError::Suicide);
            }
        }

        Ok(Board {
            stones: ColorMap::from_perspective(color, own, opponents),
        })
    }

    pub fn play_random_legal_move(&self, color: Color, rng: &mut impl rand::Rng) -> Option<Board> {
        // TODO: Perf: This could probably be made faster, somehow
        //  - PDEP for selecting bits within lane
        //  - Don't recalculate popcnt in every loop

        let mut candidates = self.empty_positions();

        while !candidates.is_empty() {
            let i = rng.random_range(0..candidates.popcnt());
            let pos = candidates.iter_positions().nth(i).unwrap();
            if let Ok(new_board) = self.play_stone(pos, color) {
                return Some(new_board);
            } else {
                candidates.set(pos, false);
            }
        }

        None
    }

    pub fn score(&self) -> ColorMap<usize> {
        let neighbors = self.stones.map_ref(|stones| stones.dilate());
        let mut score = self.stones.map_ref(|stones| stones.popcnt());
        for empty_group in self.empty_positions().iter_groups() {
            let adjanced_to = neighbors.map_ref(|neighbors| !(empty_group & neighbors).is_empty());
            if adjanced_to[Color::Black] && adjanced_to[Color::White] {
                // Neutral
                continue;
            }

            if adjanced_to[Color::Black] {
                score[Color::Black] += empty_group.popcnt();
            }
            if adjanced_to[Color::White] {
                score[Color::White] += empty_group.popcnt();
            }
        }

        score
    }

    pub fn legal_moves(&self, color: Color) -> impl Iterator<Item = (Position, Board)> {
        self.empty_positions()
            .iter_positions()
            .filter_map(move |pos| self.play_stone(pos, color).ok().map(|board| (pos, board)))
    }

    /// Generates a viewable string represetnation of the board.
    /// Empty positions are drawn as `'.'`, black stones as `'x'`, white stones as `'o'`.
    /// If `compact` is `true`, fills all unused places in the 13x13 area by `.`, otherwise only
    /// positions followed by non-empty are printed.
    pub fn format_ascii(&self, compact: bool) -> String {
        format_ascii_helper(
            self.stones
                .map_ref(|b| b.iter_rows())
                .zip()
                .map(ColorMap::zip),
            |x| match x.to_array() {
                [true, true] => unreachable!(),
                [true, false] => Some('x'),
                [false, true] => Some('o'),
                [false, false] => None,
            },
            if compact { 0 } else { Self::SIZE },
        )
    }
}
