use rand::Rng;

use crate::{Bitboard16, board::Board, color::Color, zobrist::ZobristHash};

pub struct RandomRolloutResult {
    pub termination_reason: TerminationReason,

    pub board: Board,
}

pub enum TerminationReason {
    TwoPasses,
    MoveLimitReached { to_play: Color },
}

/// Plays a random rollout of at most `move_limit` random moves.
/// Never plays into its own single-position eyes and never passes unless there
/// are other moves to play.
/// Passes consume move limit, `MoveLimitReached` doesn't guarantee that a larger
/// limit would put more stones on the board.
pub fn play_random_rollout(
    board: Board,
    to_play: Color,
    consecutive_passes: u8,
    ko_prev_hash: ZobristHash,
    move_limit: usize,
    rng: &mut impl Rng,
) -> RandomRolloutResult {
    let mut current_board = board;
    let mut current_to_play = to_play;
    let mut consecutive_passes = consecutive_passes;
    let mut ko_prev_hash = ko_prev_hash;

    for _ in 0..move_limit {
        if consecutive_passes >= 2 {
            break;
        }
        if let Some(new_board) = random_move(&current_board, current_to_play, ko_prev_hash, rng) {
            ko_prev_hash = current_board.hash();
            current_board = new_board;
            consecutive_passes = 0;
        } else {
            consecutive_passes += 1;
        }

        current_to_play = !current_to_play;
    }

    RandomRolloutResult {
        termination_reason: if consecutive_passes >= 2 {
            TerminationReason::TwoPasses
        } else {
            TerminationReason::MoveLimitReached {
                to_play: current_to_play,
            }
        },
        board: current_board,
    }
}

fn random_move(
    board: &Board,
    color: Color,
    ko_prev_hash: ZobristHash,
    rng: &mut impl Rng,
) -> Option<Board> {
    // TODO: This is basically a duplicate of Board::play_random_legal_stone,
    // either we should unify them, remove the one from Board, or at least keep
    // performance improvements in sync.

    let mut candidates = candidate_positions(board, color);

    while let Some(pos) = candidates.random_position(rng) {
        if let Ok(new_board) = board.play_stone(pos, color)
            && new_board.hash() != ko_prev_hash
        {
            return Some(new_board);
        }

        candidates.set(pos, false);
    }

    None
}

/// Returns a bitboard of empty positions that are not `color`'s single position eyes.
fn candidate_positions(board: &Board, color: Color) -> Bitboard16 {
    let opponents = board.stones[!color];
    let empty = board.empty_positions();
    let opponents_or_empty = empty | opponents;

    let not_surrounded_by_own = opponents_or_empty.orthogonal_neighbors();

    let diagonal_opponents = {
        // [lr][ud] are masks of positions that have opponent's stone as one of
        // the diagonal neighbors (lu = neighbor left up from current position)
        let lu = opponents.shift_right(1).shift_down(1);
        let ld = opponents.shift_right(1).shift_up(1);
        let ru = opponents.shift_left(1).shift_down(1);
        let rd = opponents.shift_left(1).shift_up(1);

        let two_or_more_opponent_diagonal =
            lu & ld | lu & ru | lu & rd | ld & ru | ld & rd | ru & rd;
        let one_or_more_opponent_diagonal = lu | ld | ru | rd;

        one_or_more_opponent_diagonal & Bitboard16::first_line_mask(Board::SIZE)
            | two_or_more_opponent_diagonal
    };

    let not_eyes = not_surrounded_by_own | diagonal_opponents;

    empty & not_eyes
}

#[cfg(test)]
mod test {
    use proptest::property_test;

    use super::*;

    mod candidate_positions {
        use crate::Position;

        use super::*;

        #[test]
        fn empty_board_returns_board_mask() {
            assert_eq!(
                candidate_positions(&Board::new(), Color::Black),
                Bitboard16::board_mask(Board::SIZE)
            );
        }

        #[property_test]
        fn subset_of_empty(board: Board, color: Color) {
            let empty = board.empty_positions();
            let candidates = candidate_positions(&board, color);

            assert_eq!(candidates & empty, candidates);
        }

        /// Slow, but readable reimplementation of `candidate_positions()`.
        fn candidates_reference(board: &Board, color: Color) -> Bitboard16 {
            let mut candidates = board.empty_positions();

            for pos in candidates.iter_positions() {
                // Remove this position if it is not surrounded by own stones on all sides
                let surrounded_by_own = !((pos.row > 0
                    && !board.stones[color].get(Position {
                        row: pos.row - 1,
                        col: pos.col,
                    }))
                    || (pos.row < (Board::SIZE - 1)
                        && !board.stones[color].get(Position {
                            row: pos.row + 1,
                            col: pos.col,
                        }))
                    || (pos.col > 0
                        && !board.stones[color].get(Position {
                            row: pos.row,
                            col: pos.col - 1,
                        }))
                    || (pos.col < (Board::SIZE - 1)
                        && !board.stones[color].get(Position {
                            row: pos.row,
                            col: pos.col + 1,
                        })));

                let mut diagonal_opponents = 0;
                if pos.row > 0 {
                    if pos.col > 0
                        && board.stones[!color].get(Position {
                            row: pos.row - 1,
                            col: pos.col - 1,
                        })
                    {
                        diagonal_opponents += 1;
                    }

                    if pos.col < (Board::SIZE - 1)
                        && board.stones[!color].get(Position {
                            row: pos.row - 1,
                            col: pos.col + 1,
                        })
                    {
                        diagonal_opponents += 1;
                    }
                }
                if pos.row < (Board::SIZE - 1) {
                    if pos.col > 0
                        && board.stones[!color].get(Position {
                            row: pos.row + 1,
                            col: pos.col - 1,
                        })
                    {
                        diagonal_opponents += 1;
                    }

                    if pos.col < (Board::SIZE - 1)
                        && board.stones[!color].get(Position {
                            row: pos.row + 1,
                            col: pos.col + 1,
                        })
                    {
                        diagonal_opponents += 1;
                    }
                }

                let is_first_line = pos.row == 0
                    || pos.col == 0
                    || pos.row == (Board::SIZE - 1)
                    || pos.col == (Board::SIZE - 1);

                let is_eye = surrounded_by_own
                    && !(is_first_line && diagonal_opponents >= 1
                        || (!is_first_line && diagonal_opponents >= 2));

                if is_eye {
                    candidates.set(pos, false);
                    continue;
                }
            }

            candidates
        }

        #[property_test]
        fn reference(board: Board, color: Color) {
            assert_eq!(
                candidate_positions(&board, color),
                candidates_reference(&board, color)
            );
        }

        #[property_test]
        fn colors_symmetry(board: Board, color: Color) {
            let original = candidate_positions(&board, color);
            let flipped_board = Board::from_stones(crate::color::ColorMap {
                black: board.stones.white,
                white: board.stones.black,
            });
            // assert_eq!()
        }

        mod example {
            // Examples mostly taken from sensei's library:
            // https://senseis.xmp.net/?RecognizingAnEye

            use super::*;

            macro_rules! eye_tests {
                ($(($name:ident, $board:expr, $color:expr, $row:expr, $col:expr))*) => {
                    mod eye {
                        use super::*;
                        $(
                        #[test]
                        fn $name() {
                            let board = Board::from_ascii($board).unwrap();

                            let candidates = candidate_positions(&board, $color);

                            assert!(!candidates.get(Position {
                                row: $row,
                                col: $col
                            }));
                        }
                        )*
                    }
                };
            }

            eye_tests! {
                (corner, ".x\nx", Color::Black, 0, 0)
                (side, ".....o.o\n.....oo", Color::White, 0, 6)
                (center, "\n\n\n...x\n..x.x\n...x", Color::Black, 4, 3)
                (center_with_diagonal_opponent, "\n\n\n...x\n..x.x\n..ox", Color::Black, 4, 3)
            }

            macro_rules! not_eye_tests {
                ($(($name:ident, $board:expr, $color:expr))*) => {
                    mod not_an_eye {
                        use super::*;
                        $(
                        #[test]
                        fn $name() {
                            let board = Board::from_ascii($board).unwrap();

                            let candidates = candidate_positions(&board, $color);

                            assert_eq!(candidates, board.empty_positions());
                        }
                        )*
                    }
                };
            }

            not_eye_tests! {
                (corner_false_eye, ".x\nxo", Color::Black)
                (side_false_eye, ".....o.o\n.....xo", Color::White)
                (center_false_eye, "\n\n\n..ox\n..x.x\n..ox", Color::Black)

                (side_stone_missing, ".....o\n......o", Color::White)
                (center, "\n\n\n...x\n....x\n...x", Color::Black)
            }
        }
    }
}
