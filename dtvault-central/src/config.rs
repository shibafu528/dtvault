use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Debug)]
pub struct Config {
    pub server: Server,
    pub database: Database,
    #[serde(default)]
    pub storages: Vec<Storage>,
}

impl Config {
    pub fn validate(&self) -> Result<(), String> {
        self.server.validate()?;
        self.database.validate()?;
        if self.storages.is_empty() {
            return Err("no storage found".to_string());
        }
        for storage in &self.storages {
            storage.validate()?;
        }
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
#[serde(tag = "driver")]
pub enum Storage {
    FileSystem(FileSystem),
    Tempfile,
}

impl Storage {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Storage::FileSystem(fs) => fs.validate(),
            Storage::Tempfile => Ok(()),
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct FileSystem {
    pub root_dir: String,
}

impl FileSystem {
    pub fn validate(&self) -> Result<(), String> {
        if self.root_dir.is_empty() {
            return Err("no storage_dir found".to_string());
        }

        let root_dir = Path::new(&self.root_dir);
        if !root_dir.is_dir() {
            if let Err(e) = std::fs::create_dir_all(root_dir) {
                return Err(e.to_string());
            }
        }

        Ok(())
    }
}
