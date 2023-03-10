use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use redis::{aio::ConnectionManager, AsyncCommands, Client};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::mpsc;
use tracing::{debug, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod session;

struct AppState {
    // Only used for creating pubsub connection.
    client: Client,
    // Generally used for basic commands other than pubsub.
    conn: ConnectionManager,
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

    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    // let pubsub = client.get_tokio_connection().await.unwrap().into_pubsub();
    let conn = client.get_tokio_connection_manager().await.unwrap();

    let app_state = AppState { client, conn };

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
    let (mut sender, mut receiver) = stream.split();

    let (tx, mut rx) = mpsc::channel(100);
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            sender.send(msg).await.unwrap();
        }
    });

    // Get passphrase.
    let passphrase = loop {
        // Loop until a text message is found which should be passphrase.
        if let Some(Ok(Message::Text(passphrase))) = receiver.next().await {
            if !passphrase.is_empty() {
                break passphrase;
            }
            // Passphrase should not be empty.
            tx.send(Message::Text("Empty passphrase.".into()))
                .await
                .unwrap();
            warn!("Received empty passphrase from client.");
            return;
        }
    };
    debug!("Passphrase: {passphrase}");

    let mut conn = state.conn.clone();

    // Determine a role, initiator or responder.
    let role = match conn.set_nx(&passphrase, "").await.unwrap() {
        1 => Role::Initiator,
        0 => Role::Responder,
        _ => {
            warn!("Unexpected result returned from Redis.");
            return;
        }
    };
    debug!("The client's role is: {role}.");

    let mut pubsub = state
        .client
        .get_tokio_connection()
        .await
        .unwrap()
        .into_pubsub();
    let notification_channel = [&passphrase, "notification"].join(":");
    match role {
        Role::Initiator => {
            let responder_channel = [&passphrase, "responder"].join(":");
            let tx2 = tx.clone();
            tokio::spawn(async move {
                pubsub.subscribe(responder_channel).await.unwrap();
                while let Some(msg) = pubsub.on_message().next().await {
                    let payload: String = msg.get_payload().unwrap();
                    tx2.send(Message::Text(payload)).await.unwrap();
                }
            });
            tokio::spawn(async move {
                while let Some(Ok(Message::Text(text))) = receiver.next().await {
                    let _: () = conn.publish("", text).await.unwrap();
                }
            });

            let mut pubsub = state
                .client
                .get_tokio_connection()
                .await
                .unwrap()
                .into_pubsub();
            pubsub.subscribe(&notification_channel).await.unwrap();
            if pubsub.on_message().next().await.is_some() {
                tx.send(Message::Text(Role::Initiator.to_string()))
                    .await
                    .unwrap();
            }
        }
        Role::Responder => {
            let initiator_channel = [&passphrase, "initiator"].join(":");
            tokio::spawn(async move {
                pubsub.subscribe(initiator_channel).await.unwrap();
                while let Some(msg) = pubsub.on_message().next().await {
                    let payload: String = msg.get_payload().unwrap();
                    tx.send(Message::Text(payload)).await.unwrap();
                }
            });
            let mut conn2 = conn.clone();
            tokio::spawn(async move {
                while let Some(Ok(Message::Text(text))) = receiver.next().await {
                    let _: () = conn2.publish("", text).await.unwrap();
                }
            });
            let _: () = conn.publish(notification_channel, "").await.unwrap();
        }
    }
}

// Include utf-8 file at **compile** time.
async fn index() -> Html<&'static str> {
    Html("")
}

/// Peer role.
enum Role {
    Initiator,
    Responder,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Role::Initiator => write!(f, "Initiator"),
            Role::Responder => write!(f, "Responder"),
        }
    }
}
