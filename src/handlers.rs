use crate::utils::generate_id;
use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{Duration, Utc};
use magic_crypt::MagicCryptTrait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Request body for creating a paste
#[derive(Deserialize)]
pub struct CreatePaste {
    pub content: String,
    pub password: Option<String>,
}

/// Response returned when a paste is created successfully
#[derive(Serialize)]
pub struct CreatePasteResponse {
    pub id: String,
}

/// Database row mapping structure
#[derive(sqlx::FromRow)]
pub struct PasteRow {
    pub content: String,
}

#[derive(sqlx::FromRow)]
pub struct PasteRowWithPassword {
    pub content: String,
    pub password_hash: Option<String>,
}

/// Endpoint handler to create a new paste: POST /api/paste
pub async fn create_paste(
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

    // Calculate password hash if enabled and provided
    let raw_password = payload.password.as_deref().filter(|p| !p.trim().is_empty());

    if raw_password.is_some() && !state.config.security.password_protection_enabled {
        return (
            StatusCode::BAD_REQUEST,
            "Password protection is disabled on this server.",
        )
            .into_response();
    }

    let password_hash = if state.config.security.password_protection_enabled {
        raw_password.map(|p| {
            bcrypt::hash(p, bcrypt::DEFAULT_COST).expect("Error | Hashing password failed")
        })
    } else {
        None
    };

    // Encrypt content if password protection and encryption are enabled
    let final_content = if state.config.security.password_protection_enabled
        && state.config.security.encryption_enabled
    {
        if let Some(password) = raw_password {
            let mc = magic_crypt::new_magic_crypt!(password, 256);
            mc.encrypt_str_to_base64(&payload.content)
        } else {
            payload.content.clone()
        }
    } else {
        payload.content.clone()
    };

    // Check if a paste with the exact same content already exists and is not expired (if enabled)
    // We only redirect to duplicates if the new paste has no password protection.
    if state.config.paste.redirect_to_duplicate && password_hash.is_none() {
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

        let result = if state.config.security.password_protection_enabled {
            sqlx::query("INSERT INTO pastes (id, content, expires_at, password_hash) VALUES ($1, $2, $3, $4)")
                .bind(&id)
                .bind(&final_content)
                .bind(expires_at)
                .bind(&password_hash)
                .execute(&state.pool)
                .await
        } else {
            sqlx::query("INSERT INTO pastes (id, content, expires_at) VALUES ($1, $2, $3)")
                .bind(&id)
                .bind(&final_content)
                .bind(expires_at)
                .execute(&state.pool)
                .await
        };

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
pub async fn get_paste(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let provided_password = params.get("password").cloned().or_else(|| {
        headers
            .get("x-paste-password")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string())
    });

    // Strip file extension if present (e.g. "abc12345.rs" -> "abc12345")
    let clean_id = match id.split_once('.') {
        Some((prefix, _)) => prefix.to_string(),
        None => id,
    };

    let (content, password_hash) = if state.config.security.password_protection_enabled {
        let result = sqlx::query_as::<_, PasteRowWithPassword>(
            "SELECT content, password_hash FROM pastes WHERE id = $1 AND expires_at > $2",
        )
        .bind(&clean_id)
        .bind(Utc::now())
        .fetch_optional(&state.pool)
        .await;

        match result {
            Ok(Some(row)) => (row.content, row.password_hash),
            Ok(None) => {
                return (StatusCode::NOT_FOUND, "Paste not found or has expired").into_response()
            }
            Err(e) => {
                eprintln!("Error | Database query failure: {:?}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Database query error").into_response();
            }
        }
    } else {
        let result = sqlx::query_as::<_, PasteRow>(
            "SELECT content FROM pastes WHERE id = $1 AND expires_at > $2",
        )
        .bind(&clean_id)
        .bind(Utc::now())
        .fetch_optional(&state.pool)
        .await;

        match result {
            Ok(Some(row)) => (row.content, None),
            Ok(None) => {
                return (StatusCode::NOT_FOUND, "Paste not found or has expired").into_response()
            }
            Err(e) => {
                eprintln!("Error | Database query failure: {:?}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Database query error").into_response();
            }
        }
    };

    // Verify password if protected
    let is_protected = password_hash.is_some();
    if let Some(ref hash) = password_hash {
        let is_valid = match &provided_password {
            Some(password) => bcrypt::verify(password, hash).unwrap_or(false),
            None => false,
        };

        if !is_valid {
            return (StatusCode::UNAUTHORIZED, "Password required").into_response();
        }
    }

    // Decrypt content if password protection and encryption are enabled
    let decrypted_content = if state.config.security.password_protection_enabled
        && state.config.security.encryption_enabled
        && is_protected
    {
        if let Some(ref password) = provided_password {
            let mc = magic_crypt::new_magic_crypt!(password, 256);
            match mc.decrypt_base64_to_string(&content) {
                Ok(plain) => plain,
                Err(e) => {
                    eprintln!("Error | Decryption failure: {:?}", e);
                    content
                }
            }
        } else {
            content
        }
    } else {
        content
    };

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
            "content": decrypted_content,
            "language": None::<String>
        })),
    )
        .into_response()
}

/// Endpoint handler to get raw paste text: GET /raw/:id
pub async fn raw_paste(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let provided_password = params.get("password").cloned().or_else(|| {
        headers
            .get("x-paste-password")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string())
    });

    let clean_id = match id.split_once('.') {
        Some((prefix, _)) => prefix.to_string(),
        None => id,
    };

    let (content, password_hash) = if state.config.security.password_protection_enabled {
        let result = sqlx::query_as::<_, PasteRowWithPassword>(
            "SELECT content, password_hash FROM pastes WHERE id = $1 AND expires_at > $2",
        )
        .bind(&clean_id)
        .bind(Utc::now())
        .fetch_optional(&state.pool)
        .await;

        match result {
            Ok(Some(row)) => (row.content, row.password_hash),
            Ok(None) => {
                return (StatusCode::NOT_FOUND, "Paste not found or has expired").into_response()
            }
            Err(e) => {
                eprintln!("Error | Database query failure: {:?}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Database query error").into_response();
            }
        }
    } else {
        let result = sqlx::query_as::<_, PasteRow>(
            "SELECT content FROM pastes WHERE id = $1 AND expires_at > $2",
        )
        .bind(&clean_id)
        .bind(Utc::now())
        .fetch_optional(&state.pool)
        .await;

        match result {
            Ok(Some(row)) => (row.content, None),
            Ok(None) => {
                return (StatusCode::NOT_FOUND, "Paste not found or has expired").into_response()
            }
            Err(e) => {
                eprintln!("Error | Database query failure: {:?}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Database query error").into_response();
            }
        }
    };

    // Verify password if protected
    let is_protected = password_hash.is_some();
    if let Some(ref hash) = password_hash {
        let is_valid = match &provided_password {
            Some(password) => bcrypt::verify(password, hash).unwrap_or(false),
            None => false,
        };

        if !is_valid {
            return (StatusCode::UNAUTHORIZED, "Password required").into_response();
        }
    }

    // Decrypt content if password protection and encryption are enabled
    let decrypted_content = if state.config.security.password_protection_enabled
        && state.config.security.encryption_enabled
        && is_protected
    {
        if let Some(ref password) = provided_password {
            let mc = magic_crypt::new_magic_crypt!(password, 256);
            match mc.decrypt_base64_to_string(&content) {
                Ok(plain) => plain,
                Err(e) => {
                    eprintln!("Error | Decryption failure: {:?}", e);
                    content
                }
            }
        } else {
            content
        }
    } else {
        content
    };

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
    (StatusCode::OK, decrypted_content).into_response()
}

/// SPA fallback handler to serve index.html with a 200 OK status
pub async fn spa_fallback() -> impl IntoResponse {
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
