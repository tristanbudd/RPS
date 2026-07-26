use crate::AppState;
use axum::{
    extract::{FromRef, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sysinfo::System;

#[derive(Clone, Debug)]
pub struct AdminSession {
    pub username: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

pub struct AdminUser {
    pub username: String,
}

impl<S> axum::extract::FromRequestParts<S> for AdminUser
where
    S: Send + Sync,
    crate::AppState: axum::extract::FromRef<S>,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let app_state = crate::AppState::from_ref(state);

        let cookie_header = parts
            .headers
            .get(axum::http::header::COOKIE)
            .and_then(|value| value.to_str().ok());

        let mut session_token = None;
        if let Some(cookie_str) = cookie_header {
            for cookie in cookie_str.split(';') {
                let parts: Vec<&str> = cookie.trim().split('=').collect();
                if parts.len() == 2 && parts[0] == "admin_session" {
                    session_token = Some(parts[1].to_string());
                    break;
                }
            }
        }

        let token = match session_token {
            Some(t) => t,
            None => return Err((StatusCode::UNAUTHORIZED, "Unauthorized: No session")),
        };

        let sessions = app_state.admin_sessions.lock().await;
        if let Some(session) = sessions.get(&token) {
            if session.expires_at > chrono::Utc::now() {
                return Ok(AdminUser {
                    username: session.username.clone(),
                });
            }
        }

        Err((
            StatusCode::UNAUTHORIZED,
            "Unauthorized: Invalid or expired session",
        ))
    }
}

impl<S> axum::extract::OptionalFromRequestParts<S> for AdminUser
where
    S: Send + Sync,
    crate::AppState: axum::extract::FromRef<S>,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        let app_state = crate::AppState::from_ref(state);

        let cookie_header = parts
            .headers
            .get(axum::http::header::COOKIE)
            .and_then(|value| value.to_str().ok());

        let mut session_token = None;
        if let Some(cookie_str) = cookie_header {
            for cookie in cookie_str.split(';') {
                let parts: Vec<&str> = cookie.trim().split('=').collect();
                if parts.len() == 2 && parts[0] == "admin_session" {
                    session_token = Some(parts[1].to_string());
                    break;
                }
            }
        }

        let token = match session_token {
            Some(t) => t,
            None => return Ok(None),
        };

        let sessions = app_state.admin_sessions.lock().await;
        if let Some(session) = sessions.get(&token) {
            if session.expires_at > chrono::Utc::now() {
                return Ok(Some(AdminUser {
                    username: session.username.clone(),
                }));
            }
        }

        Ok(None)
    }
}

/// Serve admin.html
pub async fn serve_admin_page() -> impl IntoResponse {
    match tokio::fs::read_to_string("src/static/admin.html").await {
        Ok(html) => (
            StatusCode::OK,
            [("content-type", "text/html; charset=utf-8")],
            html,
        )
            .into_response(),
        Err(e) => {
            eprintln!("Error | Failed to read admin.html: {:?}", e);
            (StatusCode::NOT_FOUND, "Admin dashboard page not found").into_response()
        }
    }
}

/// Redirects to GitHub OAuth authorize page
pub async fn github_login(State(state): State<AppState>) -> impl IntoResponse {
    if state.config.admin.github_client_id.is_empty()
        || state.config.admin.github_client_secret.is_empty()
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "GitHub OAuth is not configured on this server. Please set GITHUB_CLIENT_ID and GITHUB_CLIENT_SECRET."
        ).into_response();
    }

    let oauth_state = uuid::Uuid::new_v4().to_string();
    let auth_url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&state={}&scope=read:user",
        state.config.admin.github_client_id, oauth_state
    );

    let cookie = format!(
        "oauth_state={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=300",
        oauth_state
    );

    (
        StatusCode::SEE_OTHER,
        [
            (axum::http::header::SET_COOKIE, cookie),
            (axum::http::header::LOCATION, auth_url),
        ],
    )
        .into_response()
}

#[derive(Deserialize)]
pub struct CallbackParams {
    pub code: String,
    pub state: String,
}

/// Handles GitHub OAuth redirect callback
pub async fn github_callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<CallbackParams>,
) -> impl IntoResponse {
    let cookie_header = headers
        .get(axum::http::header::COOKIE)
        .and_then(|value| value.to_str().ok());

    let mut oauth_state = None;
    if let Some(cookie_str) = cookie_header {
        for cookie in cookie_str.split(';') {
            let parts: Vec<&str> = cookie.trim().split('=').collect();
            if parts.len() == 2 && parts[0] == "oauth_state" {
                oauth_state = Some(parts[1].to_string());
                break;
            }
        }
    }

    if oauth_state.is_none() || oauth_state.unwrap() != params.state {
        return (
            StatusCode::BAD_REQUEST,
            "CSRF state mismatch or expired auth session",
        )
            .into_response();
    }

    // Exchange code for access token
    let client = reqwest::Client::new();
    let token_response = match client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .json(&serde_json::json!({
            "client_id": state.config.admin.github_client_id,
            "client_secret": state.config.admin.github_client_secret,
            "code": params.code
        }))
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("Error | OAuth token exchange request failed: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to contact GitHub for token",
            )
                .into_response();
        }
    };

    let token_body: serde_json::Value = token_response
        .json()
        .await
        .unwrap_or(serde_json::Value::Null);
    let access_token = match token_body.get("access_token").and_then(|v| v.as_str()) {
        Some(tok) => tok.to_string(),
        None => {
            let err_msg = token_body
                .get("error_description")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            eprintln!("Error | GitHub OAuth error: {}", err_msg);
            return (
                StatusCode::UNAUTHORIZED,
                "Failed to retrieve access token from GitHub",
            )
                .into_response();
        }
    };

    // Get GitHub user profile
    let user_response = match client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "RPS-Admin-Dashboard")
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("Error | GitHub user profile request failed: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to contact GitHub for user info",
            )
                .into_response();
        }
    };

    let user_body: serde_json::Value = user_response
        .json()
        .await
        .unwrap_or(serde_json::Value::Null);
    let username = match user_body.get("login").and_then(|v| v.as_str()) {
        Some(login) => login.to_string(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                "Failed to retrieve GitHub login username",
            )
                .into_response()
        }
    };

    // Verify authorized username
    let allowed_username = &state.config.admin.github_allowed_username;
    if allowed_username.is_empty() || username.to_lowercase() != allowed_username.to_lowercase() {
        eprintln!(
            "Warning | Unauthorized admin login attempt by GitHub user: {}",
            username
        );
        return (
            StatusCode::FORBIDDEN,
            format!(
                "Access Denied: GitHub user '{}' is not authorized as admin.",
                username
            ),
        )
            .into_response();
    }

    // Create session
    let session_id = uuid::Uuid::new_v4().to_string();
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(24);

    let mut sessions = state.admin_sessions.lock().await;
    sessions.insert(
        session_id.clone(),
        AdminSession {
            username: username.clone(),
            expires_at,
        },
    );

    let session_cookie = format!(
        "admin_session={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        session_id,
        24 * 3600
    );
    let clear_oauth_cookie = "oauth_state=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0";

    (
        StatusCode::SEE_OTHER,
        [
            (axum::http::header::SET_COOKIE, session_cookie),
            (
                axum::http::header::SET_COOKIE,
                clear_oauth_cookie.to_string(),
            ),
            (axum::http::header::LOCATION, "/admin".to_string()),
        ],
    )
        .into_response()
}

/// Logout admin user
pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let cookie_header = headers
        .get(axum::http::header::COOKIE)
        .and_then(|value| value.to_str().ok());

    let mut session_token = None;
    if let Some(cookie_str) = cookie_header {
        for cookie in cookie_str.split(';') {
            let parts: Vec<&str> = cookie.trim().split('=').collect();
            if parts.len() == 2 && parts[0] == "admin_session" {
                session_token = Some(parts[1].to_string());
                break;
            }
        }
    }

    if let Some(token) = session_token {
        let mut sessions = state.admin_sessions.lock().await;
        sessions.remove(&token);
    }

    let clear_cookie = "admin_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0";

    (
        StatusCode::OK,
        [(axum::http::header::SET_COOKIE, clear_cookie)],
        Json(serde_json::json!({ "success": true })),
    )
}

/// Checks current admin status
pub async fn check_status(
    admin_user: Option<AdminUser>,
    State(_state): State<AppState>,
) -> impl IntoResponse {
    match admin_user {
        Some(user) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "logged_in": true,
                "username": user.username
            })),
        ),
        None => (
            StatusCode::OK,
            Json(serde_json::json!({
                "logged_in": false
            })),
        ),
    }
}

#[derive(Serialize)]
pub struct MetricsResponse {
    pub cpu_usage: f32,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub database_size_bytes: i64,
    pub pastes_table_size_bytes: i64,
    pub database_limit_bytes: u64,
    pub total_pastes: i64,
    pub active_pastes: i64,
}

/// Retrieve metrics: GET /api/admin/metrics
pub async fn get_metrics(_admin: AdminUser, State(state): State<AppState>) -> impl IntoResponse {
    // 1. Sysinfo metrics
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu();
    // Tiny sleep to gather accurate CPU usage
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    sys.refresh_cpu();

    let memory_total_bytes = sys.total_memory();
    let memory_used_bytes = sys.used_memory();
    let cpu_usage = if sys.cpus().is_empty() {
        0.0
    } else {
        sys.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / sys.cpus().len() as f32
    };

    // 2. Database metrics
    let db_size: (i64,) = sqlx::query_as("SELECT pg_database_size(current_database())")
        .fetch_one(&state.pool)
        .await
        .unwrap_or((0,));

    let table_size: (i64,) = sqlx::query_as("SELECT pg_total_relation_size('pastes')")
        .fetch_one(&state.pool)
        .await
        .unwrap_or((0,));

    let total_pastes: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM pastes")
        .fetch_one(&state.pool)
        .await
        .unwrap_or((0,));

    let active_pastes: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM pastes WHERE expires_at > $1")
        .bind(chrono::Utc::now())
        .fetch_one(&state.pool)
        .await
        .unwrap_or((0,));

    Json(MetricsResponse {
        cpu_usage,
        memory_used_bytes,
        memory_total_bytes,
        database_size_bytes: db_size.0,
        pastes_table_size_bytes: table_size.0,
        database_limit_bytes: state.config.database.storage_limit_bytes,
        total_pastes: total_pastes.0,
        active_pastes: active_pastes.0,
    })
}

#[derive(Serialize)]
pub struct AdminPasteItem {
    pub id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub content_length: i32,
    pub is_password_protected: bool,
}

/// Retrieves list of recent pastes: GET /api/admin/pastes
pub async fn list_pastes(_admin: AdminUser, State(state): State<AppState>) -> impl IntoResponse {
    let rows = sqlx::query(
        "SELECT id, created_at, expires_at, LENGTH(content) as length, (password_hash IS NOT NULL) as is_protected FROM pastes ORDER BY created_at DESC LIMIT 100"
    )
    .fetch_all(&state.pool)
    .await;

    let mut items = Vec::new();
    if let Ok(rows) = rows {
        use sqlx::Row;
        for row in rows {
            let id: String = row.get("id");
            let created_at: chrono::DateTime<chrono::Utc> = row
                .get::<Option<chrono::DateTime<chrono::Utc>>, _>("created_at")
                .unwrap_or_else(chrono::Utc::now);
            let expires_at: chrono::DateTime<chrono::Utc> = row.get("expires_at");
            let length: i32 = row.get::<Option<i32>, _>("length").unwrap_or(0);
            let is_protected: bool = row.get("is_protected");

            items.push(AdminPasteItem {
                id,
                created_at,
                expires_at,
                content_length: length,
                is_password_protected: is_protected,
            });
        }
    }

    Json(items)
}

/// Deletes a paste: DELETE /api/admin/pastes/{id}
pub async fn delete_paste(
    _admin: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let clean_id = match id.split_once('.') {
        Some((prefix, _)) => prefix.to_string(),
        None => id,
    };

    match sqlx::query("DELETE FROM pastes WHERE id = $1")
        .bind(&clean_id)
        .execute(&state.pool)
        .await
    {
        Ok(res) => {
            if res.rows_affected() > 0 {
                println!("Success | Admin deleted paste '{}' manually.", clean_id);
                (StatusCode::OK, Json(serde_json::json!({ "success": true, "message": "Paste deleted successfully" }))).into_response()
            } else {
                (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "success": false, "message": "Paste not found" })),
                )
                    .into_response()
            }
        }
        Err(e) => {
            eprintln!("Error | Failed to delete paste: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "success": false, "message": "Failed to delete paste from database" }))).into_response()
        }
    }
}

/// Manually trigger expired paste cleanups: POST /api/admin/cleanup
pub async fn manual_cleanup(_admin: AdminUser, State(state): State<AppState>) -> impl IntoResponse {
    println!("Info | Admin manually triggered expired paste cleanup...");
    let now = Utc::now();
    match sqlx::query("DELETE FROM pastes WHERE expires_at < $1")
        .bind(now)
        .execute(&state.pool)
        .await
    {
        Ok(res) => {
            let count = res.rows_affected();
            println!(
                "Success | Admin manual cleanup: Deleted {} expired pastes.",
                count
            );
            (
                StatusCode::OK,
                Json(serde_json::json!({ "success": true, "deleted_count": count })),
            )
                .into_response()
        }
        Err(e) => {
            eprintln!("Error | Admin manual cleanup failed: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "success": false, "message": "Cleanup failed" })),
            )
                .into_response()
        }
    }
}
