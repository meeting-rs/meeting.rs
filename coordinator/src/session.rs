use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::StreamExt;
use redis::aio::PubSub;
use redis::{aio::ConnectionManager, AsyncCommands, Client};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tracing::{debug, warn};

use crate::{AppState, Role};

struct Session {
    client: Client,
    conn: ConnectionManager,
    sender: SplitSink<WebSocket, Message>,
    receiver: SplitStream<WebSocket>,
    tx: Sender<Message>,
    rx: Receiver<Message>,
}

impl Session {
    fn new(stream: WebSocket, state: Arc<AppState>) -> Self {
        let (sender, receiver) = stream.split();
        let (tx, rx) = mpsc::channel(100);
        Session {
            client: state.client.clone(),
            conn: state.conn.clone(),
            sender,
            receiver,
            tx,
            rx,
        }
    }

    /// Get passphrase.
    async fn passphrase(&mut self) -> Option<String> {
        match self.receiver.next().await {
            Some(Ok(Message::Text(passphrase))) => {
                if !passphrase.is_empty() {
                    return Some(passphrase);
                }
                self.tx
                    .send(Message::Text("Empty passphrase.".into()))
                    .await
                    .unwrap();
                warn!("Received empty passphrase from client.");
                None
            }
            _ => None,
        }
    }

    /// Determine a role, initiator or responder.
    async fn role(&mut self, passphrase: &str) -> Option<Role> {
        match self.conn.set_nx(passphrase, "").await.unwrap() {
            1 => Some(Role::Initiator),
            0 => Some(Role::Responder),
            _ => {
                warn!("Unexpected result returned from Redis.");
                None
            }
        }
    }

    /// Get a new PubSub connection.
    async fn pubsub_conn(&self) -> PubSub {
        self.client
            .get_tokio_connection()
            .await
            .unwrap()
            .into_pubsub()
    }

    async fn initiator_broker(&mut self, responder_channel: String, initiator_channel: String) {
        let mut pubsub = self.pubsub_conn().await;
        pubsub.subscribe(responder_channel).await.unwrap();

        let tx = self.tx.clone();
        tokio::spawn(async move {
            while let Some(msg) = pubsub.on_message().next().await {
                let payload: String = msg.get_payload().unwrap();
                tx.send(Message::Text(payload)).await.unwrap();
            }
        });

        let mut conn = self.conn.clone();
        tokio::spawn(async move {
            while let Some(Ok(Message::Text(text))) = self.receiver.next().await {
                let _: () = conn.publish(initiator_channel, text).await.unwrap();
            }
        });
    }
}
