use crate::connected_users::ConnectedUsers;
use crate::tables::user_db::{
    CreateUserError, confirm_user_id, create_session, create_user, delete_session,
};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use warp::Filter;

#[derive(serde::Deserialize)]
pub struct LimitMessages {
    pub limit: i32,
}

#[derive(serde::Deserialize)]
pub struct UserID {
    pub id: i32,
}
#[derive(serde::Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(serde::Deserialize)]
pub struct SessionIdsRequest {
    pub session_ids: Option<Vec<String>>,
}

#[derive(serde::Deserialize)]
pub struct UserIdsRequest {
    pub user_ids: Vec<i32>,
}

#[derive(serde::Deserialize)]
pub struct UsernamePrefixRequest {
    pub prefix: String,
}

#[derive(serde::Serialize)]
pub struct AuthResponse {
    pub message: String,
    pub session_token: String,
    pub username: String,
}

#[derive(serde::Serialize)]
pub struct MeResponse {
    pub valid: bool,
    pub session_token: Option<String>,
    pub username: Option<String>,
}

pub fn login_route(
    pool: sqlx::MySqlPool,
    session_cache: Arc<RwLock<HashSet<String>>>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("api")
        .and(warp::path("login"))
        .and(warp::post()) // Intercept only POST requests
        .and(warp::body::json()) // Automatically parse JSON into LoginRequest
        .and(warp::any().map(move || pool.clone())) // Inject the database pool
        .and(warp::any().map(move || session_cache.clone()))
        .and_then(handle_login) // Pass the data to your logic function
}

pub async fn handle_login(
    auth: LoginRequest,
    pool: sqlx::MySqlPool,
    session_cache: Arc<RwLock<HashSet<String>>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let user_result = crate::tables::user_db::find_user_by_username(&pool, &auth.username).await;

    if let Ok(user) = user_result {
        if crate::tables::user_db::verify_password(&auth.password, &user.password_hash).is_ok() {
            let user_id = user.id;
            match create_session(&pool, user_id).await {
                Ok(token) => {
                    // Add to cache
                    {
                        let mut cache = session_cache.write().await;
                        cache.insert(token.clone());
                    }

                    let cookie = format!(
                        "session_token={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=604800",
                        token
                    );
                    return Ok(warp::reply::with_header(
                        warp::reply::with_status(
                            warp::reply::json(&AuthResponse {
                                message: "Login successful!".to_string(),
                                session_token: token,                                username: user.username.clone(),                            }),
                            warp::http::StatusCode::OK,
                        ),
                        "Set-Cookie",
                        cookie,
                    ));
                }
                Err(_) => {
                    return Ok(warp::reply::with_header(
                        warp::reply::with_status(
                            warp::reply::json(&"Failed to create session"),
                            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                        ),
                        "Set-Cookie",
                        "",
                    ));
                }
            }
        }
    }

    // Default failure case
    Ok(warp::reply::with_header(
        warp::reply::with_status(
            warp::reply::json(&"Invalid username or password"),
            warp::http::StatusCode::UNAUTHORIZED,
        ),
        "Set-Cookie",
        "",
    ))
}

pub fn register_route(
    pool: sqlx::MySqlPool,
    session_cache: Arc<RwLock<HashSet<String>>>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("api")
        .and(warp::path("register"))
        .and(warp::post()) // Intercept only POST requests
        .and(warp::body::json()) // Automatically parse JSON into LoginRequest
        .and(warp::any().map(move || pool.clone())) // Inject the database pool
        .and(warp::any().map(move || session_cache.clone()))
        .and_then(handle_register) // Pass the data to your logic function
}

pub async fn handle_register(
    auth: LoginRequest,
    pool: sqlx::MySqlPool,
    session_cache: Arc<RwLock<HashSet<String>>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match create_user(&pool, &auth.username, &auth.password).await {
        Ok(_) => {
            // Fetch the user to get the ID
            if let Ok(user) =
                crate::tables::user_db::find_user_by_username(&pool, &auth.username).await
            {
                let user_id = user.id;
                match create_session(&pool, user_id).await {
                    Ok(token) => {
                        // Add to cache
                        {
                            let mut cache = session_cache.write().await;
                            cache.insert(token.clone());
                        }

                        let cookie = format!(
                            "session_token={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=604800",
                            token
                        );
                        Ok(warp::reply::with_header(
                            warp::reply::with_status(
                                warp::reply::json(&AuthResponse {
                                    message: "Registered successfully".to_string(),
                                    session_token: token,
                                    username: auth.username.clone(),
                                }),
                                warp::http::StatusCode::OK,
                            ),
                            "Set-Cookie",
                            cookie,
                        ))
                    }
                    Err(_) => Ok(warp::reply::with_header(
                        warp::reply::with_status(
                            warp::reply::json(&"Registered but failed to create session"),
                            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                        ),
                        "Set-Cookie",
                        "",
                    )),
                }
            } else {
                Ok(warp::reply::with_header(
                    warp::reply::with_status(
                        warp::reply::json(&"Registered but failed to fetch ID"),
                        warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                    ),
                    "Set-Cookie",
                    "",
                ))
            }
        }
        Err(CreateUserError::UsernameTaken) => Ok(warp::reply::with_header(
            warp::reply::with_status(
                warp::reply::json(&"Username already taken"),
                warp::http::StatusCode::CONFLICT,
            ),
            "Set-Cookie",
            "",
        )),
        Err(CreateUserError::DatabaseError(error)) => {
            println!("error {:?} trying to register a user", error);
            Ok(warp::reply::with_header(
                warp::reply::with_status(
                    warp::reply::json(&"Database error"),
                    warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                ),
                "Set-Cookie",
                "",
            ))
        }
    }
}

pub fn get_chat_history(
    pool: sqlx::MySqlPool,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("api")
        .and(warp::path("get_chat_history"))
        .and(warp::get()) // Intercept only GET requests
        .and(warp::query::query())
        .and(warp::any().map(move || pool.clone())) // Inject the database pool
        .and_then(handle_chat_history) // Pass the data to your logic function
}

pub async fn handle_chat_history(
    limit: LimitMessages,
    pool: sqlx::MySqlPool,
) -> Result<impl warp::Reply, warp::Rejection> {
    let chat_history = crate::tables::user_db::get_chat_history(&pool, limit.limit).await;

    match chat_history {
        Ok(messages_vector) => Ok(warp::reply::with_status(
            warp::reply::json(&messages_vector),
            warp::http::StatusCode::OK,
        )),
        _ => Ok(warp::reply::with_status(
            warp::reply::json(&"Database is not online, please try again later"),
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

pub fn get_me_route(
    session_cache: Arc<RwLock<HashSet<String>>>,
    pool: sqlx::MySqlPool,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("api")
        .and(warp::path("me"))
        .and(warp::get())
        .and(warp::query::query::<UserID>().map(Some).or(warp::any().map(|| None)).unify())
        .and(warp::header::optional::<String>("cookie"))
        .and(warp::any().map(move || session_cache.clone()))
        .and(warp::any().map(move || pool.clone()))
        .and_then(handle_get_me)
}

pub async fn handle_get_me(
    user_id: Option<UserID>,
    cookie_header: Option<String>,
    session_cache: Arc<RwLock<HashSet<String>>>,
    pool: sqlx::MySqlPool,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut session_token = None;
    let mut username = None;
    if let Some(cookie_str) = cookie_header {
        if let Some(token) = extract_session_token(&cookie_str) {
            let cache = session_cache.read().await;
            if cache.contains(&token) {
                drop(cache);
                // Fetch username from database
                if let Ok(user) = crate::tables::user_db::get_user_by_token(&pool, &token).await {
                    session_token = Some(token);
                    username = Some(user.username);
                }
            }
        }
    } else if let Some(id_value) = user_id {
        if confirm_user_id(&pool, id_value.id).await.is_ok() {
            return Ok(warp::reply::with_status(
                warp::reply::json(&MeResponse {
                    valid: true,
                    session_token: None,
                    username: None,
                }),
                warp::http::StatusCode::OK,
            ));
        }
    }

    if let Some(token) = session_token {
        Ok(warp::reply::with_status(
            warp::reply::json(&MeResponse {
                valid: true,
                session_token: Some(token),
                username,
            }),
            warp::http::StatusCode::OK,
        ))
    } else {
        Ok(warp::reply::with_status(
            warp::reply::json(&MeResponse {
                valid: false,
                session_token: None,
                username: None,
            }),
            warp::http::StatusCode::UNAUTHORIZED,
        ))
    }
}

pub fn logout_route(
    pool: sqlx::MySqlPool,
    session_cache: Arc<RwLock<HashSet<String>>>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("api")
        .and(warp::path("logout"))
        .and(warp::post())
        .and(warp::header::optional::<String>("cookie"))
        .and(warp::any().map(move || pool.clone()))
        .and(warp::any().map(move || session_cache.clone()))
        .and_then(handle_logout)
}

pub async fn handle_logout(
    cookie_header: Option<String>,
    pool: sqlx::MySqlPool,
    session_cache: Arc<RwLock<HashSet<String>>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    if let Some(cookie_str) = cookie_header {
        if let Some(token) = extract_session_token(&cookie_str) {
            let _ = delete_session(&pool, &token).await;
            // Remove from cachea
            let mut cache = session_cache.write().await;
            cache.remove(&token);
        }
    }

    let cookie = "session_token=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0";
    Ok(warp::reply::with_header(
        warp::reply::with_status(warp::reply::json(&"Logged out"), warp::http::StatusCode::OK),
        "Set-Cookie",
        cookie,
    ))
}

pub fn get_online_users_route(
    pool: sqlx::MySqlPool,
    connected: ConnectedUsers,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("api")
        .and(warp::path("online_users"))
        .and(warp::get())
        .and(warp::any().map(move || pool.clone()))
        .and(warp::any().map(move || connected.clone()))
        .and_then(handle_get_online_users)
}

pub async fn handle_get_online_users(
    pool: sqlx::MySqlPool,
    connected: ConnectedUsers,
) -> Result<impl warp::Reply, warp::Rejection> {
    let user_ids = crate::connected_users::get_online_user_ids(&connected).await;
    let usernames = crate::tables::user_db::get_usernames_by_ids(&pool, &user_ids)
        .await
        .unwrap_or_default();
    Ok(warp::reply::json(&usernames))
}

pub fn get_user_ids_by_sessions_route(
    pool: sqlx::MySqlPool,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("api")
        .and(warp::path("user_ids_by_sessions"))
        .and(warp::post())
        .and(warp::header::optional::<String>("cookie"))
        .and(warp::body::json())
        .and(warp::any().map(move || pool.clone()))
        .and_then(handle_get_user_ids_by_sessions)
}

pub async fn handle_get_user_ids_by_sessions(
    cookie_header: Option<String>,
    request: SessionIdsRequest,
    pool: sqlx::MySqlPool,
) -> Result<impl warp::Reply, warp::Rejection> {
    let session_ids = if let Some(ids) = request.session_ids {
        if ids.is_empty() {
            // Use cookie token
            if let Some(cookie_str) = cookie_header {
                if let Some(token) = extract_session_token(&cookie_str) {
                    vec![token]
                } else {
                    return Ok(warp::reply::json(&Vec::<Option<i32>>::new()));
                }
            } else {
                return Ok(warp::reply::json(&Vec::<Option<i32>>::new()));
            }
        } else {
            ids
        }
    } else {
        // Use cookie token
        if let Some(cookie_str) = cookie_header {
            if let Some(token) = extract_session_token(&cookie_str) {
                vec![token]
            } else {
                return Ok(warp::reply::json(&Vec::<Option<i32>>::new()));
            }
        } else {
            return Ok(warp::reply::json(&Vec::<Option<i32>>::new()));
        }
    };

    let mut user_ids = Vec::new();
    for session_id in session_ids {
        if let Ok(user) = crate::tables::user_db::get_user_by_token(&pool, &session_id).await {
            user_ids.push(Some(user.id));
        } else {
            user_ids.push(None);
        }
    }

    Ok(warp::reply::json(&user_ids))
}

pub fn get_usernames_by_ids_route(
    pool: sqlx::MySqlPool,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("api")
        .and(warp::path("usernames_by_ids"))
        .and(warp::post())
        .and(warp::body::json())
        .and(warp::any().map(move || pool.clone()))
        .and_then(handle_get_usernames_by_ids)
}

pub async fn handle_get_usernames_by_ids(
    request: UserIdsRequest,
    pool: sqlx::MySqlPool,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut usernames = Vec::new();
    for &user_id in &request.user_ids {
        if let Ok(row) = sqlx::query_as!(
            crate::tables::user_db::User,
            "SELECT id, username, password_hash, created_at FROM app_users WHERE id = ?",
            user_id
        )
        .fetch_one(&pool)
        .await
        {
            usernames.push(Some(row.username));
        } else {
            usernames.push(None);
        }
    }

    Ok(warp::reply::json(&usernames))
}

pub fn get_usernames_by_prefix_route(
    pool: sqlx::MySqlPool,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("api")
        .and(warp::path("usernames_by_prefix"))
        .and(warp::post())
        .and(warp::body::json())
        .and(warp::any().map(move || pool.clone()))
        .and_then(handle_get_usernames_by_prefix)
}

pub async fn handle_get_usernames_by_prefix(
    request: UsernamePrefixRequest,
    pool: sqlx::MySqlPool,
) -> Result<impl warp::Reply, warp::Rejection> {
    if request.prefix.is_empty() {
        return Ok(warp::reply::json(&Vec::<String>::new()));
    }

    let pattern = format!("{}%", request.prefix.to_lowercase());
    let rows = sqlx::query!(
        "SELECT username FROM app_users WHERE LOWER(username) LIKE ? ORDER BY username LIMIT 10",
        pattern
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let usernames: Vec<String> = rows.into_iter().map(|r| r.username).collect();
    Ok(warp::reply::json(&usernames))
}

fn extract_session_token(cookie_str: &str) -> Option<String> {
    for cookie in cookie_str.split(';') {
        let parts: Vec<&str> = cookie.trim().splitn(2, '=').collect();
        if parts.len() == 2 && parts[0] == "session_token" {
            return Some(parts[1].to_string());
        }
    }
    None
}
