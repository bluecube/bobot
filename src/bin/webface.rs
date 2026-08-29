use std::{ops::Deref, sync::Arc};

use axum::{
    Router,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{Html, IntoResponse, Json, Redirect},
    routing::{get, post},
};
use bobot::{Bitboard16, Color, GameState, Move};
use dashmap::DashMap;
use serde::Serialize;

type GameId = u128;

struct AppState {
    games: DashMap<GameId, GameState>,
}

#[derive(Serialize)]
struct GameStateResponse {
    board: String,
    legal_moves: String,
}

impl From<&GameState> for GameStateResponse {
    fn from(value: &GameState) -> Self {
        GameStateResponse {
            board: value.board.format_ascii(true),
            legal_moves: (!Bitboard16::from_iter(
                value
                    .board
                    .legal_moves(Color::Black)
                    .map(|(pos, _board)| pos),
            ))
            .format_ascii(0),
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
    app.games.insert(game_id, GameState::new());

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
    let mut game = app.games.get_mut(&game_id).ok_or(StatusCode::NOT_FOUND)?;

    game.play_move(next_move)
        .map_err(|_| StatusCode::CONFLICT)?;

    game.play_random_legal_move(&mut rand::rng())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(GameStateResponse::from(game.deref())))
}
