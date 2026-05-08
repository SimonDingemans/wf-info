use std::path::{Path, PathBuf};

use crate::config::{CliConfigOverrides, Config, Settings};

#[derive(Clone, Debug)]
pub struct AppContext {
    name: &'static str,
    config_path: PathBuf,
    config_overrides: CliConfigOverrides,
}

impl AppContext {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            config_path: Config::path_for_app(name),
            config_overrides: CliConfigOverrides::default(),
        }
    }

    pub fn with_config_overrides(mut self, overrides: CliConfigOverrides) -> Self {
        self.config_overrides = overrides;
        self
    }

    pub fn with_config_path(mut self, config_path: impl Into<PathBuf>) -> Self {
        self.config_path = config_path.into();
        self
    }

    pub const fn name(&self) -> &'static str {
        self.name
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn config_overrides(&self) -> &CliConfigOverrides {
        &self.config_overrides
    }

    pub fn load_settings(&self) -> Result<Settings, String> {
        Config::read_or_create(&self.config_path)
            .map(|config| config.with_cli_overrides(&self.config_overrides))
            .map(Settings::from)
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<(), String> {
        settings.write(&self.config_path)
    }
}
