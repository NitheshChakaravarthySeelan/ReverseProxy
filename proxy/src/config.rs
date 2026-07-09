use std::net::SocketAddr;
use std::path::Path;

use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub listen: SocketAddr,
    pub backends: Vec<SocketAddr>,
    #[serde(default = "default_health_interval")]
    pub health_check_interval_secs: u64,
    #[serde(default = "default_health_path")]
    pub health_check_path: String,
}

fn default_health_interval() -> u64 {
    10
}

fn default_health_path() -> String {
    "/health".to_string()
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}
