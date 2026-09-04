use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::extract::ws::Message;
use tokio::sync::{mpsc, RwLock};

pub type ConnId = u64;

struct ConnectionInfo {
    sender: mpsc::UnboundedSender<Message>,
    registered_peer_id: Option<String>,
}

#[derive(Default)]
struct Inner {
    connections: HashMap<ConnId, ConnectionInfo>,
    registry: HashMap<String, ConnId>,
    pairs: HashMap<ConnId, ConnId>,
}

pub struct AppState {
    inner: RwLock<Inner>,
    next_id: AtomicU64,
}

impl AppState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: RwLock::new(Inner::default()),
            next_id: AtomicU64::new(1),
        })
    }

    pub fn next_conn_id(&self) -> ConnId {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    pub async fn add_connection(&self, conn_id: ConnId, sender: mpsc::UnboundedSender<Message>) {
        let mut inner = self.inner.write().await;
        inner.connections.insert(
            conn_id,
            ConnectionInfo {
                sender,
                registered_peer_id: None,
            },
        );
    }

    /// Registers `peer_id` as reachable via `conn_id`, replacing any prior
    /// (stale) registration under the same peer_id.
    pub async fn register(&self, conn_id: ConnId, peer_id: String) {
        let mut inner = self.inner.write().await;
        if let Some(info) = inner.connections.get_mut(&conn_id) {
            info.registered_peer_id = Some(peer_id.clone());
        }
        inner.registry.insert(peer_id, conn_id);
    }

    /// Looks up the registered daemon for `peer_id` and, if found, pairs it
    /// with `browser_id` bidirectionally.
    pub async fn connect(&self, browser_id: ConnId, peer_id: &str) -> Option<ConnId> {
        let mut inner = self.inner.write().await;
        let daemon_id = *inner.registry.get(peer_id)?;
        if !inner.connections.contains_key(&daemon_id) {
            return None;
        }
        inner.pairs.insert(browser_id, daemon_id);
        inner.pairs.insert(daemon_id, browser_id);
        Some(daemon_id)
    }

    /// Read-only check for whether `peer_id` currently has a live registered
    /// connection (used by the REST accounts API to report online status).
    pub async fn is_registered(&self, peer_id: &str) -> bool {
        let inner = self.inner.read().await;
        inner.registry.contains_key(peer_id)
    }

    pub async fn paired_with(&self, conn_id: ConnId) -> Option<ConnId> {
        let inner = self.inner.read().await;
        inner.pairs.get(&conn_id).copied()
    }

    pub async fn send_to(&self, conn_id: ConnId, message: Message) -> bool {
        let inner = self.inner.read().await;
        match inner.connections.get(&conn_id) {
            Some(info) => info.sender.send(message).is_ok(),
            None => false,
        }
    }

    /// Removes `conn_id`, dropping any registry entry and pairing it held.
    /// Returns the peer (if any) it was paired with, so the caller can
    /// notify that peer of the disconnect.
    pub async fn remove_connection(&self, conn_id: ConnId) -> Option<ConnId> {
        let mut inner = self.inner.write().await;

        if let Some(info) = inner.connections.remove(&conn_id) {
            if let Some(peer_id) = info.registered_peer_id {
                if inner.registry.get(&peer_id) == Some(&conn_id) {
                    inner.registry.remove(&peer_id);
                }
            }
        }

        let paired_id = inner.pairs.remove(&conn_id);
        if let Some(other_id) = paired_id {
            inner.pairs.remove(&other_id);
        }
        paired_id
    }
}
