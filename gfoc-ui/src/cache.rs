use std::{
    fs::{File, OpenOptions},
    io::Write,
};

use crate::client::Config;

pub fn save_file(config: &Config) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open("config.json")?;
    let content = serde_json::to_string_pretty(config)?;
    file.write_all(content.as_bytes())
}

pub fn load_config() -> std::io::Result<Option<Config>> {
    let data = std::fs::read("config.json")?;
    Ok(serde_json::from_slice(data.as_slice()).unwrap_or(None))
}
