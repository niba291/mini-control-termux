use std::{
    env,
    net::SocketAddr,
    path::PathBuf,
};

use anyhow::{Context, Result};

pub struct Config {
    pub addr: SocketAddr,
    pub root: PathBuf,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let host = env::var("POCKET_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = match env::var("POCKET_PORT") {
            Ok(value) => value
                .parse::<u16>()
                .with_context(|| format!("invalid POCKET_PORT: {value}"))?,
            Err(_) => 8080,
        };

        let addr = format!("{host}:{port}")
            .parse()
            .with_context(|| format!("invalid bind address: {host}:{port}"))?;

        let root = match env::var_os("POCKET_ROOT") {
            Some(value) => PathBuf::from(value),
            None => env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".")),
        };
        let root = std::fs::canonicalize(&root)
            .with_context(|| format!("invalid POCKET_ROOT: {}", root.display()))?;

        Ok(Self { addr, root })
    }
}
