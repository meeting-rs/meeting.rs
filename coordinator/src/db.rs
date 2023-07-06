//! This module is a mimicry of Redis db with only limited while necessary commands.
//! Most code is excerpted from https://github.com/tokio-rs/mini-redis/blob/master/src/db.rs.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tokio::sync::broadcast;

#[derive(Debug)]
pub(crate) struct DbHolder {
    db: Db,
}

#[derive(Debug, Clone)]
pub(crate) struct Db {
    shared: Arc<Shared>,
}

#[derive(Debug)]
struct Shared {
    state: Mutex<State>,
}

#[derive(Debug)]
struct State {
    entries: HashMap<String, Entry>,
    pub_sub: HashMap<String, broadcast::Sender<String>>,
}

/// Entry in the key-value store
#[derive(Debug)]
struct Entry {
    /// Stored data
    data: String,
}

impl DbHolder {
    pub(crate) fn new() -> DbHolder {
        DbHolder { db: Db::new() }
    }

    /// Get the shared database. Internally, this is an
    /// `Arc`, so a clone only increments the ref count.
    pub(crate) fn db(&self) -> Db {
        self.db.clone()
    }
}

impl Db {
    pub(crate) fn new() -> Db {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                entries: HashMap::new(),
                pub_sub: HashMap::new(),
            }),
        });

        Db { shared }
    }

    /// Delete the value associated with a key.
    ///
    /// Returns the number of value deleted, which may be 1 or 0.
    pub(crate) fn delete(&self, key: &str) -> usize {
        let mut state = self.shared.state.lock().unwrap();
        state.entries.remove(key).map(|_| 1).unwrap_or(0)
    }

    /// Delete the pubsub channel associated with a key.
    ///
    /// Returns the number of pubsub channel deleted, which may be 1 or 0.
    pub(crate) fn delete_channel(&self, key: &str) -> usize {
        let mut state = self.shared.state.lock().unwrap();
        state.pub_sub.remove(key).map(|_| 1).unwrap_or(0)
    }

    /// Set the value associated with a key along with an optional expiration
    /// Duration.
    ///
    /// If a value is already associated with the key, it won't insert.
    ///
    /// Returns 1 if operation is successful, otherwise 0.
    pub(crate) fn set_nx(&self, key: String, value: String) -> usize {
        let mut state = self.shared.state.lock().unwrap();
        if state.entries.contains_key(&key) {
            return 0;
        }

        // Insert the entry into the `HashMap`.
        state.entries.insert(key, Entry { data: value });

        // Release the mutex before notifying the background task. This helps
        // reduce contention by avoiding the background task waking up only to
        // be unable to acquire the mutex due to this function still holding it.
        drop(state);

        1
    }

    /// Returns a `Receiver` for the requested channel.
    ///
    /// The returned `Receiver` is used to receive values broadcast by `PUBLISH`
    /// commands.
    pub(crate) fn subscribe(&self, key: String) -> broadcast::Receiver<String> {
        use std::collections::hash_map::Entry;

        // Acquire the mutex
        let mut state = self.shared.state.lock().unwrap();

        // If there is no entry for the requested channel, then create a new
        // broadcast channel and associate it with the key. If one already
        // exists, return an associated receiver.
        match state.pub_sub.entry(key) {
            Entry::Occupied(e) => e.get().subscribe(),
            Entry::Vacant(e) => {
                // No broadcast channel exists yet, so create one.
                //
                // The channel is created with a capacity of `1024` messages. A
                // message is stored in the channel until **all** subscribers
                // have seen it. This means that a slow subscriber could result
                // in messages being held indefinitely.
                //
                // When the channel's capacity fills up, publishing will result
                // in old messages being dropped. This prevents slow consumers
                // from blocking the entire system.
                let (tx, rx) = broadcast::channel(1024);
                e.insert(tx);
                rx
            }
        }
    }

    /// Publish a message to the channel. Returns the number of subscribers
    /// listening on the channel.
    pub(crate) fn publish(&self, key: &str, value: String) -> usize {
        let state = self.shared.state.lock().unwrap();

        state
            .pub_sub
            .get(key)
            // On a successful message send on the broadcast channel, the number
            // of subscribers is returned. An error indicates there are no
            // receivers, in which case, `0` should be returned.
            .map(|tx| tx.send(value).unwrap_or(0))
            // If there is no entry for the channel key, then there are no
            // subscribers. In this case, return `0`.
            .unwrap_or(0)
    }
}
