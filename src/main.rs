mod cleanup;
mod config;
mod db;
mod handlers;
mod middleware;
mod utils;

use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;

use crate::cleanup::start_cleanup_task;
use crate::config::{load_config, Config};
use crate::db::init_db;
use crate::handlers::{create_paste, get_paste, raw_paste, spa_fallback};
use crate::middleware::{cache_control_middleware, ip_rate_limit_middleware};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub config: Config,
    pub ip_limits: Arc<
        tokio::sync::Mutex<std::collections::HashMap<std::net::IpAddr, Vec<std::time::Instant>>>,
    >,
}

#[tokio::main]
async fn main() {
    // Load config
    let config = load_config();

    // Setup database connection pool
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| config.database.url.clone());
    println!("Info | Connecting to database: {}...", db_url);
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .connect(&db_url)
        .await
        .expect(
            "Error | Failed to connect to database. Make sure Postgres is running and accessible.",
        );

    // Run table initialization
    init_db(&pool, &config)
        .await
        .expect("Error | Database schema initialization failed");

    // Setup sharing state
    let state = AppState {
        pool: pool.clone(),
        config: config.clone(),
        ip_limits: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
    };

    // Start database cleanup scheduler
    start_cleanup_task(
        pool.clone(),
        config.paste.cleanup_interval_seconds,
        state.ip_limits.clone(),
    );

    // Configure static directories and SPA index.html fallbacks (with 200 OK status for SPA routes)
    let serve_dir = ServeDir::new("src/static").fallback(axum::routing::any(spa_fallback));

    // Build the Axum router
    let mut app = Router::new()
        .route("/api/paste", post(create_paste))
        .route("/api/paste/:id", get(get_paste))
        .route("/raw/:id", get(raw_paste))
        .fallback_service(serve_dir)
        .layer(CompressionLayer::new())
        .layer(axum::middleware::from_fn(cache_control_middleware))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            ip_rate_limit_middleware,
        ))
        .layer(axum::extract::DefaultBodyLimit::max(
            config.paste.max_length,
        ))
        .with_state(state.clone());

    if config.rate_limit.enabled {
        app = app.layer(tower::limit::ConcurrencyLimitLayer::new(
            config.rate_limit.max_concurrent_requests,
        ));
    }

    // Bind and start the web server
    let host = config
        .server
        .host
        .parse::<std::net::IpAddr>()
        .unwrap_or([0, 0, 0, 0].into());
    let addr = SocketAddr::new(host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    println!("Info | RPS - Web Server listening on: http://{}", addr);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
