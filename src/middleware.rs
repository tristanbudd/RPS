use crate::AppState;
use axum::{
    extract::{ConnectInfo, State},
    http::StatusCode,
    middleware::Next,
    response::IntoResponse,
};
use std::net::SocketAddr;

/// Middleware to enforce per-IP rate limiting
pub async fn ip_rate_limit_middleware(
    State(state): State<AppState>,
    request: axum::http::Request<axum::body::Body>,
    next: Next,
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

/// Middleware to add Cache-Control headers for static assets and page requests
pub async fn cache_control_middleware(
    request: axum::http::Request<axum::body::Body>,
    next: Next,
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
