use serde::{Deserialize, Serialize};

/// What the client sends over WebSocket
#[derive(Debug, Deserialize)]
pub struct WsIncoming {
    #[serde(rename = "type")]
    pub msg_type: MessageType,
    pub metadata: IncomingMetadata,
    pub content: String,
    /// Arbitrary extra data — forwarded as-is on ephemeral messages
    /// for client-to-client custom communications
    #[serde(default)]
    pub extra: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum MessageType {
    Broadcast,
    Private,
    Ephemeral,
}

#[derive(Debug, Deserialize)]
pub struct IncomingMetadata {
    pub session_id: String,
    /// Target username — required for `Private` type
    pub to_username: Option<String>,
    /// Optional client-provided timestamp override
    pub sent_when_override: Option<String>,
}

/// What the server sends to clients
#[derive(Debug, Serialize, Clone)]
pub struct WsOutgoing {
    #[serde(rename = "type")]
    pub msg_type: OutgoingType,
    pub username: String,
    pub content: String,
    /// Present on private messages to indicate the recipient
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_username: Option<String>,
    /// Present on `who` responses (internal server probes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub users: Option<Vec<String>>,
    /// Forwarded extra data on ephemeral messages
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum OutgoingType {
    Broadcast,
    Private,
    Ephemeral,
    /// Internal server probe — not a user-facing message type
    Who,
    Error,
}
