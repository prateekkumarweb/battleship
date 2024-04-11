#![forbid(unsafe_code)]
#![warn(
    clippy::pedantic,
    clippy::nursery,
    clippy::unwrap_used,
    clippy::expect_used
)]

use std::{collections::VecDeque, sync::Arc, time::Duration};

use anyhow::Context;
use axum::{
    extract::{ws::WebSocket, State, WebSocketUpgrade},
    http::header,
    response::{Redirect, Response},
    routing::get,
    Router,
};
use game::Game;
use message::SystemMsg;
use player::Player;
use tokio::sync::Mutex;
use tower::ServiceBuilder;
use tower_http::{
    request_id::MakeRequestUuid,
    services::ServeDir,
    timeout::TimeoutLayer,
    trace::{
        DefaultMakeSpan, DefaultOnBodyChunk, DefaultOnEos, DefaultOnFailure, DefaultOnRequest,
        DefaultOnResponse, TraceLayer,
    },
    LatencyUnit, ServiceBuilderExt,
};
use tracing::Level;

mod game;
mod message;
mod player;

#[derive(Debug)]
struct AppState {
    queue: Mutex<VecDeque<Player>>,
    games: Mutex<Vec<Game>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let app_state = AppState {
        queue: Mutex::new(VecDeque::new()),
        games: Mutex::new(Vec::new()),
    };
    let app = router().with_state(Arc::new(app_state));
    let address = "0.0.0.0:8080";
    tracing::info!("Listening on http://{}", address);
    let listener = tokio::net::TcpListener::bind(address).await?;
    axum::serve(listener, app)
        .await
        .context("Failed to bind to address")
}

fn router() -> Router<Arc<AppState>> {
    let sensitive_headers: Arc<[_]> = Arc::new([header::AUTHORIZATION, header::COOKIE]);

    let middleware = ServiceBuilder::new()
        .set_x_request_id(MakeRequestUuid)
        .sensitive_request_headers(sensitive_headers.clone())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(
                    DefaultMakeSpan::new()
                        .level(Level::INFO)
                        .include_headers(true),
                )
                .on_request(DefaultOnRequest::new().level(Level::INFO))
                .on_response(
                    DefaultOnResponse::new()
                        .level(Level::INFO)
                        .include_headers(true)
                        .latency_unit(LatencyUnit::Micros),
                )
                .on_body_chunk(DefaultOnBodyChunk::new())
                .on_eos(
                    DefaultOnEos::new()
                        .level(Level::INFO)
                        .latency_unit(LatencyUnit::Micros),
                )
                .on_failure(
                    DefaultOnFailure::new()
                        .level(Level::INFO)
                        .latency_unit(LatencyUnit::Micros),
                ),
        )
        .propagate_x_request_id()
        .sensitive_response_headers(sensitive_headers)
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        .compression();

    Router::new()
        .route("/", get(|| async { Redirect::temporary("/game/") }))
        .nest_service("/game", ServeDir::new("game"))
        .route("/ws", get(ws_handler))
        .layer(middleware)
}

async fn ws_handler(State(state): State<Arc<AppState>>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(|ws| handle_socket(state, ws))
}

async fn handle_socket(state: Arc<AppState>, socket: WebSocket) {
    let mut player = Player::new(socket);
    let mut queue = state.queue.lock().await;
    if let Err(e) = player.send_id().await {
        tracing::error!("Failed to send id: {:?}", e);
        return;
    }
    if let Err(e) = player
        .send(SystemMsg::Log("Searching for an opponent".to_owned()))
        .await
    {
        tracing::error!("Failed to send log: {:?}", e);
        return;
    };
    queue.push_back(player);
    drop(queue);
    match_players(state).await;
}

async fn match_players(state: Arc<AppState>) {
    let mut queue = state.queue.lock().await;
    if queue.len() < 2 {
        return;
    }
    let mut player1 = loop {
        let player = queue.pop_front();
        match player {
            Some(player) if !player.is_closed().await => break player,
            Some(_) | None => {}
        }
    };
    let mut player2 = {
        let player = queue.pop_front();
        match player {
            Some(player) if !player.is_closed().await => player,
            Some(_) | None => {
                queue.push_front(player1);
                return;
            }
        }
    };
    drop(queue);
    if (player1.send_opponent_id(&player2).await).is_err() {
        player1.close();
        return;
    };
    if (player2.send_opponent_id(&player1).await).is_err() {
        player1.close();
        return;
    }
    let game = game::Game::new(player1, player2).await;
    let mut games = state.games.lock().await;
    games.push(game);
}
