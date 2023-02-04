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
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc;
use tracing::{debug, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

struct AppState {
    set: Arc<Mutex<HashMap<String, String>>>,
    context: zmq::Context,
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

    let set = Arc::new(Mutex::new(HashMap::new()));
    let context = zmq::Context::new();

    let app_state = AppState { set, context };

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

    // Determine a role, initiator or responder.
    let (role, initiator_channel) = {
        let mut set = state.set.lock().unwrap();
        match set.get(&passphrase) {
            Some(initiator_channel) => (Role::Responder, initiator_channel.to_owned()),
            None => {
                // TODO: Generate a unique channel name.
                let channel = String::from("xxx");
                set.insert(passphrase, channel.clone());
                (Role::Initiator, channel)
            }
        }
    };
    debug!("The client's role is: {role}, initiator channel: {initiator_channel}");

    // Exchange messages between two roles.
    // NOTE: The order of the following workflow is *very strict* for both initiator and responder.
    // One principle thumb of rule: receiving task should always be started before sending task if
    // these tasks are communicating with each other.
    match role {
        Role::Initiator => {
            debug!("Enter initiator workflow.");
            let context = &state.context;

            let context1 = context.clone();
            let initiator_channel_clone = initiator_channel.clone();
            // Relay messages from initiator websocket to responder.
            tokio::spawn(async move {
                // Create a publisher to relay initiator's messages to responder once responder is connected.
                let publisher = context1.socket(zmq::PUB).unwrap();
                publisher.set_sndhwm(100000).unwrap();
                publisher
                    .bind(&format!("inproc://{initiator_channel_clone}"))
                    .unwrap();

                while let Some(Ok(Message::Text(msg))) = receiver.next().await {
                    publisher.send(msg.as_str(), 0).unwrap();
                }
                publisher.send("END", 0).unwrap();
            });

            // Waiting for responder to be connected.
            let responder_channel = {
                let receiver = context.socket(zmq::PAIR).unwrap();
                receiver
                    .bind(&format!("inproc://{initiator_channel}:sync"))
                    .unwrap();
                receiver.recv_string(0).unwrap().unwrap()
            };
            debug!("responder channel: {responder_channel}");

            let context2 = context.clone();
            let tx1 = tx.clone();
            // Relay messages form responder to initiator's websocket.
            tokio::spawn(async move {
                // Create subscriber to subscribe from responder's messages.
                let subscriber = context2.socket(zmq::SUB).unwrap();
                subscriber
                    .connect(&format!("inproc://{responder_channel}"))
                    .unwrap();
                subscriber.set_subscribe(b"").unwrap();

                loop {
                    let message = subscriber.recv_string(0).unwrap().unwrap();
                    if message == "END" {
                        break;
                    }
                    tx1.send(Message::Text(message)).await.unwrap();
                }
            });

            // Tell client its role. So initiator will send its local SDP before responder does the same.
            // This message actually fires the data flow.
            tx.send(Message::Text(role.to_string())).await.unwrap();
        }
        Role::Responder => {
            debug!("Enter responder workflow.");
            // Tell client its role.
            tx.send(Message::Text(role.to_string())).await.unwrap();

            let context = &state.context;

            let context1 = context.clone();
            let tx1 = tx.clone();
            let initiator_channel_clone = initiator_channel.clone();
            // Relay messages from initiator to responder websocket.
            tokio::spawn(async move {
                // Create a subscriber to subscribe from initiator's messages.
                let subscriber = context1.socket(zmq::SUB).unwrap();
                subscriber
                    .connect(&format!("inproc://{initiator_channel_clone}"))
                    .unwrap();
                subscriber.set_subscribe(b"").unwrap();

                loop {
                    let message = subscriber.recv_string(0).unwrap().unwrap();
                    if message == "END" {
                        break;
                    }
                    tx1.send(Message::Text(message)).await.unwrap();
                }
            });

            // Signal Initiator that this responder client is connected.
            let responder_channel = {
                let xmitter = context.socket(zmq::PAIR).unwrap();
                xmitter
                    .connect(&format!("inproc://{initiator_channel}:sync"))
                    .unwrap();
                // TODO: generate a unique channel name.
                let responder_channel = "yyy";
                xmitter.send(responder_channel, 0).unwrap();
                responder_channel
            };

            let context2 = context.clone();
            // Relay messages form responder's websocket to initiator.
            tokio::spawn(async move {
                // Create a publisher to relay responder's messages to initiator.
                let publisher = context2.socket(zmq::PUB).unwrap();
                publisher.set_sndhwm(100000).unwrap();
                publisher
                    .bind(&format!("inproc://{responder_channel}"))
                    .unwrap();

                while let Some(Ok(Message::Text(msg))) = receiver.next().await {
                    publisher.send(msg.as_str(), 0).unwrap();
                }
                publisher.send("END", 0).unwrap();
            });
        }
    };
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
