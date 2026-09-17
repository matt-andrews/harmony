use std::net::SocketAddr;

use anyhow::Context;

pub struct Config {
    /// Raw `HARMONY_STORAGE` spec; see [`crate::storage::Storage::from_spec`].
    pub storage_spec: String,
    pub bind: SocketAddr,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let storage_spec = std::env::var("HARMONY_STORAGE")
            .context("HARMONY_STORAGE is not set (see .env.example)")?;
        let bind = std::env::var("HARMONY_BIND")
            .unwrap_or_else(|_| "127.0.0.1:8080".into())
            .parse()
            .context("HARMONY_BIND must be host:port")?;
        Ok(Self { storage_spec, bind })
    }
}
