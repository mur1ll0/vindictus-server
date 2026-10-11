use crate::error::Error;
use std::{env, net::SocketAddr, path::PathBuf};
use zeroize::Zeroizing;

/// Startup configuration; intentionally excludes Debug to avoid printing secrets.
pub struct Config {
    pub database_url: Zeroizing<String>,
    pub bind_address: SocketAddr,
    pub db_max_connections: u32,
}

impl Config {
    pub fn from_env() -> Result<Self, Error> {
        let database_url = match (
            env::var("DATABASE_URL").ok(),
            env::var_os("DATABASE_URL_FILE"),
        ) {
            (Some(_), Some(_)) => {
                return Err(Error::Configuration(
                    "set only DATABASE_URL or DATABASE_URL_FILE".into(),
                ));
            }
            (Some(value), None) => Zeroizing::new(value),
            (None, Some(path)) => Zeroizing::new(
                std::fs::read_to_string(PathBuf::from(path))
                    .map_err(|_| Error::Configuration("could not read DATABASE_URL_FILE".into()))?
                    .trim_end_matches(['\r', '\n'])
                    .to_owned(),
            ),
            (None, None) => {
                return Err(Error::Configuration(
                    "DATABASE_URL or DATABASE_URL_FILE is required".into(),
                ));
            }
        };
        if !(database_url.starts_with("postgres://") || database_url.starts_with("postgresql://")) {
            return Err(Error::Configuration(
                "database URL must use PostgreSQL".into(),
            ));
        }
        let bind_address = env::var("BIND_ADDRESS")
            .unwrap_or_else(|_| "0.0.0.0:8080".into())
            .parse()
            .map_err(|_| {
                Error::Configuration("BIND_ADDRESS must be an IP:port socket address".into())
            })?;
        let db_max_connections = env::var("DB_MAX_CONNECTIONS")
            .unwrap_or_else(|_| "10".into())
            .parse::<u32>()
            .map_err(|_| Error::Configuration("DB_MAX_CONNECTIONS must be an integer".into()))?;
        if !(1..=100).contains(&db_max_connections) {
            return Err(Error::Configuration(
                "DB_MAX_CONNECTIONS must be in 1..=100".into(),
            ));
        }
        Ok(Self {
            database_url,
            bind_address,
            db_max_connections,
        })
    }
}
