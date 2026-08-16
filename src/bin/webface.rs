use axum::{Router, http::header, response::Html, response::IntoResponse, routing::get};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // build our application with a route
    let app = Router::new()
        .route("/", get(index_html))
        .route("/board.js", get(board_js));

    // run it
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await;
}

async fn index_html() -> Html<&'static str> {
    Html(include_str!("../../webface/index.html"))
}

async fn board_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript;charset=utf-8")],
        include_str!("../../webface/board.js"),
    )
}
