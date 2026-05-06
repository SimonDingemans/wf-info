use std::path::{Path, PathBuf};

use crate::config::{Config, Settings};

#[derive(Clone, Debug)]
pub struct AppContext {
    name: &'static str,
    config_path: PathBuf,
}

impl AppContext {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            config_path: Config::path_for_app(name),
        }
    }

    pub const fn name(&self) -> &'static str {
        self.name
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn load_settings(&self) -> Result<Settings, String> {
        Config::read(&self.config_path).map(Settings::from)
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<(), String> {
        settings.to_config().write(&self.config_path)
    }
}
