use crate::{AppContext, config::LoggingSettings};

pub fn init(context: &AppContext) -> Result<(), String> {
    let settings = context.load_settings()?;
    init_with_settings(&settings.logging)
}

fn init_with_settings(settings: &LoggingSettings) -> Result<(), String> {
    let mut builder = env_logger::Builder::new();
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| settings.level.clone());

    builder.parse_filters(&filter);
    builder.format_timestamp_secs();
    builder.try_init().map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::init_with_settings;
    use crate::config::LoggingSettings;

    #[test]
    fn logging_init_accepts_configured_level() {
        let settings = LoggingSettings {
            level: "debug".to_owned(),
            file: String::new(),
        };

        let result = init_with_settings(&settings);

        assert!(
            result.is_ok()
                || result
                    .as_ref()
                    .err()
                    .is_some_and(|err| err.contains("already initialized"))
        );
    }
}
