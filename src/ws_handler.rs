use futures_util::{SinkExt, StreamExt as _};
use tokio::sync::{broadcast, mpsc, RwLock};
use warp::filters::ws::{Message, WebSocket};
use std::sync::Arc;
use std::collections::HashSet;

use crate::connected_users::{self, ConnectedUsers};
use crate::ws_types::*;

pub async fn handle_connection(
    pool: sqlx::MySqlPool,
    ws: WebSocket,
    tx: broadcast::Sender<String>,
    session_cache: Arc<RwLock<HashSet<String>>>,
    connected: ConnectedUsers,
) {
    let (mut ws_sender, mut ws_receiver) = ws.split();

    // Per-connection channel for direct messages (private, error, who probes, etc.)
    let (direct_tx, mut direct_rx) = mpsc::unbounded_channel::<Message>();

    // Broadcast listener
    let mut broadcast_rx = tx.subscribe();

    // Forward both broadcast and direct messages to the WS sender
    tokio::spawn(async move {
        loop {
            tokio::select! {
                Ok(msg) = broadcast_rx.recv() => {
                    if ws_sender.send(Message::text(msg)).await.is_err() {
                        break;
                    }
                }
                Some(msg) = direct_rx.recv() => {
                    if ws_sender.send(msg).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Track this connection's authenticated state
    let mut authenticated_user_id: Option<i32> = None;
    let mut authenticated_username: Option<String> = None;
    let mut sender_index: Option<usize> = None;

    while let Some(result) = ws_receiver.next().await {
        match result {
            Ok(message) => {
                if let Ok(text) = message.to_str() {
                    if let Ok(ws_msg) = serde_json::from_str::<WsIncoming>(text) {
                        // ── Authenticate ──
                        let (user_id, username) =
                            match resolve_session(&pool, &session_cache, &ws_msg.metadata.session_id).await {
                                Some((uid, uname)) => {
                                    // Register in connected users on first auth
                                    if authenticated_user_id.is_none() {
                                        let idx = connected_users::register(
                                            &connected,
                                            uid,
                                            direct_tx.clone(),
                                        )
                                        .await;
                                        authenticated_user_id = Some(uid);
                                        authenticated_username = Some(uname.clone());
                                        sender_index = Some(idx);
                                    }
                                    (uid, uname)
                                }
                                None => {
                                    send_error(&direct_tx, "Invalid or expired session");
                                    continue;
                                }
                            };

                        // ── Route by message type ──
                        match ws_msg.msg_type {
                            MessageType::Broadcast => {
                                handle_broadcast(&pool, &tx, user_id, &username, &ws_msg.content).await;
                            }
                            MessageType::Private => {
                                handle_private(
                                    &pool,
                                    &connected,
                                    &direct_tx,
                                    user_id,
                                    &username,
                                    &ws_msg,
                                )
                                .await;
                            }
                            MessageType::Ephemeral => {
                                handle_ephemeral(&tx, &username, &ws_msg.content, ws_msg.extra);
                            }
                        }
                    } else {
                        println!("Failed to parse WS message: {}", text);
                    }
                }
            }
            Err(_e) => break,
        }
    }

    // ── Cleanup on disconnect ──
    if let (Some(uid), Some(idx)) = (authenticated_user_id, sender_index) {
        connected_users::deregister(&connected, uid, idx).await;
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Validate session token and return (user_id, username).
async fn resolve_session(
    pool: &sqlx::MySqlPool,
    session_cache: &Arc<RwLock<HashSet<String>>>,
    token: &str,
) -> Option<(i32, String)> {
    let cache = session_cache.read().await;
    if !cache.contains(token) {
        return None;
    }
    drop(cache);

    match crate::tables::user_db::get_user_by_token(pool, token).await {
        Ok(user) => Some((user.id, user.username)),
        Err(_) => None,
    }
}

/// Send an error frame to a single connection.
fn send_error(direct_tx: &mpsc::UnboundedSender<Message>, msg: &str) {
    let out = WsOutgoing {
        msg_type: OutgoingType::Error,
        username: "system".to_string(),
        content: msg.to_string(),
        to_username: None,
        users: None,
        extra: None,
    };
    if let Ok(json) = serde_json::to_string(&out) {
        let _ = direct_tx.send(Message::text(json));
    }
}

// ─── Message type handlers ──────────────────────────────────────────────────

async fn handle_broadcast(
    pool: &sqlx::MySqlPool,
    tx: &broadcast::Sender<String>,
    user_id: i32,
    username: &str,
    content: &str,
) {
    if crate::tables::user_db::save_message(pool, user_id, content)
        .await
        .is_err()
    {
        println!("Failed to save broadcast message from user {}", user_id);
        return;
    }

    let out = WsOutgoing {
        msg_type: OutgoingType::Broadcast,
        username: username.to_string(),
        content: content.to_string(),
        to_username: None,
        users: None,
        extra: None,
    };
    if let Ok(json) = serde_json::to_string(&out) {
        let _ = tx.send(json);
    }
}

async fn handle_private(
    pool: &sqlx::MySqlPool,
    connected: &ConnectedUsers,
    sender_direct_tx: &mpsc::UnboundedSender<Message>,
    sender_id: i32,
    sender_username: &str,
    ws_msg: &WsIncoming,
) {
    let to_username = match &ws_msg.metadata.to_username {
        Some(name) => name.clone(),
        None => {
            send_error(sender_direct_tx, "Private message requires 'to_username' in metadata");
            return;
        }
    };

    // Resolve target user to get their ID
    let target_user = match crate::tables::user_db::find_user_by_username(pool, &to_username).await {
        Ok(u) => u,
        Err(_) => {
            send_error(sender_direct_tx, &format!("User '{}' not found", to_username));
            return;
        }
    };

    let out = WsOutgoing {
        msg_type: OutgoingType::Private,
        username: sender_username.to_string(),
        content: ws_msg.content.clone(),
        to_username: Some(to_username.clone()),
        users: None,
        extra: None,
    };
    let json = match serde_json::to_string(&out) {
        Ok(j) => j,
        Err(_) => return,
    };

    // First try direct delivery
    let delivered = connected_users::send_to_user(connected, target_user.id, &json).await;
    if delivered > 0 {
        // Echo back to sender so they see their own PM
        if sender_id != target_user.id {
            let _ = sender_direct_tx.send(Message::text(json));
        }
        return;
    }

    let who_probe = WsOutgoing {
        msg_type: OutgoingType::Who,
        username: "system".to_string(),
        content: format!("looking for user '{}'", to_username),
        to_username: Some(to_username.clone()),
        users: None,
        extra: None,
    };
    if let Ok(who_json) = serde_json::to_string(&who_probe) {
        // Log the probe (in a multi-server setup this would go to other instances)
        println!("[WHO probe] {}", who_json);
    }

    // Wait briefly for the user to potentially appear
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Re-check after timeout
    let delivered = connected_users::send_to_user(connected, target_user.id, &json).await;
    if delivered > 0 {
        if sender_id != target_user.id {
            let _ = sender_direct_tx.send(Message::text(json));
        }
    } else {
        // Void the message — user never appeared
        send_error(
            sender_direct_tx,
            &format!("User '{}' is not reachable. Message voided.", to_username),
        );
    }
}

/// Ephemeral: broadcast (not saved to DB) and forward any extra metadata
/// for client-to-client custom communications.
fn handle_ephemeral(
    tx: &broadcast::Sender<String>,
    username: &str,
    content: &str,
    extra: Option<serde_json::Value>,
) {
    let out = WsOutgoing {
        msg_type: OutgoingType::Ephemeral,
        username: username.to_string(),
        content: content.to_string(),
        to_username: None,
        users: None,
        extra,
    };
    if let Ok(json) = serde_json::to_string(&out) {
        let _ = tx.send(json);
    }
}