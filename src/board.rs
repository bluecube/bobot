use crate::{
    bitboard::{Bitboard16, Position, format_ascii_helper},
    color::{Color, ColorMap},
    zobrist::{ZobristHash, ZobristHasher},
};
use rand::RngExt as _;

#[derive(Debug, PartialEq, Eq)]
pub enum MoveError {
    NonEmptyPosition,
    Suicide,
    Ko,
}

/// Describes potential problems with board. Mostly debug-only.
#[derive(Debug, PartialEq, Eq)]
pub enum BoardInvariantError {
    Overlap,
    StoneOutsideBoard,
    ZeroLiberties,
    WrongHash,
}

/// Represents a board state in game, doesn't consider side to play or ko history.
/// Invariants:
///   - Black and white stones never overlap.
///   - No stones outside of SIZE x SIZE.
///   - No zero liberty groups.
///   - Precomputed hash in `hash()` is always correct and would match `rehash()` result.
///
/// Compared to Bitboard16 Board also has stronger protections on inputs, so it can be used
/// from unverified user input.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Board {
    stones: ColorMap<Bitboard16>,

    hash: ZobristHash,
}

impl Board {
    pub const SIZE: usize = 13;

    pub fn new() -> Board {
        Board::default()
    }

    /// Constructs a new board from stone bitboards stored in a color map,
    /// calculates a correct hash.
    pub fn from_stones(
        stones: ColorMap<Bitboard16>,
        hasher: &ZobristHasher,
    ) -> Result<Board, BoardInvariantError> {
        let mut board = Board {
            stones,
            hash: ZobristHash::default(),
        };
        board.check_stone_invariants()?;
        board.hash = board.rehash(hasher)?;

        Ok(board)
    }

    pub fn hash(&self) -> ZobristHash {
        self.hash
    }

    /// Returns a set of empty positions on the board.
    pub fn empty_positions(&self) -> Bitboard16 {
        let occupied = self.stones[Color::Black] | self.stones[Color::White];
        Bitboard16::board_mask(Self::SIZE) & !occupied
        // TODO: don't rebuild mask?
    }

    /// Plays a single stone.
    /// Returns next board state if the move was valid, Err if was invalid.
    pub fn play_stone(
        &self,
        pos: Position,
        color: Color,
        hasher: &ZobristHasher,
    ) -> Result<Board, MoveError> {
        debug_assert_eq!(hasher.board_size(), Self::SIZE);

        if pos.row >= Self::SIZE || pos.col >= Self::SIZE {
            return Err(MoveError::NonEmptyPosition);
        }

        let empty = self.empty_positions();
        let hash = self.hash ^ hasher.stone(pos, color);
        let current_move = Bitboard16::single(pos);

        if (current_move & empty).is_empty() {
            return Err(MoveError::NonEmptyPosition);
        }

        let neighbors = current_move.dilate();
        let (own, opponents) = self.stones.clone().to_perspective(color);
        let own = own | current_move;
        let empty = empty & !current_move;

        let (opponents, empty, hash) = if (neighbors & opponents).is_empty() {
            // If the current move doesn't neighbor with any opponent stones it couldn't
            // have captured any, so we skip the whole capture process
            (opponents, empty, hash)
        } else {
            // Every live group neighbors with empty positions, flood filling from
            // these selects all opponents live stones.
            let live = (opponents & empty.dilate()).flood_fill(opponents);

            let captured = opponents & !live;

            let opponents_color = !color;
            let hash = captured
                .iter_positions()
                .map(|pos| hasher.stone(pos, opponents_color))
                .fold(hash, |a, b| a ^ b);

            (live, empty | captured, hash)
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
            hash,
        })
    }

    pub fn play_random_legal_move(
        &self,
        color: Color,
        hasher: &ZobristHasher,
        rng: &mut impl rand::Rng,
    ) -> Option<Board> {
        // TODO: Perf: This could probably be made faster, somehow
        //  - PDEP for selecting bits within lane
        //  - Don't recalculate popcnt in every loop

        debug_assert_eq!(hasher.board_size(), Self::SIZE);

        let mut candidates = self.empty_positions();

        while !candidates.is_empty() {
            let i = rng.random_range(0..candidates.popcnt());
            let pos = candidates.iter_positions().nth(i).unwrap();
            if let Ok(new_board) = self.play_stone(pos, color, hasher) {
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

    pub fn legal_moves(
        &self,
        color: Color,
        hasher: &ZobristHasher,
    ) -> impl Iterator<Item = (Position, Board)> {
        self.empty_positions()
            .iter_positions()
            .filter_map(move |pos| {
                self.play_stone(pos, color, hasher)
                    .ok()
                    .map(|board| (pos, board))
            })
    }

    /// Generates a viewable string represetnation of the board.
    /// Empty positions are drawn as `'.'`, black stones as `'x'`, white stones as `'o'`.
    /// If `compact` is `false`, fills all unused places in the 13x13 area by `.`, otherwise only
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

    /// Builds a bitboard from ascii representation, compatible with `Self::format_ascii`.
    /// Each line in text is a row in the bitboard, 'x' marks black, 'o' marks white,
    /// any other char marks an unset position.
    pub fn from_ascii(s: &str, hasher: &ZobristHasher) -> Result<Board, BoardInvariantError> {
        debug_assert_eq!(hasher.board_size(), Self::SIZE);

        let mut stones: ColorMap<Bitboard16> = ColorMap::default();

        for (row, line) in s.lines().enumerate() {
            for (col, c) in line.chars().enumerate() {
                if c != 'x' && c != 'o' {
                    continue;
                }

                if row >= Board::SIZE || col >= Board::SIZE {
                    return Err(BoardInvariantError::StoneOutsideBoard);
                }

                let pos = Position { row, col };
                if c == 'x' {
                    stones[Color::Black].set(pos, true);
                } else if c == 'o' {
                    stones[Color::White].set(pos, true);
                }
            }
        }

        Board::from_stones(stones, hasher)
    }

    fn rehash(&self, hasher: &ZobristHasher) -> Result<ZobristHash, BoardInvariantError> {
        debug_assert_eq!(hasher.board_size(), Self::SIZE);

        Ok(self
            .stones
            .as_ref()
            .into_iter()
            .map(|(color, bitboard)| {
                bitboard
                    .iter_positions()
                    .map(|pos| hasher.stone(pos, color))
                    .fold(ZobristHash::default(), |a, b| a ^ b)
            })
            .reduce(|a, b| a ^ b)
            .expect("ColorMap always has two elements"))
    }

    /// Checks invariants of the board (as documented in class docstring), except hash correctness.
    fn check_stone_invariants(&self) -> Result<(), BoardInvariantError> {
        if !(self.stones[Color::Black] & self.stones[Color::White]).is_empty() {
            return Err(BoardInvariantError::Overlap);
        }

        let out_of_board_mask = !Bitboard16::board_mask(Self::SIZE);
        let empty = self.empty_positions();
        for (_color, stones) in self.stones.as_ref() {
            if !(stones & out_of_board_mask).is_empty() {
                return Err(BoardInvariantError::StoneOutsideBoard);
            }
            if (stones & empty.dilate()).flood_fill(*stones) != *stones {
                return Err(BoardInvariantError::ZeroLiberties);
            }
        }

        Ok(())
    }

    /// Checks invariants of the board (as documented in class docstring).
    #[cfg(test)]
    fn check_invariants(&self, hasher: &ZobristHasher) -> Result<(), BoardInvariantError> {
        self.check_stone_invariants()?;

        if self.hash() != self.rehash(hasher).unwrap() {
            return Err(BoardInvariantError::WrongHash);
        }

        Ok(())
    }
}

#[cfg(test)]
#[derive(Debug)]
pub struct BoardAndHasher(Board, ZobristHasher);

#[cfg(test)]
impl proptest::arbitrary::Arbitrary for BoardAndHasher {
    type Parameters = ();
    type Strategy = proptest::strategy::BoxedStrategy<BoardAndHasher>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        use proptest::{
            bool::weighted,
            collection::vec,
            strategy::{Just, Strategy},
        };

        use crate::zobrist::zobrist_hasher_strategy;

        (
            zobrist_hasher_strategy(Just(Board::SIZE)),
            Bitboard16::arbitrary_with(Some(Board::SIZE)),
            0f64..=1f64,
        )
            .prop_flat_map(|(hasher, occupancy, white_ratio)| {
                let count = occupancy.popcnt();
                (
                    Just(hasher),
                    Just(occupancy),
                    vec(weighted(white_ratio), count..=count),
                )
            })
            .prop_map(|(hasher, occupancy, white_vec)| {
                assert_eq!(occupancy.popcnt(), white_vec.len());

                let white: Bitboard16 = occupancy
                    .iter_positions()
                    .zip(white_vec.iter())
                    .filter_map(|(pos, white)| white.then_some(pos))
                    .collect();
                let black = occupancy & !white;

                let empty = Bitboard16::board_mask(Board::SIZE) & !(black | white);

                let white = (white & empty.dilate()).flood_fill(white);
                let black = (black & empty.dilate()).flood_fill(black);

                BoardAndHasher(
                    Board::from_stones([black, white].into(), &hasher).unwrap(),
                    hasher,
                )
            })
            .boxed()
    }
}

#[cfg(test)]
mod test {
    use proptest::{prop_oneof, property_test};

    use super::*;

    fn transpose(board: Board, hasher: &ZobristHasher) -> Board {
        Board::from_stones(
            board.stones.map(|stones| {
                stones
                    .iter_positions()
                    .map(|Position { row, col }| Position { row: col, col: row })
                    .collect()
            }),
            hasher,
        )
        .unwrap()
    }

    fn color_swap(board: Board, hasher: &ZobristHasher) -> Board {
        let [black, white] = board.stones.into();
        Board::from_stones([white, black].into(), hasher).unwrap()
    }

    /// Tests mostly the Arbitrary machinery, not actual production code
    #[property_test]
    fn arbitrary_board_is_valid(BoardAndHasher(board, hasher): BoardAndHasher) {
        board.check_invariants(&hasher).unwrap();
    }

    #[property_test]
    fn from_stones_outside(
        #[strategy = prop_oneof![
                (Board::SIZE..16usize, 0usize..Board::SIZE),
                (0usize..Board::SIZE, Board::SIZE..16usize),
                (Board::SIZE..16usize, Board::SIZE..16usize)
            ]]
        coords: (usize, usize),
        color: Color,
    ) {
        let pos = Position::new(coords.0, coords.1);
        let hasher = ZobristHasher::new(Board::SIZE, &mut rand::rng());
        assert_eq!(
            Board::from_stones(
                ColorMap::from_perspective(color, Bitboard16::single(pos), Bitboard16::new()),
                &hasher
            ),
            Err(BoardInvariantError::StoneOutsideBoard)
        );
    }

    mod play_stone {
        use super::*;

        #[property_test]
        fn color_swap_produces_color_swapped_result(
            BoardAndHasher(board, hasher): BoardAndHasher,
            #[strategy = 0usize..Board::SIZE] row: usize,
            #[strategy = 0usize..Board::SIZE] col: usize,
            color: Color,
        ) {
            let pos = Position { row, col };
            assert_eq!(
                color_swap(board.clone(), &hasher).play_stone(pos, !color, &hasher),
                board
                    .play_stone(pos, color, &hasher)
                    .map(|board| color_swap(board, &hasher))
            );
        }

        #[property_test]
        fn transpose_produces_transposed_result(
            BoardAndHasher(board, hasher): BoardAndHasher,
            #[strategy = 0usize..Board::SIZE] row: usize,
            #[strategy = 0usize..Board::SIZE] col: usize,
            color: Color,
        ) {
            let pos = Position { row, col };
            assert_eq!(
                transpose(board.clone(), &hasher).play_stone(
                    Position::new(pos.col, pos.row),
                    color,
                    &hasher
                ),
                board
                    .play_stone(pos, color, &hasher)
                    .map(|board| transpose(board, &hasher))
            );
        }

        #[property_test]
        fn out_of_bounds(
            BoardAndHasher(board, hasher): BoardAndHasher,
            color: Color,
            #[strategy = prop_oneof![
                (Board::SIZE..256usize, 0usize..Board::SIZE),
                (0usize..Board::SIZE, Board::SIZE..256usize),
                (Board::SIZE..256usize, Board::SIZE..256usize)
            ]]
            coords: (usize, usize),
        ) {
            let pos = Position::new(coords.0, coords.1);
            assert_eq!(
                board.play_stone(pos, color, &hasher),
                Err(MoveError::NonEmptyPosition)
            );
        }

        mod legal_moves {
            use super::*;

            #[derive(Debug)]
            pub struct BoardHasherAndLegalMove(Board, ZobristHasher, Position, Color);

            impl proptest::arbitrary::Arbitrary for BoardHasherAndLegalMove {
                type Parameters = ();
                type Strategy = proptest::strategy::BoxedStrategy<BoardHasherAndLegalMove>;

                fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
                    use proptest::{
                        sample::select,
                        strategy::{Just, Strategy},
                    };

                    BoardAndHasher::arbitrary()
                        .prop_filter_map(
                            "some move must be available",
                            |BoardAndHasher(board, hasher)| {
                                let legal_moves: Vec<_> = [Color::Black, Color::White]
                                    .into_iter()
                                    .flat_map(|color| {
                                        board
                                            .legal_moves(color, &hasher)
                                            .map(move |(pos, _board)| (pos, color))
                                    })
                                    .collect();
                                if legal_moves.is_empty() {
                                    None
                                } else {
                                    Some((board, hasher, legal_moves))
                                }
                            },
                        )
                        .prop_flat_map(|(board, hasher, legal_moves)| {
                            (Just(board), Just(hasher), select(legal_moves))
                        })
                        .prop_map(|(board, hasher, (pos, color))| {
                            BoardHasherAndLegalMove(board, hasher, pos, color)
                        })
                        .boxed()
                }
            }

            #[property_test]
            fn keeps_invariants(
                BoardHasherAndLegalMove(board, hasher, pos, color): BoardHasherAndLegalMove,
            ) {
                board
                    .play_stone(pos, color, &hasher)
                    .unwrap()
                    .check_invariants(&hasher)
                    .unwrap();
            }

            #[property_test]
            fn adds_the_played_stone(
                BoardHasherAndLegalMove(board, hasher, pos, color): BoardHasherAndLegalMove,
            ) {
                assert_eq!(
                    board.play_stone(pos, color, &hasher).unwrap().stones[color],
                    board.stones[color] | Bitboard16::single(pos)
                );
            }

            #[property_test]
            fn does_not_add_enemy_stones(
                BoardHasherAndLegalMove(board, hasher, pos, color): BoardHasherAndLegalMove,
            ) {
                let opponents_stones = board.stones[!color];
                let board_after = board.play_stone(pos, color, &hasher).unwrap();
                let opponents_stones_after = board_after.stones[!color];

                assert!((opponents_stones_after & !opponents_stones).is_empty());
            }

            #[property_test]
            fn captured_groups_only_liberty_was_the_move(
                BoardHasherAndLegalMove(board, hasher, pos, color): BoardHasherAndLegalMove,
            ) {
                let move_bitmask = Bitboard16::single(pos);
                let opponents_stones = board.stones[!color];
                let board_after = board.play_stone(pos, color, &hasher).unwrap();

                let captured_stones = opponents_stones & !board_after.stones[!color];

                for group in captured_stones.iter_groups() {
                    assert_eq!(group.dilate() & board.empty_positions(), move_bitmask);
                }
            }
        }

        mod example {
            use super::*;

            #[test]
            fn empty_board() {
                let hasher = ZobristHasher::new(Board::SIZE, &mut rand::rng());
                let board = Board::new();

                assert_eq!(
                    board.play_stone(Position::new(0, 0), Color::Black, &hasher),
                    Ok(Board::from_ascii("x", &hasher).unwrap())
                );
            }

            #[test]
            fn next_to_opponent_no_capture() {
                let hasher = ZobristHasher::new(Board::SIZE, &mut rand::rng());
                let board = Board::from_ascii("o", &hasher).unwrap();

                assert_eq!(
                    board.play_stone(Position::new(1, 0), Color::Black, &hasher),
                    Ok(Board::from_ascii("o\nx", &hasher).unwrap())
                );
            }

            #[test]
            fn corner_capture() {
                let hasher = ZobristHasher::new(Board::SIZE, &mut rand::rng());
                let board = Board::from_ascii("ox", &hasher).unwrap();

                assert_eq!(
                    board.play_stone(Position::new(1, 0), Color::Black, &hasher),
                    Ok(Board::from_ascii(".x\nx", &hasher).unwrap())
                );
            }

            #[test]
            fn corner_capture2() {
                let hasher = ZobristHasher::new(Board::SIZE, &mut rand::rng());
                let board =
                    Board::from_ascii("\n\n\n\n\n\n\n\n\n\n\n\n...........xo", &hasher).unwrap();

                assert_eq!(
                    board.play_stone(Position::new(11, 12), Color::Black, &hasher),
                    Ok(Board::from_ascii(
                        "\n\n\n\n\n\n\n\n\n\n\n............x\n...........x.",
                        &hasher
                    )
                    .unwrap())
                );
            }

            #[test]
            fn playing_nonempty_position() {
                let hasher = ZobristHasher::new(Board::SIZE, &mut rand::rng());
                let board = Board::from_ascii("x", &hasher).unwrap();

                assert_eq!(
                    board.play_stone(Position::new(0, 0), Color::Black, &hasher),
                    Err(MoveError::NonEmptyPosition)
                );
            }

            #[test]
            fn suicide() {
                let hasher = ZobristHasher::new(Board::SIZE, &mut rand::rng());
                let board =
                    Board::from_ascii(".ooooo\noxxxxo\noxx.xo\noxxxxo\noooooo", &hasher).unwrap();
                assert_eq!(
                    board.play_stone(Position::new(2, 3), Color::Black, &hasher),
                    Err(MoveError::Suicide)
                );
            }

            #[test]
            fn not_suicide() {
                let hasher = ZobristHasher::new(Board::SIZE, &mut rand::rng());
                let board =
                    Board::from_ascii(".ooooo\noxxxxo\noxx.xo\noxxxxo\nooo.oo", &hasher).unwrap();
                assert_eq!(
                    board.play_stone(Position::new(2, 3), Color::Black, &hasher),
                    Ok(
                        Board::from_ascii(".ooooo\noxxxxo\noxxxxo\noxxxxo\nooo.oo", &hasher)
                            .unwrap()
                    )
                );
            }

            #[test]
            fn liberty_from_captured_stone() {
                let hasher = ZobristHasher::new(Board::SIZE, &mut rand::rng());
                let board = Board::from_ascii("oxo\n.xo\nooo", &hasher).unwrap();

                assert_eq!(
                    board.play_stone(Position::new(1, 0), Color::Black, &hasher),
                    Ok(Board::from_ascii(".xo\nxxo\nooo", &hasher).unwrap())
                );
            }

            #[test]
            fn capture_multiple() {
                let hasher = ZobristHasher::new(Board::SIZE, &mut rand::rng());
                let board = Board::from_ascii("oo.oox\nxxxxxx", &hasher).unwrap();

                assert_eq!(
                    board.play_stone(Position::new(0, 2), Color::Black, &hasher),
                    Ok(Board::from_ascii("..x..x\nxxxxxx", &hasher).unwrap())
                );
            }

            #[test]
            fn capture_multiple_liberty_from_captured() {
                let hasher = ZobristHasher::new(Board::SIZE, &mut rand::rng());
                let board = Board::from_ascii("oo.oox\nxxoxxx", &hasher).unwrap();

                assert_eq!(
                    board.play_stone(Position::new(0, 2), Color::Black, &hasher),
                    Ok(Board::from_ascii("..x..x\nxxoxxx", &hasher).unwrap())
                );
            }

            #[test]
            fn capture_one_of_two_groups() {
                let hasher = ZobristHasher::new(Board::SIZE, &mut rand::rng());
                let board = Board::from_ascii("oo.oo\nxxxxxx", &hasher).unwrap();

                assert_eq!(
                    board.play_stone(Position::new(0, 2), Color::Black, &hasher),
                    Ok(Board::from_ascii("..xoo\nxxxxxx", &hasher).unwrap())
                );
            }
        }
    }

    mod legal_move {
        use std::collections::HashSet;

        use super::*;

        #[property_test]
        fn are_legal_and_boards_match(BoardAndHasher(board, hasher): BoardAndHasher, color: Color) {
            for (pos, expected_result) in board.legal_moves(color, &hasher) {
                assert_eq!(board.play_stone(pos, color, &hasher), Ok(expected_result));
            }
        }

        #[property_test]
        fn no_other_moves_are_legal(BoardAndHasher(board, hasher): BoardAndHasher, color: Color) {
            let legal_moves: HashSet<_> = board
                .legal_moves(color, &hasher)
                .map(|(pos, _board)| pos)
                .collect();

            for row in 0..Board::SIZE {
                for col in 0..Board::SIZE {
                    let pos = Position { row, col };

                    if legal_moves.contains(&pos) {
                        continue;
                    }

                    assert!(board.play_stone(pos, color, &hasher).is_err());
                }
            }
        }

        #[property_test]
        fn move_errors(
            BoardAndHasher(board, hasher): BoardAndHasher,
            #[strategy = 0usize..Board::SIZE] row: usize,
            #[strategy = 0usize..Board::SIZE] col: usize,
            color: Color,
        ) {
            let pos = Position { row, col };
            let move_mask = Bitboard16::single(pos);
            let empty = board.empty_positions();

            match board.play_stone(pos, color, &hasher) {
                Ok(_) => assert!(empty.get(pos)),
                Err(MoveError::NonEmptyPosition) => {
                    assert!(!empty.get(pos))
                }
                Err(MoveError::Suicide) => {
                    assert!(empty.get(pos));
                    assert_eq!(move_mask.dilate() & empty, move_mask);
                }
                Err(MoveError::Ko) => unimplemented!("play_stone never returns ko error"),
            };
        }

        mod example {
            use super::*;

            #[test]
            fn full_board_one_move() {
                let hasher = ZobristHasher::new(Board::SIZE, &mut rand::rng());
                let board = Board::from_ascii(
                    "xxxxxxxxxxxxx\n\
                     xxxxxxxxxxxxx\n\
                     xxxxxxxxxxxxx\n\
                     xxxxxxxxxxxxx\n\
                     xxxxxxxxxxxxx\n\
                     xxxxxxxxxxxxx\n\
                     xxxxxx.oooooo\n\
                     ooooooooooooo\n\
                     ooooooooooooo\n\
                     ooooooooooooo\n\
                     ooooooooooooo\n\
                     ooooooooooooo\n\
                     ooooooooooooo",
                    &hasher,
                )
                .unwrap();

                let black_legal_moves: Vec<_> = board
                    .legal_moves(Color::Black, &hasher)
                    .map(|(pos, _board)| pos)
                    .collect();
                let white_legal_moves: Vec<_> = board
                    .legal_moves(Color::White, &hasher)
                    .map(|(pos, _board)| pos)
                    .collect();

                assert_eq!(black_legal_moves, vec![Position::new(6, 6)]);
                assert_eq!(white_legal_moves, vec![Position::new(6, 6)]);
            }

            #[test]
            fn full_board_no_moves_for_black() {
                let hasher = ZobristHasher::new(Board::SIZE, &mut rand::rng());
                let board = Board::from_ascii(
                    "xxxxxxxxxxxxx\n\
                     xxxxxxxxxxxxx\n\
                     xxxxxxxxxxxxx\n\
                     xxxxxxxxxxxxx\n\
                     xxxxxxxxxxxxx\n\
                     xxxxxxxxxxxxx\n\
                     xxxxxx.xxxxxx\n\
                     xxxxxxxxxxxxx\n\
                     xxxxxxxxxxxxx\n\
                     xxxxxxxxxxxxx\n\
                     xxxxxxxxxxxxx\n\
                     xxxxxxxxxxxxx\n\
                     xxxxxxxxxxxxx",
                    &hasher,
                )
                .unwrap();

                assert_eq!(board.legal_moves(Color::Black, &hasher).count(), 0);
                assert_eq!(board.legal_moves(Color::White, &hasher).count(), 1);
            }
        }
    }

    mod score {
        use proptest::prop_assume;

        use crate::zobrist::zobrist_hasher_strategy;

        use super::*;

        #[test]
        fn empty() {
            assert_eq!(Board::new().score(), [0, 0].into());
        }

        #[property_test]
        fn stone_count(BoardAndHasher(board, _hasher): BoardAndHasher) {
            for (color, score) in board.score() {
                assert!(score >= board.stones[color].popcnt());
            }
        }

        #[property_test]
        fn limited_by_board_size(BoardAndHasher(board, _hasher): BoardAndHasher) {
            let [black, white] = board.score().into();
            assert!(black + white <= Board::SIZE * Board::SIZE);
        }

        #[property_test]
        fn color_swap_swaps(BoardAndHasher(board, hasher): BoardAndHasher) {
            let [black, white] = board.score().into();
            let swapped_board = color_swap(board, &hasher);
            assert_eq!(swapped_board.score(), [white, black].into());
        }

        #[property_test]
        fn transpose_no_change(BoardAndHasher(board, hasher): BoardAndHasher) {
            assert_eq!(board.score(), transpose(board, &hasher).score());
        }

        #[property_test]
        fn single_color_board(
            #[strategy = Bitboard16::arbitrary_with(Some(Board::SIZE))] stones: Bitboard16,
            color: Color,
            #[strategy = zobrist_hasher_strategy(Board::SIZE..=Board::SIZE)] hasher: ZobristHasher,
        ) {
            prop_assume!(!stones.is_empty());
            prop_assume!(stones != Bitboard16::board_mask(Board::SIZE));

            let board = Board::from_stones(
                ColorMap::from_perspective(color, stones, Bitboard16::new()),
                &hasher,
            )
            .unwrap();

            assert_eq!(
                board.score().to_perspective(color),
                (Board::SIZE * Board::SIZE, 0)
            );
        }

        #[test]
        fn example() {
            let hasher = ZobristHasher::new(Board::SIZE, &mut rand::rng());
            let board =
                Board::from_ascii("\n\n\nxxxxxxxxxxxxx\n\n\n\nooooooooooooo", &hasher).unwrap();

            assert_eq!(board.score(), [4 * 13, 6 * 13].into());
        }
    }

    mod random_legal_move {
        use rand::SeedableRng;

        use super::*;

        #[property_test]
        fn some_if_legal_moves_exist(
            BoardAndHasher(board, hasher): BoardAndHasher,
            color: Color,
            seed: u64,
        ) {
            let mut rng = rand::rngs::SmallRng::seed_from_u64(seed);
            assert_eq!(
                board
                    .play_random_legal_move(color, &hasher, &mut rng)
                    .is_some(),
                board.legal_moves(color, &hasher).count() > 0
            );
        }

        #[property_test]
        fn all_legal_moves_appear(
            BoardAndHasher(board, hasher): BoardAndHasher,
            color: Color,
            seed: u64,
        ) {
            let mut rng = rand::rngs::SmallRng::seed_from_u64(seed);
            let mut remaining_moves: Vec<_> = board
                .legal_moves(color, &hasher)
                .map(|(_pos, board)| board)
                .collect();
            let number_of_tries = remaining_moves.len() * 100;

            for _i in 0..number_of_tries {
                if remaining_moves.is_empty() {
                    break;
                }

                let random_board = board
                    .play_random_legal_move(color, &hasher, &mut rng)
                    .unwrap();

                if let Some(index) = remaining_moves.iter().position(|b| b == &random_board) {
                    remaining_moves.swap_remove(index);
                }
            }

            assert!(remaining_moves.is_empty());
        }
    }

    mod ascii {
        use super::*;

        #[property_test]
        fn round_trip(BoardAndHasher(board, hasher): BoardAndHasher, compact: bool) {
            let s = board.format_ascii(compact);
            let unpacked = Board::from_ascii(&s, &hasher).unwrap();

            assert_eq!(unpacked, board);
        }

        #[property_test]
        fn bitboard_as_board(BoardAndHasher(board, hasher): BoardAndHasher) {
            // Bitboard doesn't need to respect Board invariants, so we have to start with a Board.
            let black_stones = board.stones[Color::Black];
            let s = black_stones.format_ascii(0);

            let board = Board::from_ascii(&s, &hasher).unwrap();

            assert_eq!(board.stones[Color::Black], black_stones);
        }

        #[test]
        fn empty_positions_outside_board() {
            let hasher = ZobristHasher::new(Board::SIZE, &mut rand::rng());

            assert!(
                Board::from_ascii(
                    "\n...x\n\n\n\n\n......................................................\n\
                    \n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n",
                    &hasher
                )
                .is_ok()
            );
        }

        #[test]
        fn too_long_row() {
            let hasher = ZobristHasher::new(Board::SIZE, &mut rand::rng());

            assert_eq!(
                Board::from_ascii("\n..........................x", &hasher),
                Err(BoardInvariantError::StoneOutsideBoard)
            );
        }

        #[test]
        fn too_many_rows() {
            let hasher = ZobristHasher::new(Board::SIZE, &mut rand::rng());

            assert_eq!(
                Board::from_ascii(
                    "\n...x\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\nx",
                    &hasher
                ),
                Err(BoardInvariantError::StoneOutsideBoard)
            );
        }
    }
}
