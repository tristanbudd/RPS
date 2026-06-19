use axum::{
    extract::{ConnectInfo, Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{Duration, Utc};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;

/// Server configurations
#[derive(Deserialize, Clone, Debug)]
struct Config {
    server: ServerConfig,
    database: DatabaseConfig,
    paste: PasteConfig,
    rate_limit: RateLimitConfig,
}

#[derive(Deserialize, Clone, Debug)]
struct ServerConfig {
    host: String,
    port: u16,
}

#[derive(Deserialize, Clone, Debug)]
struct DatabaseConfig {
    url: String,
}

#[derive(Deserialize, Clone, Debug)]
struct PasteConfig {
    default_expiry_days: i64,
    extend_expiry_on_read: bool,
    cleanup_interval_seconds: u64,
    id_type: String,
    id_length: usize,
    redirect_to_duplicate: bool,
    max_length: usize,
}

#[derive(Deserialize, Clone, Debug)]
struct RateLimitConfig {
    enabled: bool,
    max_concurrent_requests: usize,
    requests_per_minute: usize,
}

/// Shared application state
#[derive(Clone)]
struct AppState {
    pool: sqlx::PgPool,
    config: Config,
    ip_limits: Arc<
        tokio::sync::Mutex<std::collections::HashMap<std::net::IpAddr, Vec<std::time::Instant>>>,
    >,
}

/// Request body for creating a paste
#[derive(Deserialize)]
struct CreatePaste {
    content: String,
}

/// Response returned when a paste is created successfully
#[derive(Serialize)]
struct CreatePasteResponse {
    id: String,
}

/// Database row mapping structure
#[derive(sqlx::FromRow)]
struct PasteRow {
    content: String,
}

/// Loads configuration from `config.toml` or falls back to environment defaults.
fn load_config() -> Config {
    if let Ok(content) = std::fs::read_to_string("config.toml") {
        if let Ok(cfg) = toml::from_str::<Config>(&content) {
            println!("Success | Configuration loaded successfully from config.toml");
            return cfg;
        }
    }
    println!("Info | config.toml not found or invalid; falling back to environment settings");
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/rps".to_string());
    Config {
        server: ServerConfig {
            host: "0.0.0.0".to_string(),
            port: 8000,
        },
        database: DatabaseConfig { url: db_url },
        paste: PasteConfig {
            default_expiry_days: 30,
            extend_expiry_on_read: true,
            cleanup_interval_seconds: 3600,
            id_type: "alphanumeric".to_string(),
            id_length: 8,
            redirect_to_duplicate: true,
            max_length: 5_000_000,
        },
        rate_limit: RateLimitConfig {
            enabled: true,
            max_concurrent_requests: 100,
            requests_per_minute: 300,
        },
    }
}

/// Starts an asynchronous background task to periodically delete expired pastes.
fn start_cleanup_task(
    pool: sqlx::PgPool,
    interval_seconds: u64,
    ip_limits: Arc<
        tokio::sync::Mutex<std::collections::HashMap<std::net::IpAddr, Vec<std::time::Instant>>>,
    >,
) {
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs(interval_seconds));
        loop {
            interval.tick().await;
            println!("Info | Background worker: Cleaning up expired pastes...");
            let now = Utc::now();
            match sqlx::query("DELETE FROM pastes WHERE expires_at < $1")
                .bind(now)
                .execute(&pool)
                .await
            {
                Ok(res) => {
                    println!(
                        "Success | Background worker: Deleted {} expired pastes.",
                        res.rows_affected()
                    );
                }
                Err(e) => {
                    eprintln!(
                        "Error | Background worker: Failed to delete expired pastes: {:?}",
                        e
                    );
                }
            }

            // Prune expired rate limiting IPs
            let now_instant = std::time::Instant::now();
            let mut limits = ip_limits.lock().await;
            limits.retain(|_, timestamps| {
                timestamps.retain(|&t| now_instant.duration_since(t).as_secs() < 60);
                !timestamps.is_empty()
            });
            println!("Success | Background worker: Pruned expired rate-limit IP records.");
        }
    });
}

#[tokio::main]
async fn main() {
    // Load config
    let config = load_config();

    // Setup database connection pool
    println!("Info | Connecting to database: {}...", config.database.url);
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database.url)
        .await
        .expect(
            "Error | Failed to connect to database. Make sure Postgres is running and accessible.",
        );

    // Run table initialization
    println!("Info | Initializing database schema...");
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pastes (
            id VARCHAR(64) PRIMARY KEY,
            content TEXT NOT NULL,
            created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
            expires_at TIMESTAMP WITH TIME ZONE NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .expect("Error | Failed to initialize pastes database table");

    // Create an expression index on md5(content) for fast duplicate content checks
    sqlx::query("CREATE INDEX IF NOT EXISTS pastes_content_md5_idx ON pastes (md5(content))")
        .execute(&pool)
        .await
        .expect("Error | Failed to create md5 content index");

    // Create an index on expires_at for fast background cleanup
    sqlx::query("CREATE INDEX IF NOT EXISTS pastes_expires_at_idx ON pastes (expires_at)")
        .execute(&pool)
        .await
        .expect("Error | Failed to create expires_at index");
    println!("Success | Database schema initialized");

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

/// Generates a unique paste ID based on the format specified in configuration
fn generate_id(config: &PasteConfig) -> String {
    match config.id_type.as_str() {
        "numeric" => {
            const CHARSET: &[u8] = b"0123456789";
            let mut rng = thread_rng();
            (0..config.id_length)
                .map(|_| {
                    let idx = rng.gen_range(0..CHARSET.len());
                    CHARSET[idx] as char
                })
                .collect()
        }
        "lowercase" => {
            const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
            let mut rng = thread_rng();
            (0..config.id_length)
                .map(|_| {
                    let idx = rng.gen_range(0..CHARSET.len());
                    CHARSET[idx] as char
                })
                .collect()
        }
        "uuid" => uuid::Uuid::new_v4().to_string(),
        _ => thread_rng()
            .sample_iter(&Alphanumeric)
            .take(config.id_length)
            .map(char::from)
            .collect(),
    }
}

/// Endpoint handler to create a new paste: POST /api/paste
async fn create_paste(
    State(state): State<AppState>,
    Json(payload): Json<CreatePaste>,
) -> impl IntoResponse {
    if payload.content.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "Content cannot be empty").into_response();
    }

    if payload.content.len() > state.config.paste.max_length {
        return (
            StatusCode::BAD_REQUEST,
            "Content exceeds the maximum configured length",
        )
            .into_response();
    }

    // Check if a paste with the exact same content already exists and is not expired (if enabled)
    if state.config.paste.redirect_to_duplicate {
        let existing: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM pastes WHERE md5(content) = md5($1) AND content = $1 AND expires_at > $2 LIMIT 1"
        )
        .bind(&payload.content)
        .bind(Utc::now())
        .fetch_optional(&state.pool)
        .await
        .unwrap_or(None);

        if let Some((existing_id,)) = existing {
            println!(
                "Info | Exact duplicate content found. Redirecting to existing paste '{}'.",
                existing_id
            );
            if state.config.paste.extend_expiry_on_read {
                let new_expires_at =
                    Utc::now() + Duration::days(state.config.paste.default_expiry_days);
                let _ = sqlx::query("UPDATE pastes SET expires_at = $1 WHERE id = $2")
                    .bind(new_expires_at)
                    .bind(&existing_id)
                    .execute(&state.pool)
                    .await;
            }
            return (
                StatusCode::OK,
                Json(CreatePasteResponse { id: existing_id }),
            )
                .into_response();
        }
    }

    // Calculate expiry based on config
    let expires_at = Utc::now() + Duration::days(state.config.paste.default_expiry_days);

    let mut retries = 0;
    let max_retries = 10;

    loop {
        let id = generate_id(&state.config.paste);

        let result =
            sqlx::query("INSERT INTO pastes (id, content, expires_at) VALUES ($1, $2, $3)")
                .bind(&id)
                .bind(&payload.content)
                .bind(expires_at)
                .execute(&state.pool)
                .await;

        match result {
            Ok(_) => {
                println!("Success | Saved paste '{}' successfully.", id);
                return (StatusCode::CREATED, Json(CreatePasteResponse { id })).into_response();
            }
            Err(e) => {
                if let Some(db_err) = e.as_database_error() {
                    // Check for unique key violation (PostgreSQL code 23505)
                    if db_err.code().as_deref() == Some("23505") {
                        retries += 1;
                        if retries < max_retries {
                            println!(
                                "Info | Duplicate ID '{}' detected. Retrying ID generation (attempt {}/{})...",
                                id, retries, max_retries
                            );
                            continue;
                        } else {
                            eprintln!(
                                "Error | Max retries reached ({}) trying to generate a unique paste ID.",
                                max_retries
                            );
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "Failed to generate unique paste ID",
                            )
                                .into_response();
                        }
                    }
                }
                eprintln!("Error | Failed to save paste to database: {:?}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Database save failure")
                    .into_response();
            }
        }
    }
}

/// Endpoint handler to get a paste JSON payload: GET /api/paste/:id
async fn get_paste(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    // Strip file extension if present (e.g. "abc12345.rs" -> "abc12345")
    let clean_id = match id.split_once('.') {
        Some((prefix, _)) => prefix.to_string(),
        None => id,
    };

    // Query active paste from DB
    let result = sqlx::query_as::<_, PasteRow>(
        "SELECT content FROM pastes WHERE id = $1 AND expires_at > $2",
    )
    .bind(&clean_id)
    .bind(Utc::now())
    .fetch_optional(&state.pool)
    .await;

    match result {
        Ok(Some(row)) => {
            // Re-extend expiration if extend_expiry_on_read is configured
            if state.config.paste.extend_expiry_on_read {
                let new_expires_at =
                    Utc::now() + Duration::days(state.config.paste.default_expiry_days);
                let _ = sqlx::query("UPDATE pastes SET expires_at = $1 WHERE id = $2")
                    .bind(new_expires_at)
                    .bind(&clean_id)
                    .execute(&state.pool)
                    .await;
                println!(
                    "Success | Extended expiration for paste '{}' by 30 days.",
                    clean_id
                );
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "content": row.content,
                    "language": None::<String>
                })),
            )
                .into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Paste not found or has expired").into_response(),
        Err(e) => {
            eprintln!("Error | Database query failure: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database query error").into_response()
        }
    }
}

/// Endpoint handler to get raw paste text: GET /raw/:id
async fn raw_paste(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let clean_id = match id.split_once('.') {
        Some((prefix, _)) => prefix.to_string(),
        None => id,
    };

    let result = sqlx::query_as::<_, PasteRow>(
        "SELECT content FROM pastes WHERE id = $1 AND expires_at > $2",
    )
    .bind(&clean_id)
    .bind(Utc::now())
    .fetch_optional(&state.pool)
    .await;

    match result {
        Ok(Some(row)) => {
            if state.config.paste.extend_expiry_on_read {
                let new_expires_at =
                    Utc::now() + Duration::days(state.config.paste.default_expiry_days);
                let _ = sqlx::query("UPDATE pastes SET expires_at = $1 WHERE id = $2")
                    .bind(new_expires_at)
                    .bind(&clean_id)
                    .execute(&state.pool)
                    .await;
                println!(
                    "Success | Extended expiration for paste '{}' by 30 days.",
                    clean_id
                );
            }
            (StatusCode::OK, row.content).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Paste not found or has expired").into_response(),
        Err(e) => {
            eprintln!("Error | Database query failure: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database query error").into_response()
        }
    }
}

/// Middleware to enforce per-IP rate limiting
async fn ip_rate_limit_middleware(
    State(state): State<AppState>,
    request: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    if !state.config.rate_limit.enabled {
        return Ok(next.run(request).await);
    }

    // 1. Get client IP
    let ip = get_client_ip(&request);

    // 2. Lock and update timestamps
    let now = std::time::Instant::now();
    let mut limits = state.ip_limits.lock().await;
    let timestamps = limits.entry(ip).or_insert_with(Vec::new);

    // Retain only requests within the last 60 seconds
    timestamps.retain(|&t| now.duration_since(t).as_secs() < 60);

    if timestamps.len() >= state.config.rate_limit.requests_per_minute {
        println!("Warning | Rate limit exceeded for IP: {}", ip);
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    timestamps.push(now);
    drop(limits);

    Ok(next.run(request).await)
}

/// Helper function to retrieve client IP supporting proxy headers
fn get_client_ip(request: &axum::http::Request<axum::body::Body>) -> std::net::IpAddr {
    // Check X-Forwarded-For header
    if let Some(forwarded_for) = request.headers().get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded_for.to_str() {
            if let Some(first_ip_str) = forwarded_str.split(',').next() {
                if let Ok(ip) = first_ip_str.trim().parse::<std::net::IpAddr>() {
                    return ip;
                }
            }
        }
    }

    // Check X-Real-IP header
    if let Some(real_ip) = request.headers().get("x-real-ip") {
        if let Ok(real_ip_str) = real_ip.to_str() {
            if let Ok(ip) = real_ip_str.trim().parse::<std::net::IpAddr>() {
                return ip;
            }
        }
    }

    // Fallback to peer address
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip())
        .unwrap_or_else(|| [127, 0, 0, 1].into())
}

/// SPA fallback handler to serve index.html with a 200 OK status
async fn spa_fallback() -> impl IntoResponse {
    match tokio::fs::read_to_string("src/static/index.html").await {
        Ok(html) => (
            StatusCode::OK,
            [("content-type", "text/html; charset=utf-8")],
            html,
        )
            .into_response(),
        Err(e) => {
            eprintln!("Error | Failed to read index.html: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
        }
    }
}

/// Middleware to add Cache-Control headers for static assets and page requests
async fn cache_control_middleware(
    request: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> impl IntoResponse {
    let path = request.uri().path().to_string();
    let mut response = next.run(request).await;

    let is_static_asset = path.ends_with(".css")
        || path.ends_with(".js")
        || path.ends_with(".svg")
        || path.ends_with(".png")
        || path.ends_with(".ico")
        || path.ends_with(".webmanifest")
        || path.ends_with(".woff")
        || path.ends_with(".woff2");

    if is_static_asset && response.status().is_success() {
        let headers = response.headers_mut();
        headers.insert(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
    } else if path == "/" || path.ends_with(".html") {
        let headers = response.headers_mut();
        headers.insert(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-cache, no-store, must-revalidate"),
        );
    }

    response
}
