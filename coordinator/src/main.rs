mod db;

use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use db::DbHolder;
use futures_util::{SinkExt, StreamExt};
use protocol::{Event, Role};
use tokio::sync::mpsc;
use tower_http::services::ServeDir;
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
        .nest_service("/", ServeDir::new("static"))
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
            if let Err(error) = sender.send(msg).await {
                warn!("WebSocket failed to send message: {error}");
                return;
            }
        }
    });

    // Get passphrase.
    let passphrase = match receiver.next().await {
        Some(Ok(Message::Text(msg))) => match serde_json::from_str::<Event>(&msg) {
            Ok(Event::Passphrase(passphrase)) => passphrase,
            _ => {
                tx.send(Message::Text(
                    serde_json::to_string(&Event::Error("Invalid passphrase.".into())).unwrap(),
                ))
                .await
                .unwrap();
                warn!("Received invalid passphrase from client.");
                return;
            }
        },
        _ => {
            tx.send(Message::Text(
                serde_json::to_string(&Event::Error(
                    "First message should be a passphrase string.".into(),
                ))
                .unwrap(),
            ))
            .await
            .unwrap();
            warn!("No passphrase string received.");
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
    let tx_clone = tx.clone();
    let mut subscriber = db.subscribe(channel_for_opposite_role.clone());
    let subscribe_task = tokio::spawn(async move {
        while let Ok(msg) = subscriber.recv().await {
            tx_clone.send(Message::Text(msg)).await.unwrap();
        }
    });

    let db_clone = db.clone();
    let channel_for_role_clone = channel_for_role.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(msg))) = receiver.next().await {
            if matches!(
                serde_json::from_str::<Event>(&msg).unwrap(),
                Event::CloseConnection
            ) {
                // Return from the receiving task will end this session.
                return;
            }
            if db_clone.publish(&channel_for_role_clone, msg) == 0 {
                warn!("Publish not successful.");
            }
        }
    });

    // Signal coordination.
    let notification_channel_name = [&passphrase, "notification"].join(":");
    match role {
        Role::Initiator => {
            db.subscribe(notification_channel_name.clone())
                .recv()
                .await
                .unwrap();
        }
        Role::Responder => {
            if db.publish(&notification_channel_name, String::from("")) == 0 {
                warn!("Publish not successful.");
            }
        }
    }
    let role_clone = role.clone();
    tx.send(Message::Text(
        serde_json::to_string(&Event::Role(role_clone)).unwrap(),
    ))
    .await
    .unwrap();

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

fn channel_name(prefix: String, role: &Role) -> String {
    [prefix, role.to_string()].join(":")
}
