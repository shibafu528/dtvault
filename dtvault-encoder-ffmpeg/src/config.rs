use dtvault_types::shibafu528::dtvault::encoder as types;
use serde::Deserialize;
use std::process::Stdio;
use tokio::process::Command;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub listen: String,
    #[serde(default)]
    pub presets: Vec<Preset>,
}

impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.presets.is_empty() {
            return Err("no presets found".to_string());
        }
        for preset in &self.presets {
            preset.validate()?;
        }
        Ok(())
    }
}

#[derive(Deserialize, Debug)]
pub struct Preset {
    pub id: String,
    pub title: Option<String>,
    pub command: String,
}

impl Preset {
    pub fn validate(&self) -> Result<(), String> {
        if let Err(e) = self.make_command() {
            return Err(format!("{}", e));
        }
        Ok(())
    }

    pub fn make_command(&self) -> Result<Command, InvalidCommand> {
        let words = shell_words::split(&self.command)?;
        let (program, args) = words.split_first().ok_or(InvalidCommand::Empty)?;

        let mut cmd = Command::new(program);
        cmd.args(args);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::inherit());
        cmd.kill_on_drop(true);

        Ok(cmd)
    }

    pub fn exchangeable(&self) -> types::Preset {
        types::Preset {
            preset_id: self.id.clone(),
            title: self.title.as_ref().unwrap_or_else(|| &self.id).clone(),
            command: self.command.clone(),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum InvalidCommand {
    #[error(transparent)]
    ParseError(#[from] shell_words::ParseError),
    #[error("command is empty")]
    Empty,
}
