use chrono::{DateTime, Utc};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::env;

use crate::db::secrets::get_secret;

mod secrets;

#[derive(sqlx::FromRow, Debug)]
pub struct ChatMessage {
    pub message_id: i64,
    pub user_id: i32,
    pub username: String,
    pub content: String,
    pub created_at: std::option::Option<DateTime<Utc>>,
}

impl serde::Serialize for ChatMessage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(serde::Serialize)]
        struct SerializedChatMessage {
            username: String,
            content: String,
        }
        let message = SerializedChatMessage {
            username: self.username.clone(),
            content: self.content.clone(),
        };
        message.serialize(serializer)
    }
}


pub async fn create_pool() -> Result<MySqlPool, sqlx::Error> {
    let database_url = get_secret(env::var("DATABASE_URL_NAME")
        .expect("The name of the secret containing the full database url must be passed").as_str());

    MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
}