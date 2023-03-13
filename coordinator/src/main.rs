mod db;

use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use db::DbHolder;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tracing::{debug, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

struct AppState {
    db_holder: DbHolder,
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

    let db_holder = DbHolder::new();
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
    let (mut sender, mut receiver) = stream.split();

    let (tx, mut rx) = mpsc::channel(100);
    let mut send_task = tokio::spawn(async move {
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

    let db = state.db_holder.db();

    // Determine a role, initiator or responder.
    let role = match db.set_nx(passphrase.clone(), String::from("")) {
        1 => Role::Initiator,
        _ => Role::Responder,
    };
    debug!("The client's role is: {role}.");

    let channel_for_role = channel_name(passphrase.clone(), &role);
    let channel_for_opposite_role = channel_name(passphrase.clone(), &role.opposite());

    // Exchange messages between initiator and responder.
    let tx2 = tx.clone();
    let mut subscriber = db.subscribe(channel_for_opposite_role.clone());
    let subscribe_task = tokio::spawn(async move {
        while let Ok(msg) = subscriber.recv().await {
            let _ = tx2.send(Message::Text(msg)).await;
        }
    });

    let db2 = db.clone();
    let channel_for_role2 = channel_for_role.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(msg))) = receiver.next().await {
            if db2.publish(&channel_for_role2, msg) == 0 {
                warn!("Publish not successful.");
            }
        }
    });

    // Signal coordination.
    let notification_channel_name = [&passphrase, "notification"].join(":");
    match role {
        Role::Initiator => {
            let _ = db.subscribe(notification_channel_name.clone()).recv().await;
        }
        Role::Responder => {
            if db.publish(&notification_channel_name, String::from("")) == 0 {
                warn!("Publish not successful.");
            }
        }
    }
    let _ = tx.send(Message::Text(role.to_string())).await;

    // If any one of the tasks run to completion, we abort the other.
    tokio::select! {
        _ =(&mut send_task) => {
            recv_task.abort();
            subscribe_task.abort();
        },
        _ = (&mut recv_task) => {
            send_task.abort();
            subscribe_task.abort();
        },
    }

    // Cleaning task.
    db.delete(&passphrase);
    for channel in [
        channel_for_role,
        channel_for_opposite_role,
        notification_channel_name,
    ] {
        db.delete_channel(&channel);
    }
    debug!("Session {passphrase}:{role} ended.");
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

impl Role {
    fn opposite(&self) -> Role {
        match self {
            Role::Initiator => Role::Responder,
            Role::Responder => Role::Initiator,
        }
    }
}

fn channel_name(prefix: String, role: &Role) -> String {
    [prefix, role.to_string()].join(":")
}
