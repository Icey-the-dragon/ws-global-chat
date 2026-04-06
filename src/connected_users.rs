use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use warp::filters::ws::Message;

/// Each user_id maps to a list of senders (one per open tab/connection).
pub type ConnectedUsers = Arc<RwLock<HashMap<i32, Vec<mpsc::UnboundedSender<Message>>>>>;

/// Create an empty connected-users registry.
pub fn new_registry() -> ConnectedUsers {
    Arc::new(RwLock::new(HashMap::new()))
}

/// Register a new sender for a user. Returns the index so we can remove it later.
pub async fn register(
    connected: &ConnectedUsers,
    user_id: i32,
    sender: mpsc::UnboundedSender<Message>,
) -> usize {
    let mut map = connected.write().await;
    let senders = map.entry(user_id).or_default();
    senders.push(sender);
    senders.len() - 1
}

/// Remove a sender for a user by index. Cleans up the entry if empty.
pub async fn deregister(connected: &ConnectedUsers, user_id: i32, index: usize) {
    let mut map = connected.write().await;
    if let Some(senders) = map.get_mut(&user_id) {
        if index < senders.len() {
            senders.remove(index);
        }
        if senders.is_empty() {
            map.remove(&user_id);
        }
    }
}

/// Send a text message to all connections of a specific user.
/// Returns the number of senders that were still alive.
pub async fn send_to_user(
    connected: &ConnectedUsers,
    user_id: i32,
    text: &str,
) -> usize {
    let map = connected.read().await;
    let mut delivered = 0;
    if let Some(senders) = map.get(&user_id) {
        for sender in senders {
            if sender.send(Message::text(text)).is_ok() {
                delivered += 1;
            }
        }
    }
    delivered
}

/// Get the list of all currently connected user IDs.
pub async fn get_online_user_ids(connected: &ConnectedUsers) -> Vec<i32> {
    let map = connected.read().await;
    map.keys().cloned().collect()
}
