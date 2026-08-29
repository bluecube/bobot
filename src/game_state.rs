use std::collections::HashSet;

use serde::Deserialize;

use crate::{Board, Color, Position, ZobristHash, board::PlayStoneError};

#[derive(Debug, Clone)]
pub struct GameState {
    pub board: Board,
    pub to_play: Color,
    pub superko_history: HashSet<ZobristHash>,
    pub consecutive_passes: u8,
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
