use std::{env, net::SocketAddr};

use anyhow::{Context, Result};

pub struct Config {
    pub addr: SocketAddr,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = match env::var("PORT") {
            Ok(value) => value
                .parse::<u16>()
                .with_context(|| format!("invalid PORT: {value}"))?,
            Err(_) => 8080,
        };

        let addr = format!("{host}:{port}")
            .parse()
            .with_context(|| format!("invalid bind address: {host}:{port}"))?;

        Ok(Self { addr })
    }
}
