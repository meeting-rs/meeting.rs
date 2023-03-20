use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Event {
    Role(Role),
    Passphrase(String),
    Offer(String),
    Answer(String),
    IceCandidate(String),
    Error(String),
}

/// Peer role.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Role {
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
    pub fn opposite(&self) -> Role {
        match self {
            Role::Initiator => Role::Responder,
            Role::Responder => Role::Initiator,
        }
    }
}
