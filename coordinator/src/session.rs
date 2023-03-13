use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{stream::SplitStream, SinkExt, StreamExt};
use tokio::sync::mpsc::{self, Sender};
use tracing::{debug, warn};

use crate::{db::Db, AppState};

pub(crate) struct Session {
    // To overcome lifetime issue, we use an Option here.
    // TODO: Find other better way to fix lifetime issue.
    receiver: Option<SplitStream<WebSocket>>,
    tx: Sender<Message>,
    db: Db,
    passphrase: Option<String>,
    // Pre-allocated with size of 3 vector.
    channels: Vec<String>,
}

impl Session {
    pub(crate) async fn new(stream: WebSocket, state: Arc<AppState>) -> Self {
        let (mut sender, receiver) = stream.split();

        let (tx, mut rx) = mpsc::channel(100);
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                sender.send(msg).await.unwrap();
            }
        });

        Session {
            receiver: Some(receiver),
            tx,
            db: state.db_holder.db(),
            passphrase: None,
            channels: Vec::with_capacity(3),
        }
    }

    /// Gets passphrase.
    pub(crate) async fn passphrase(&mut self) -> String {
        loop {
            // Loop until a text message is found which should be passphrase.
            if let Some(Ok(Message::Text(passphrase))) =
                self.receiver.as_mut().unwrap().next().await
            {
                if !passphrase.is_empty() {
                    self.passphrase = Some(passphrase.clone());
                    break passphrase;
                }
                // Passphrase should not be empty.
                self.tx
                    .send(Message::Text("Empty passphrase.".into()))
                    .await
                    .unwrap();
                warn!("Received empty passphrase from client.");
            }
        }
    }

    /// Determines a role, initiator or responder.
    pub(crate) fn role(&mut self, passphrase: String) -> Role {
        match self.db.set_nx(passphrase, String::from(""), None) {
            1 => Role::Initiator,
            _ => Role::Responder,
        }
    }

    pub(crate) async fn exchange_messages(&mut self, passphrase: String, role: &Role) {
        let channel_for_role = channel_name(passphrase.clone(), role);
        let channel_for_opposite_role = channel_name(passphrase, &role.opposite());

        self.channels.push(channel_for_role.clone());
        self.channels.push(channel_for_opposite_role.clone());

        let tx = self.tx.clone();
        let mut subscriber = self.db.subscribe(channel_for_opposite_role);
        tokio::spawn(async move {
            while let Ok(msg) = subscriber.recv().await {
                tx.send(Message::Text(msg)).await.unwrap();
            }
        });

        let db = self.db.clone();
        let mut receiver = self.receiver.take().unwrap();
        tokio::spawn(async move {
            while let Some(Ok(Message::Text(msg))) = receiver.next().await {
                if db.publish(&channel_for_role, msg) == 0 {
                    warn!("Publish not successful.");
                }
            }
        });
    }

    pub(crate) async fn notify(&mut self, passphrase: &str, role: &Role) {
        let notification_channel_name = [passphrase, "notification"].join(":");
        self.channels.push(notification_channel_name.clone());

        match role {
            Role::Initiator => {
                let _ = self.db.subscribe(notification_channel_name).recv().await;
                self.tx.send(Message::Text(role.to_string())).await.unwrap();
            }
            Role::Responder => {
                if self
                    .db
                    .publish(&notification_channel_name, String::from(""))
                    == 0
                {
                    warn!("Publish not successful.");
                }
            }
        }
    }
}

/// Peer role.
pub(crate) enum Role {
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

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(passphrase) = self.passphrase.as_ref() {
            self.db.delete(passphrase);
            debug!("Session {} ended.", passphrase);
        }

        self.channels.iter().for_each(|channel| {
            self.db.delete_channel(channel);
        });
    }
}
