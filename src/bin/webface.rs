use std::sync::Arc;

use axum::{
    Router,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{Html, IntoResponse, Json, Redirect},
    routing::{get, post},
};
use bobot::{Board, Color};
use dashmap::DashMap;
use rand::RngExt as _;
use serde::{Deserialize, Serialize};

type GameId = u128;

#[derive(Default)]
struct AppState {
    games: DashMap<GameId, Game>,
}

#[derive(Clone, Default)]
struct Game {
    board: Board,
}

impl Game {
    fn make_response(&self) -> GameStateResponse {
        GameStateResponse {
            board: self.board.format_ascii(true),
            legal_moves: (!self.board.legal_moves(Color::Black)).format_ascii(0),
        }
    }
}

#[derive(Serialize)]
struct GameStateResponse {
    board: String,
    legal_moves: String,
}

#[derive(Deserialize)]
struct PlayMove {
    row: usize,
    col: usize,
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(index_html))
        .route("/board.js", get(board_js))
        .route("/new_game", post(new_game))
        .route("/games/{game_id}", get(game_html))
        .route("/games/{game_id}/state", get(game_state))
        .route("/games/{game_id}/move", post(play_move))
        .with_state(Arc::new(AppState::default()));

    // run it
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn index_html() -> Html<&'static str> {
    Html(include_str!("../../webface/index.html"))
}

async fn game_html() -> Html<&'static str> {
    Html(include_str!("../../webface/game.html"))
}

async fn board_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript;charset=utf-8")],
        include_str!("../../webface/board.js"),
    )
}

#[axum::debug_handler]
async fn new_game(State(app): State<Arc<AppState>>) -> Redirect {
    let game_id = rand::random();
    app.games.insert(game_id, Game::default());

    Redirect::to(&format!("/games/{game_id}"))
}

#[axum::debug_handler]
async fn game_state(
    State(app): State<Arc<AppState>>,
    Path(game_id): Path<GameId>,
) -> Result<Json<GameStateResponse>, StatusCode> {
    let game = app.games.get(&game_id).ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(game.make_response()))
}

#[axum::debug_handler]
async fn play_move(
    State(app): State<Arc<AppState>>,
    Path(game_id): Path<GameId>,
    Json(next_move): Json<PlayMove>,
) -> Result<Json<GameStateResponse>, StatusCode> {
    let mut game = app.games.get_mut(&game_id).ok_or(StatusCode::NOT_FOUND)?;

    game.board = game
        .board
        .play_stone(next_move.row, next_move.col, Color::Black)
        .map_err(|_| StatusCode::CONFLICT)?;

    game.board = random_move(&game.board, Color::White);

    Ok(Json(game.make_response()))
}

fn random_move(board: &Board, color: Color) -> Board {
    let mut candidates = board.empty_positions();
    let mut rng = rand::rng();

    while !candidates.is_empty() {
        let i = rng.random_range(0..candidates.popcnt());
        let (r, c) = candidates.iter_positions().skip(i).next().unwrap();
        if let Ok(new_board) = board.play_stone(r, c, color) {
            return new_board;
        } else {
            candidates.set(r, c, false);
        }
    }

    Board::new()
}
