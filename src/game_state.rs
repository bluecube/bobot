use std::{
    collections::HashSet,
    fmt::Display,
    ops::{Add, Sub},
};

use serde::Deserialize;

use crate::{Board, Color, Position, ZobristHash, board::PlayStoneError};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct HalfPoints(i32);

impl HalfPoints {
    pub const ZERO: HalfPoints = HalfPoints(0);

    pub fn new_round(v: f32) -> HalfPoints {
        assert!(!v.is_nan());
        HalfPoints((v * 2.0).round() as i32)
    }

    pub fn signum(&self) -> HalfPoints {
        HalfPoints(2 * self.0.signum())
    }
}

impl From<HalfPoints> for f32 {
    fn from(value: HalfPoints) -> Self {
        value.0 as f32 / 2.0
    }
}

impl From<i32> for HalfPoints {
    fn from(value: i32) -> Self {
        HalfPoints(value * 2)
    }
}

impl Display for HalfPoints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f32::from(*self).fmt(f)
    }
}

impl Add for HalfPoints {
    type Output = HalfPoints;

    fn add(self, rhs: Self) -> Self::Output {
        HalfPoints(self.0 + rhs.0)
    }
}

impl Sub for HalfPoints {
    type Output = HalfPoints;

    fn sub(self, rhs: Self) -> Self::Output {
        HalfPoints(self.0 - rhs.0)
    }
}

#[derive(Debug, Clone)]
pub struct GameState {
    pub board: Board,
    pub to_play: Color,
    pub superko_history: HashSet<ZobristHash>,
    pub consecutive_passes: u8,
    pub komi: HalfPoints,
}

#[derive(Debug, Clone, Deserialize)]
pub enum Move {
    Stone(Position),
    Pass,
}

pub enum MoveError {
    NonEmptyPosition,
    Suicide,
    Ko,
    GameFinished,
}

impl From<PlayStoneError> for MoveError {
    fn from(value: PlayStoneError) -> Self {
        match value {
            PlayStoneError::NonEmptyPosition => MoveError::NonEmptyPosition,
            PlayStoneError::Suicide => MoveError::Suicide,
        }
    }
}

impl GameState {
    pub fn new() -> Self {
        GameState {
            board: Board::new(),
            to_play: Color::Black,
            superko_history: HashSet::new(), // TODO: Perf the default hasher is needlessly slow
            consecutive_passes: 0,
            komi: HalfPoints::from(0i32), // TODO: Something reasonable;
        }
    }

    pub fn is_finished(&self) -> bool {
        self.consecutive_passes >= 2
    }

    pub fn play_move(&mut self, m: Move) -> Result<(), MoveError> {
        if self.is_finished() {
            return Err(MoveError::GameFinished);
        }
        match m {
            Move::Stone(pos) => {
                let new_board = self.board.play_stone(pos, self.to_play)?;
                self.superko_history.insert(self.board.hash());
                self.board = new_board;
                self.consecutive_passes = 0;
            }
            Move::Pass => {
                self.consecutive_passes += 1;
            }
        }

        self.to_play = !self.to_play;

        Ok(())
    }

    pub fn play_random_legal_move(&mut self, rng: &mut impl rand::Rng) -> Result<(), MoveError> {
        if self.is_finished() {
            return Err(MoveError::GameFinished);
        }
        if let Some(new_board) = self.board.play_random_legal_stone(self.to_play, rng) {
            self.superko_history.insert(self.board.hash());
            self.board = new_board;
            self.consecutive_passes = 0;
        } else {
            self.consecutive_passes += 1;
        }

        self.to_play = !self.to_play;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    mod half_points {
        use proptest::property_test;

        use crate::game_state::HalfPoints;

        #[property_test]
        fn from_i32_f32_round_trip(#[strategy = -100i32..100i32] v: i32) {
            assert_eq!(f32::from(HalfPoints::from(v)) as i32, v);
        }

        #[test]
        fn zero() {
            assert_eq!(HalfPoints::from(0), HalfPoints::ZERO);
        }

        #[test]
        fn add() {
            assert_eq!(
                HalfPoints::from(1) + HalfPoints::from(2),
                HalfPoints::from(3)
            );
        }

        #[test]
        fn sub() {
            assert_eq!(
                HalfPoints::from(8) - HalfPoints::from(15),
                HalfPoints::from(-7)
            );
        }

        #[test]
        fn rounding() {
            assert_eq!(HalfPoints::new_round(0.1), HalfPoints::ZERO)
        }

        #[test]
        fn signum() {
            assert_eq!(HalfPoints::from(-10).signum(), HalfPoints::from(-1));
            assert_eq!(HalfPoints::from(3).signum(), HalfPoints::from(1));
            assert_eq!(HalfPoints::ZERO.signum(), HalfPoints::ZERO);
        }
    }
}
