use serde::Deserialize;

/// Server configurations
#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub paste: PasteConfig,
    pub rate_limit: RateLimitConfig,
    pub security: SecurityConfig,
    #[serde(default)]
    pub admin: AdminConfig,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SecurityConfig {
    pub password_protection_enabled: bool,
    pub encryption_enabled: bool,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Deserialize, Clone, Debug)]
pub struct DatabaseConfig {
    pub url: String,
    #[serde(default = "default_database_storage_limit_bytes")]
    pub storage_limit_bytes: u64,
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

#[derive(Deserialize, Clone, Debug)]
pub struct AdminConfig {
    #[serde(default)]
    pub github_client_id: String,
    #[serde(default)]
    pub github_client_secret: String,
    #[serde(default)]
    pub github_allowed_username: String,
}

fn default_database_storage_limit_bytes() -> u64 {
    10 * 1024 * 1024 * 1024 // 10 GB
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            github_client_id: std::env::var("GITHUB_CLIENT_ID").unwrap_or_default(),
            github_client_secret: std::env::var("GITHUB_CLIENT_SECRET").unwrap_or_default(),
            github_allowed_username: std::env::var("GITHUB_ALLOWED_USERNAME").unwrap_or_default(),
        }
    }
}

/// Loads configuration from `config.toml` or falls back to environment defaults.
pub fn load_config() -> Config {
    let mut config = if let Ok(content) = std::fs::read_to_string("config.toml") {
        if let Ok(cfg) = toml::from_str::<Config>(&content) {
            println!("Success | Configuration loaded successfully from config.toml");
            cfg
        } else {
            println!("Warning | Failed to parse config.toml; using default configuration");
            default_config()
        }
    } else {
        println!("Info | config.toml not found or invalid; falling back to environment settings");
        default_config()
    };

    // Override with environment variables if present
    if let Ok(id) = std::env::var("GITHUB_CLIENT_ID") {
        config.admin.github_client_id = id;
    }
    if let Ok(sec) = std::env::var("GITHUB_CLIENT_SECRET") {
        config.admin.github_client_secret = sec;
    }
    if let Ok(user) = std::env::var("GITHUB_ALLOWED_USERNAME") {
        config.admin.github_allowed_username = user;
    }
    if let Ok(limit) = std::env::var("DATABASE_STORAGE_LIMIT_BYTES") {
        if let Ok(l) = limit.parse() {
            config.database.storage_limit_bytes = l;
        }
    }

    config
}

fn default_config() -> Config {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/rps".to_string());
    Config {
        server: ServerConfig {
            host: "0.0.0.0".to_string(),
            port: 8000,
        },
        database: DatabaseConfig {
            url: db_url,
            storage_limit_bytes: default_database_storage_limit_bytes(),
        },
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
        security: SecurityConfig {
            password_protection_enabled: true,
            encryption_enabled: true,
        },
        admin: AdminConfig::default(),
    }
}
