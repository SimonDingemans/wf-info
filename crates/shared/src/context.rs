use std::path::{Path, PathBuf};

use crate::config::{CliConfigOverrides, Config, Settings};

#[derive(Clone, Debug)]
pub struct AppContext {
    name: &'static str,
    config_path: PathBuf,
    cache_dir: PathBuf,
    config_overrides: CliConfigOverrides,
}

impl AppContext {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            config_path: Config::path_for_app(name),
            cache_dir: cache_dir_for_app(name),
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

    pub fn with_cache_dir(mut self, cache_dir: impl Into<PathBuf>) -> Self {
        self.cache_dir = cache_dir.into();
        self
    }

    pub const fn name(&self) -> &'static str {
        self.name
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
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

fn cache_dir_for_app(app_name: &str) -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join(app_name)
}

#[cfg(test)]
mod tests {
    use super::AppContext;

    #[test]
    fn app_context_allows_cache_dir_override_for_service_tests() {
        let cache_dir = std::env::temp_dir().join("wf-info-context-cache-test");
        let context = AppContext::new("wf-info-test").with_cache_dir(&cache_dir);

        assert_eq!(context.cache_dir(), cache_dir.as_path());
    }
}
