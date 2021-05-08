mod condition;

use self::condition::Condition;
use crate::program::{Program, Video};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tonic::transport::Uri;
use uuid::Uuid;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub server: Server,
    pub database: Database,
    #[serde(default)]
    pub storages: Vec<Storage>,
    #[serde(default)]
    pub outlet: Outlet,
    #[serde(default)]
    pub storage_rules: Vec<StorageRule>,
    #[serde(default)]
    pub prefix_rules: Vec<PrefixRule>,
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
        for rule in &self.storage_rules {
            rule.validate()?;
        }
        for rule in &self.prefix_rules {
            rule.validate()?;
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
    Tempfile(Tempfile),
}

impl Storage {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Storage::FileSystem(fs) => fs.validate(),
            Storage::Tempfile(tf) => tf.validate(),
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct FileSystem {
    pub label: String,
    pub root_dir: String,
}

impl FileSystem {
    pub fn validate(&self) -> Result<(), String> {
        if self.label.is_empty() {
            return Err("label is empty".to_string());
        }

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

#[derive(Deserialize, Debug)]
pub struct Tempfile {
    pub label: String,
}

impl Tempfile {
    pub fn validate(&self) -> Result<(), String> {
        if self.label.is_empty() {
            return Err("label is empty".to_string());
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

#[derive(Deserialize, Debug, Default)]
pub struct StorageRule {
    condition: Condition,
    pub storage_label: String,
    #[serde(with = "crate::serde::uuid")]
    pub storage_id: Uuid,
}

impl StorageRule {
    pub fn validate(&self) -> Result<(), String> {
        self.condition.validate()?;
        if !self.storage_label.is_empty() && !self.storage_id.is_nil() {
            return Err("you may only specify one of these properties: storage_label, storage_id".to_string());
        }

        Ok(())
    }

    pub fn matches(&self, program: &Program, video: &Video) -> bool {
        self.condition.matches(program, video)
    }
}

#[derive(Deserialize, Debug, Default)]
pub struct PrefixRule {
    condition: Condition,
    pub prefix: String,
}

impl PrefixRule {
    pub fn validate(&self) -> Result<(), String> {
        self.condition.validate()?;
        if self.prefix.is_empty() {
            return Err("prefix is empty".to_string());
        }

        Ok(())
    }

    pub fn matches(&self, program: &Program, video: &Video) -> bool {
        self.condition.matches(program, video)
    }
}
