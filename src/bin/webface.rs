use std::{ops::Deref, sync::Arc};

use axum::{
    Router,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{Html, IntoResponse, Json, Redirect},
    routing::{get, post},
};
use bobot::{Bitboard16, Color, GameState, Move, solver::Solver};
use dashmap::DashMap;
use serde::Serialize;
use tokio::task::spawn_blocking;

type GameId = u128;

struct AppState {
    games: DashMap<GameId, WebfaceGameState>,
}

#[derive(Debug, Default)]
struct WebfaceGameState {
    game_state: GameState,
    last_move: Option<Move>,
}

#[derive(Serialize)]
struct GameStateResponse {
    board: String,
    last_move: Option<Move>,
    legal_moves: String,
}

impl From<&WebfaceGameState> for GameStateResponse {
    fn from(value: &WebfaceGameState) -> Self {
        GameStateResponse {
            board: value.game_state.board.format_ascii(true),
            legal_moves: (!Bitboard16::from_iter(
                value
                    .game_state
                    .board
                    .legal_moves(Color::Black)
                    .map(|(pos, _board)| pos),
            ))
            .format_ascii(0),
            last_move: value.last_move.clone(),
        }
    }
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
        .with_state(Arc::new(AppState {
            games: DashMap::default(),
        }));

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
    app.games.insert(game_id, WebfaceGameState::default());

    Redirect::to(&format!("/games/{game_id}"))
}

#[axum::debug_handler]
async fn game_state(
    State(app): State<Arc<AppState>>,
    Path(game_id): Path<GameId>,
) -> Result<Json<GameStateResponse>, StatusCode> {
    let game = app.games.get(&game_id).ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(GameStateResponse::from(game.deref())))
}

#[axum::debug_handler]
async fn play_move(
    State(app): State<Arc<AppState>>,
    Path(game_id): Path<GameId>,
    Json(next_move): Json<Move>,
) -> Result<Json<GameStateResponse>, StatusCode> {
    // Play the input move
    let game_state = {
        let mut game = app.games.get_mut(&game_id).ok_or(StatusCode::NOT_FOUND)?;

        game.game_state
            .play_move(next_move)
            .map_err(|_| StatusCode::CONFLICT)?;

        game.game_state.clone()
    };

    // Run the solver in background task
    let best_move = spawn_blocking(|| {
        // TODO: Don't clone, keep solver in state, reuse tree on move.
        let mut solver = Solver::new(game_state);

        for _i in 0..10000 {
            solver.one_step();
        }

        solver.best_move()
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Play the response move
    let mut game = app.games.get_mut(&game_id).ok_or(StatusCode::NOT_FOUND)?;
    game.game_state
        .play_move(best_move.clone())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    game.last_move = Some(best_move);

    Ok(Json(GameStateResponse::from(game.deref())))
}
