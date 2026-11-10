pub mod random_rollout;

use std::range::Range;

use rand::Rng;

use crate::{
    Board, Color, ZobristHash,
    game_state::{GameState, HalfPoints, Move},
    solver::random_rollout::play_random_rollout,
};

pub struct Solver {
    game_state: GameState,

    nodes: Vec<Node>, // Node 0 is always the current state
}

struct Node {
    // TODO: Perf: Split state from Node, so that we don't have to load the board state
    // on every node selection. Profile first, though.
    state: Board,
    mov: Move,

    consecutive_passes: u8,

    parent_idx: usize,

    prior_probability: f32,

    visit_count: u32,

    // value_sum is from the point of view of the Node's parent's color to play
    value_sum: f32,

    child_indices: Option<Range<usize>>, // TODO: Perf: Use custom option instead of this to get rid of one word.
}

impl Solver {
    pub fn new(game: GameState) -> Solver {
        let nodes = vec![Node {
            state: game.board.clone(),
            mov: Move::Pass, // Has no meaning in root node

            consecutive_passes: game.consecutive_passes,

            parent_idx: 0, // Doesn't mean anything for root

            prior_probability: 0.5, // Doesn't mean anything for root

            visit_count: 0u32,
            value_sum: 0f32,

            child_indices: None, // Not expanded yet
        }];

        Solver {
            game_state: game,
            nodes,
        }
    }

    /// Does one step of the MCTS algorithm.
    /// Expands one new node, evaluates it and back propagates.
    pub fn one_step(&mut self) {
        let mut current_idx = 0;
        let mut current_parent_score = 0f32;
        let mut current_to_play = self.game_state.to_play;
        let mut local_superko_history = Vec::new();

        loop {
            self.nodes[current_idx].visit_count += 1;
            let current_node = &self.nodes[current_idx];
            local_superko_history.push(current_node.state.hash());
            let Some(child_indices) = current_node.child_indices else {
                break;
            };
            current_idx = self.select(
                child_indices,
                current_node.visit_count,
                current_parent_score,
            );
            current_to_play = !current_to_play;

            current_parent_score = -self.nodes[current_idx]
                .score()
                .unwrap_or(current_parent_score);
        }

        let mut value = if self.nodes[current_idx].is_terminal() {
            // This needs to be from the node parent's side to play perspective, not its own,
            // so we're inverting
            -score_value(
                &self.nodes[current_idx].state,
                current_to_play,
                self.game_state.komi,
            )
        } else {
            self.expand_node(
                current_idx,
                current_to_play,
                &local_superko_history,
                &mut rand::rng(),
            )
        };

        while current_idx != 0 {
            self.nodes[current_idx].value_sum += value;
            value = -value;
            current_idx = self.nodes[current_idx].parent_idx;
        }
        self.nodes[0].value_sum += value;
    }

    /// Performs one batched step.
    /// Expands n new nodes, evaluates them and back propagates.
    pub fn ones_step_batched(&mut self, n: usize) {
        todo!("Figure out how will batching work exactly")
    }

    /// Selects a move to be played next.
    /// This is the most visited child of root in the MCTS tree (TODO: Is this correct? Claude seems to think so).
    pub fn best_move(&mut self) -> Option<Move> {
        let child_indices = self.nodes[0].child_indices?;

        let mut best = Move::Pass; // Just a placeholder, will be overwritten.
        let mut best_visits = 0;
        for i in child_indices.iter() {
            let node = &self.nodes[i];
            if node.visit_count > best_visits {
                best = node.mov.clone();
                best_visits = node.visit_count;
            }
        }

        Some(best)
    }

    /// Selects a child of node_idx for descending
    /// `parent_score` is evaluated from the parent node's side to play perspective.
    fn select(
        &self,
        child_indices: Range<usize>,
        parent_visit_count: u32,
        parent_score: f32,
    ) -> usize {
        debug_assert!(!child_indices.is_empty());

        let mut best = 0;
        let mut best_score = -f32::INFINITY;
        for i in child_indices.iter() {
            let score = self.nodes[i].puct_score(parent_visit_count, parent_score);
            if score > best_score {
                best = i;
                best_score = score;
            }
        }

        best
    }

    fn expand_node(
        &mut self,
        node_idx: usize,
        to_play: Color,
        local_superko_history: &Vec<ZobristHash>,
        rng: &mut impl Rng,
    ) -> f32 {
        let node = &self.nodes[node_idx];

        assert!(node.child_indices.is_none());

        if node.is_terminal() {
            return -score_value(&node.state, to_play, self.game_state.komi);
        }

        let first_child = self.nodes.len();
        let board = node.state.clone();

        let nodes_len_before = self.nodes.len();

        self.nodes.extend(
            board
                .legal_moves(to_play)
                .filter(|(_position, board)| {
                    !(local_superko_history.contains(&board.hash())
                        || self.game_state.superko_history.contains(&board.hash()))
                })
                .map(|(position, board)| Node {
                    state: board,
                    mov: Move::Stone(position),
                    consecutive_passes: 0,
                    parent_idx: node_idx,
                    prior_probability: 0.0, // will be overwritten, TODO: Should be filled in by the NN
                    visit_count: 0,
                    value_sum: 0f32,
                    child_indices: None,
                }),
        );

        // Pass action
        self.nodes.push(Node {
            state: board.clone(),
            mov: Move::Pass,
            consecutive_passes: self.nodes[node_idx].consecutive_passes + 1,
            parent_idx: node_idx,
            prior_probability: 0.0, // will be overwritten, TODO: Should be filled in by the NN
            visit_count: 0,
            value_sum: 0f32,
            child_indices: None,
        });

        // Temporary, setting uniform prior probability for all nodes
        let nodes_added = self.nodes.len() - nodes_len_before;
        let node_prior_probability = 1.0 / (nodes_added as f32);
        for node in &mut self.nodes[nodes_len_before..] {
            node.prior_probability = node_prior_probability
        }

        let rollout_result = play_random_rollout(
            board.clone(),
            to_play,
            self.nodes[node_idx].consecutive_passes,
            self.nodes[self.nodes[node_idx].parent_idx].state.hash(),
            2 * Board::AREA as usize, /* move limit */
            rng,
        );

        self.nodes[node_idx].child_indices = Some(Range {
            start: first_child,
            end: self.nodes.len(),
        });

        // This needs to be from the node parent's side to play perspective, not its own,
        // so we're inverting
        -score_value(&rollout_result.board, to_play, self.game_state.komi) // TODO: Should be computed by the NN 
    }
}

impl Node {
    fn is_terminal(&self) -> bool {
        self.consecutive_passes >= 2
    }

    /// Returns the average score of this node (from it's parent's perspective),
    /// or None, if the node was not visited yet.
    fn score(&self) -> Option<f32> {
        if self.visit_count == 0 {
            None
        } else {
            Some(self.value_sum / (self.visit_count as f32))
        }
    }

    /// Returns the PUCT exploration score for the node.
    /// `parent_score` is from the perspective of the parent's side to play.
    fn puct_score(&self, parent_visit_count: u32, parent_score: f32) -> f32 {
        const EXPLORATION_CONSTANT: f32 = 1.25; //TODO: Verify!
        self.score().unwrap_or(parent_score)
            + EXPLORATION_CONSTANT * self.prior_probability * (parent_visit_count as f32).sqrt()
                / ((1 + self.visit_count) as f32)
    }
}

/// Calculate a value of a terminal position.
fn score_value(board: &Board, perspective: Color, komi: HalfPoints) -> f32 {
    const REL_DIFFERENCE_WEIGHT: f32 = 0.1;

    // calculate score from perspective of black first
    let areas = board.areas().map(|x| HalfPoints::from(x as i32));

    let difference = areas.black - areas.white - komi;
    let difference_score = f32::from(difference) * (REL_DIFFERENCE_WEIGHT / Board::AREA as f32);
    let win_score = f32::from(difference.signum()) * (1.0 - REL_DIFFERENCE_WEIGHT);
    let score = win_score + difference_score;

    match perspective {
        Color::Black => score,
        Color::White => -score,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn score_value_perspective() {
        let board = Board::from_ascii("x").unwrap();

        assert_eq!(score_value(&board, Color::Black, HalfPoints::ZERO), 1.0);
        assert_eq!(score_value(&board, Color::White, HalfPoints::ZERO), -1.0);
    }
}
