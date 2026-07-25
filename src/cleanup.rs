use chrono::Utc;
use std::sync::Arc;

/// Starts an asynchronous background task to periodically delete expired pastes.
pub fn start_cleanup_task(
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
