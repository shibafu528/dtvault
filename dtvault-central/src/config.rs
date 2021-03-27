use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Debug)]
pub struct Config {
    pub server: Server,
    pub database: Database,
    pub storage: Storage,
}

impl Config {
    pub fn validate(&self) -> Result<(), String> {
        self.server.validate()?;
        self.database.validate()?;
        self.storage.validate()?;
        Ok(())
    }
}

#[derive(Deserialize, Debug)]
pub struct Server {
    pub listen: String,
}

impl Server {
    pub fn validate(&self) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Deserialize, Debug)]
pub struct Database {
    data_dir: String,
}

impl Database {
    pub fn validate(&self) -> Result<(), String> {
        let data_dir = Path::new(&self.data_dir);
        if !data_dir.is_dir() {
            if let Err(e) = std::fs::create_dir_all(data_dir) {
                return Err(e.to_string());
            }
        }
        Ok(())
    }

    pub fn programs_file_path(&self) -> PathBuf {
        PathBuf::from(self.data_dir.to_string()).join("programs.pb")
    }
}

#[derive(Deserialize, Debug)]
pub struct Storage {
    storage_dir: Vec<String>,
}

impl Storage {
    pub fn validate(&self) -> Result<(), String> {
        if self.storage_dir.is_empty() {
            return Err("no storage_dir found".to_string());
        }
        for dir in &self.storage_dir {
            let dir = Path::new(dir);
            if !dir.is_dir() {
                if let Err(e) = std::fs::create_dir_all(dir) {
                    return Err(e.to_string());
                }
            }
        }
        Ok(())
    }

    pub fn primary_storage_dir(&self) -> &str {
        self.storage_dir.first().unwrap()
    }
}
