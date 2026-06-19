use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{Duration, Utc};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tower_http::services::{ServeDir, ServeFile};

/// Server configurations
#[derive(Deserialize, Clone, Debug)]
struct Config {
    server: ServerConfig,
    database: DatabaseConfig,
    paste: PasteConfig,
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

/// Shared application state
#[derive(Clone)]
struct AppState {
    pool: sqlx::PgPool,
    config: Config,
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
    }
}

/// Starts an asynchronous background task to periodically delete expired pastes.
fn start_cleanup_task(pool: sqlx::PgPool, interval_seconds: u64) {
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
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS pastes_content_md5_idx ON pastes (md5(content))"
    )
    .execute(&pool)
    .await
    .expect("Error | Failed to create md5 content index");
    println!("Success | Database schema initialized");

    // Start database cleanup scheduler
    start_cleanup_task(pool.clone(), config.paste.cleanup_interval_seconds);

    // Setup sharing state
    let state = AppState {
        pool,
        config: config.clone(),
    };

    // Configure static directories and SPA index.html fallbacks
    let serve_dir =
        ServeDir::new("src/static").not_found_service(ServeFile::new("src/static/index.html"));

    // Build the Axum router
    let app = Router::new()
        .route("/api/paste", post(create_paste))
        .route("/api/paste/:id", get(get_paste))
        .route("/raw/:id", get(raw_paste))
        .fallback_service(serve_dir)
        .layer(axum::extract::DefaultBodyLimit::max(config.paste.max_length))
        .with_state(state);

    // Bind and start the web server
    let host = config
        .server
        .host
        .parse::<std::net::IpAddr>()
        .unwrap_or([0, 0, 0, 0].into());
    let addr = SocketAddr::new(host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    println!("Info | RPS - Web Server listening on: http://{}", addr);
    axum::serve(listener, app).await.unwrap();
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
        "uuid" => {
            uuid::Uuid::new_v4().to_string()
        }
        "alphanumeric" | _ => {
            thread_rng()
                .sample_iter(&Alphanumeric)
                .take(config.id_length)
                .map(char::from)
                .collect()
        }
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
            println!("Info | Exact duplicate content found. Redirecting to existing paste '{}'.", existing_id);
            if state.config.paste.extend_expiry_on_read {
                let new_expires_at = Utc::now() + Duration::days(state.config.paste.default_expiry_days);
                let _ = sqlx::query("UPDATE pastes SET expires_at = $1 WHERE id = $2")
                    .bind(new_expires_at)
                    .bind(&existing_id)
                    .execute(&state.pool)
                    .await;
            }
            return (StatusCode::OK, Json(CreatePasteResponse { id: existing_id })).into_response();
        }
    }

    // Calculate expiry based on config
    let expires_at = Utc::now() + Duration::days(state.config.paste.default_expiry_days);

    let mut retries = 0;
    let max_retries = 10;

    loop {
        let id = generate_id(&state.config.paste);

        let result = sqlx::query("INSERT INTO pastes (id, content, expires_at) VALUES ($1, $2, $3)")
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
                return (StatusCode::INTERNAL_SERVER_ERROR, "Database save failure").into_response();
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
