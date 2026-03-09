use tokio::sync::{broadcast, RwLock};
use warp::Filter;
use std::collections::HashSet;
use std::sync::Arc;

use crate::{api::{login_route, register_route, get_chat_history, get_me_route, logout_route}, routes::ws_route};
//mod ~= namespace import
mod db;
mod api;
mod routes;
mod tables;
mod ws_handler;
mod ws_types;
mod connected_users;
//declare main thread runs this
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (tx, _rx) = broadcast::channel::<String>(100);
    let pool = db::create_pool().await?;

    // Session cache (in-memory HashSet of valid tokens)
    let session_cache = Arc::new(RwLock::new(HashSet::new()));

    // Initial cache population
    if let Ok(sessions) = crate::tables::user_db::get_all_valid_sessions(&pool).await {
        let mut cache = session_cache.write().await;
        for token in sessions {
            cache.insert(token);
        }
    }

    //ROUTES
    let login_route = login_route(pool.clone(), session_cache.clone());
    let register_route = register_route(pool.clone(), session_cache.clone());
    let chat_history_route = get_chat_history(pool.clone());
    let me_route = get_me_route(session_cache.clone());
    let logout_route = logout_route(pool.clone(), session_cache.clone());
    let connected_users = connected_users::new_registry();
    let ws_route = ws_route(pool.clone(), tx.clone(), session_cache.clone(), connected_users.clone());

    let total_route = ws_route.or(login_route).or(register_route).or(chat_history_route).or(me_route).or(logout_route);

    // Background task for session cleanup AND cache sync
    let pool_cleanup = pool.clone();
    let session_cache_cleanup = session_cache.clone();
    tokio::spawn(async move {
        loop {
            // Cleanup expired in DB
            let _ = crate::tables::user_db::cleanup_expired_sessions(&pool_cleanup).await;
            
            // Re-sync cache from DB
            if let Ok(sessions) = crate::tables::user_db::get_all_valid_sessions(&pool_cleanup).await {
                let mut cache = session_cache_cleanup.write().await;
                cache.clear();
                for token in sessions {
                    cache.insert(token);
                }
            }
            
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
        }
    });

    //Serve
    println!("\n\tNow serving server, setup successful\n\tDatabase connection engaged");
    warp::serve(total_route).run(([0, 0, 0, 0], 8000)).await;
    Ok(())
}
