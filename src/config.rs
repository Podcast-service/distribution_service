use anyhow::{anyhow, Context, Result};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub bind_addr: String,
    pub public_base_url: String,
    pub db_max_connections: u32,
    pub db_connect_timeout_seconds: u64,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .map_err(|_| anyhow!("DATABASE_URL is required"))?;

        let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8788".into());

        let public_base_url = std::env::var("PUBLIC_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:8788".into());
        let public_base_url = public_base_url.trim_end_matches('/').to_string();

        let db_max_connections = parse_env("DB_MAX_CONNECTIONS", 10)?;
        let db_connect_timeout_seconds = parse_env("DB_CONNECT_TIMEOUT_SECONDS", 5)?;

        Ok(Self {
            database_url,
            bind_addr,
            public_base_url,
            db_max_connections,
            db_connect_timeout_seconds,
        })
    }
}

fn parse_env<T: std::str::FromStr>(key: &str, default: T) -> Result<T>
where
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    match std::env::var(key) {
        Ok(v) => v
            .parse::<T>()
            .map_err(|e| anyhow!("invalid {}: {}", key, e))
            .with_context(|| format!("env {key}")),
        Err(_) => Ok(default),
    }
}
