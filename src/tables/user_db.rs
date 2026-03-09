use argon2::{
    Argon2, PasswordVerifier,
    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::db::ChatMessage;

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: i32,          // maps to INT
    pub username: String, // maps to VARCHAR
    #[serde(skip_serializing)]
    pub password_hash: String, // maps to VARCHAR
    pub created_at: DateTime<Utc>, // maps to TIMESTAMP
}

pub async fn get_chat_history(
    pool: &sqlx::MySqlPool,
    limit: i32,
) -> Result<Vec<ChatMessage>, sqlx::Error> {
    sqlx::query_as!(
        ChatMessage,
        r#"
        SELECT 
            m.id as message_id,
            m.user_id,
            u.username, 
            m.content, 
            m.created_at,
            NULL as `session_id: String`
        FROM messages m
        JOIN app_users u ON m.user_id = u.id
        ORDER BY m.created_at ASC
        LIMIT ?
        "#,
        limit
    )
    .fetch_all(pool)
    .await
}

pub async fn find_user_by_username(
    pool: &sqlx::MySqlPool,
    name: &str,
) -> Result<User, sqlx::Error> {
    sqlx::query_as!(
        User,
        "SELECT id, username, password_hash, created_at FROM app_users WHERE username = ?",
        name
    )
    .fetch_one(pool)
    .await
}

pub fn hash_password(password: &str) -> String {
    let salt = SaltString::generate(OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .expect("Error hashing password")
        .to_string()
}

pub fn verify_password(password: &str, hashstr: &str) -> Result<(), argon2::password_hash::Error> {
    let argon2 = Argon2::default();
    let hash = match argon2::PasswordHash::parse(hashstr, argon2::password_hash::Encoding::B64) {
        Ok(parsed_hash) => {parsed_hash},
        Err(_e) => {
            print!("error: couldnt parse hash: {:?}",hashstr);
            return Err(argon2::password_hash::Error::PhcStringField);
        }
    };
    argon2.verify_password(password.as_bytes(), &hash)
}

pub async fn create_user(
    pool: &sqlx::MySqlPool,
    username: &str,
    raw_password: &str,
) -> Result<u64, sqlx::Error> {
    // 1. Hash the password using the function we talked about earlier
    let hashed_password = hash_password(raw_password);

    // 2. Insert into the database
    let result = sqlx::query!(
        r#"
        INSERT INTO app_users (username, password_hash)
        VALUES (?, ?)
        "#,
        username,
        hashed_password
    )
    .execute(pool)
    .await?;

    // Returns the number of rows affected (should be 1)
    Ok(result.rows_affected())
}

pub async fn save_message(
    pool: &sqlx::MySqlPool,
    user_id: i32,
    content: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO messages (user_id, content) VALUES (?, ?)",
        user_id,
        content
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn create_session(
    pool: &sqlx::MySqlPool,
    user_id: i32,
) -> Result<String, sqlx::Error> {
    let token = uuid::Uuid::new_v4().to_string();
    let expires_at = Utc::now() + chrono::Duration::days(7);

    sqlx::query!(
        "INSERT INTO sessions (token, user_id, expires_at) VALUES (?, ?, ?)",
        token,
        user_id,
        expires_at
    )
    .execute(pool)
    .await?;

    Ok(token)
}

pub async fn delete_session(
    pool: &sqlx::MySqlPool,
    token: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM sessions WHERE token = ?",
        token
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn _confirm_user_id(
    pool: &sqlx::MySqlPool,
    user_id: i32,
    username: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "SELECT * FROM app_users WHERE username=? AND id=?",
        username,
        user_id
    )
    .fetch_one(pool)
    .await?;
    Ok(())
}

pub async fn cleanup_expired_sessions(
    pool: &sqlx::MySqlPool,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query!(
        "DELETE FROM sessions WHERE expires_at < ?",
        Utc::now()
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

pub async fn get_all_valid_sessions(
    pool: &sqlx::MySqlPool,
) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query!(
        "SELECT token FROM sessions WHERE expires_at > ?",
        Utc::now()
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.token).collect())
}

pub async fn get_user_by_token(
    pool: &sqlx::MySqlPool,
    token: &str,
) -> Result<User, sqlx::Error> {
    sqlx::query_as!(
        User,
        r#"
        SELECT u.id, u.username, u.password_hash, u.created_at
        FROM app_users u
        JOIN sessions s ON u.id = s.user_id
        WHERE s.token = ? AND s.expires_at > ?
        "#,
        token,
        Utc::now()
    )
    .fetch_one(pool)
    .await
}

/// Resolve a list of user IDs to their usernames.
pub async fn get_usernames_by_ids(
    pool: &sqlx::MySqlPool,
    ids: &[i32],
) -> Result<Vec<String>, sqlx::Error> {
    let mut usernames = Vec::new();
    for &uid in ids {
        if let Ok(row) = sqlx::query_as!(
            User,
            "SELECT id, username, password_hash, created_at FROM app_users WHERE id = ?",
            uid
        )
        .fetch_one(pool)
        .await
        {
            usernames.push(row.username);
        }
    }
    Ok(usernames)
}