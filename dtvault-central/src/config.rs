use serde::Deserialize;
use std::path::{Path, PathBuf};
use tonic::transport::Uri;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub server: Server,
    pub database: Database,
    #[serde(default)]
    pub storages: Vec<Storage>,
    #[serde(default)]
    pub outlet: Outlet,
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
        self.outlet.validate()?;
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

#[derive(Deserialize, Debug, Default)]
pub struct Outlet {
    pub encoder_url: String,
}

impl Outlet {
    pub fn validate(&self) -> Result<(), String> {
        if !self.encoder_url.is_empty() {
            if let Err(e) = self.encoder_url.parse::<Uri>() {
                return Err(format!("outlet.encoder_url is invalid: {}", e));
            }
        }

        Ok(())
    }

    pub fn encoder_url(&self) -> Option<Uri> {
        if self.encoder_url.is_empty() {
            None
        } else {
            Some(self.encoder_url.parse().unwrap())
        }
    }
}
