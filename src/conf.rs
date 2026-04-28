use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
    pub schema: String,
}

static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn conf_init(path: &str) {
    let content =
        std::fs::read_to_string(path).unwrap_or_else(|_| panic!("failed to read config: {path}"));
    let config: Config = toml::from_str(&content)
        .unwrap_or_else(|e| panic!("failed to parse config '{path}': {e}"));
    eprintln!("loaded config from {path}");
    CONFIG
        .set(config)
        .unwrap_or_else(|_| panic!("config already initialized"));
}

pub fn conf_get() -> &'static Config {
    CONFIG.get().expect("config not initialized")
}
