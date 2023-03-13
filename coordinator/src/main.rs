mod db;
mod session;

use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{
        ws::{WebSocket, WebSocketUpgrade},
        State,
    },
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use db::DbDropGuard;
use tracing::debug;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use session::Session;

struct AppState {
    db_holder: DbDropGuard,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "coordinator=trace".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let db_holder = DbDropGuard::new();
    let app_state = AppState { db_holder };

    let app = Router::new()
        .route("/", get(index))
        .route("/websocket", get(websocket_handler))
        .with_state(app_state.into());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, state))
}

async fn websocket(stream: WebSocket, state: Arc<AppState>) {
    let mut session = Session::new(stream, state).await;

    let passphrase = session.passphrase().await;
    debug!("Passphrase: {passphrase}");

    let role = session.role(passphrase.clone());
    debug!("The client's role is: {role}.");

    session.exchange_messages(passphrase.clone(), &role).await;

    session.notify(&passphrase, &role).await;
}

// Include utf-8 file at **compile** time.
async fn index() -> Html<&'static str> {
    Html("")
}
