use shared::{AppContext, config::CliConfigOverrides};

pub fn run() {
    let args = CliArgs::parse(std::env::args().skip(1));
    let context = AppContext::new("wf-info").with_config_overrides(args.config_overrides);

    if let Err(err) = shared::logging::init(&context) {
        eprintln!("failed to initialize logging: {err}");
    }

    match args.debug_overlay {
        Some(overlay) => {
            if let Err(err) = overlay::run(&context, overlay) {
                eprintln!("failed to run debug overlay: {err}");
            }
        }
        None => {
            if let Err(err) = application::run(&context) {
                eprintln!("failed to run application: {err}");
            }
        }
    }
}

struct CliArgs {
    debug_overlay: Option<overlay::DebugOverlay>,
    config_overrides: CliConfigOverrides,
}

impl CliArgs {
    fn parse(args: impl Iterator<Item = String>) -> Self {
        let mut mode = None;
        let mut output_name = None;
        let mut lines = Vec::new();
        let mut config_overrides = CliConfigOverrides::default();
        let mut args = args.peekable();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--debug" => config_overrides.set_logging_level("debug"),
                "--trace" => config_overrides.set_logging_level("trace"),
                "--log-level" => {
                    if let Some(level) = args.next() {
                        config_overrides.set_logging_level(level);
                    }
                }
                "--debug-monitor-info" => mode = Some("monitor-info"),
                "--debug-test-overlay" => mode = Some("test"),
                "--output" => output_name = args.next(),
                "--line" => {
                    if let Some(line) = args.next() {
                        lines.push(line);
                    }
                }
                _ => {}
            }
        }

        Self {
            debug_overlay: debug_overlay_from(mode, output_name, lines),
            config_overrides,
        }
    }
}

fn debug_overlay_from(
    mode: Option<&'static str>,
    output_name: Option<String>,
    lines: Vec<String>,
) -> Option<overlay::DebugOverlay> {
    match mode {
        Some("monitor-info") => Some(overlay::DebugOverlay::MonitorInfo { output_name, lines }),
        Some("test") => Some(overlay::DebugOverlay::Test { output_name }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::CliArgs;

    #[test]
    fn cli_args_parse_logging_level_override() {
        let args = CliArgs::parse(["--log-level", "trace"].into_iter().map(str::to_owned));

        assert_eq!(args.config_overrides.logging_level(), Some("trace"));
    }

    #[test]
    fn cli_args_parse_debug_overlay_with_lines() {
        let args = CliArgs::parse(
            [
                "--debug-monitor-info",
                "--output",
                "DP-1",
                "--line",
                "size: 1920x1080",
            ]
            .into_iter()
            .map(str::to_owned),
        );

        match args.debug_overlay {
            Some(overlay::DebugOverlay::MonitorInfo { output_name, lines }) => {
                assert_eq!(output_name.as_deref(), Some("DP-1"));
                assert_eq!(lines, vec!["size: 1920x1080"]);
            }
            _ => panic!("expected monitor info debug overlay"),
        }
    }
}
