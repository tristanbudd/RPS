use serde::Deserialize;

/// Server configurations
#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub paste: PasteConfig,
    pub rate_limit: RateLimitConfig,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Deserialize, Clone, Debug)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct PasteConfig {
    pub default_expiry_days: i64,
    pub extend_expiry_on_read: bool,
    pub cleanup_interval_seconds: u64,
    pub id_type: String,
    pub id_length: usize,
    pub redirect_to_duplicate: bool,
    pub max_length: usize,
}

#[derive(Deserialize, Clone, Debug)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub max_concurrent_requests: usize,
    pub requests_per_minute: usize,
}

/// Loads configuration from `config.toml` or falls back to environment defaults.
pub fn load_config() -> Config {
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
