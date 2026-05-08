use shared::{AppContext, config::CliConfigOverrides, rewards::RewardOverlayEntry};

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
        let mut rewards = Vec::new();
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
                "--reward-overlay" => mode = Some("rewards"),
                "--output" => output_name = args.next(),
                "--line" => {
                    if let Some(line) = args.next() {
                        lines.push(line);
                    }
                }
                "--reward-name" => {
                    if let Some(name) = args.next() {
                        rewards.push(RewardOverlayEntry::name_only(name));
                    }
                }
                "--reward-platinum" => {
                    if let Some(platinum) = args.next().and_then(|value| value.parse().ok()) {
                        set_last_reward_platinum(&mut rewards, platinum);
                    }
                }
                "--reward-ducats" => {
                    if let Some(ducats) = args.next().and_then(|value| value.parse().ok()) {
                        set_last_reward_ducats(&mut rewards, ducats);
                    }
                }
                "--reward-vaulted" => {
                    if let Some(vaulted) = args.next().and_then(|value| parse_bool_arg(&value)) {
                        set_last_reward_vaulted(&mut rewards, vaulted);
                    }
                }
                _ => {}
            }
        }

        Self {
            debug_overlay: debug_overlay_from(mode, output_name, lines, rewards),
            config_overrides,
        }
    }
}

fn set_last_reward_platinum(rewards: &mut [RewardOverlayEntry], platinum: u32) {
    if let Some(reward) = rewards.last_mut() {
        reward.set_platinum(platinum);
    }
}

fn set_last_reward_ducats(rewards: &mut [RewardOverlayEntry], ducats: u32) {
    if let Some(reward) = rewards.last_mut() {
        reward.set_ducats(ducats);
    }
}

fn set_last_reward_vaulted(rewards: &mut [RewardOverlayEntry], vaulted: bool) {
    if let Some(reward) = rewards.last_mut() {
        reward.set_vaulted(vaulted);
    }
}

fn parse_bool_arg(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "1" => Some(true),
        "false" | "no" | "0" => Some(false),
        _ => None,
    }
}

fn debug_overlay_from(
    mode: Option<&'static str>,
    output_name: Option<String>,
    lines: Vec<String>,
    rewards: Vec<RewardOverlayEntry>,
) -> Option<overlay::DebugOverlay> {
    match mode {
        Some("monitor-info") => Some(overlay::DebugOverlay::MonitorInfo { output_name, lines }),
        Some("test") => Some(overlay::DebugOverlay::Test { output_name }),
        Some("rewards") => Some(overlay::DebugOverlay::Rewards {
            output_name,
            rewards,
        }),
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

    #[test]
    fn cli_args_parse_reward_overlay_entries() {
        let args = CliArgs::parse(
            [
                "--reward-overlay",
                "--output",
                "DP-1",
                "--reward-name",
                "Forma Blueprint",
                "--reward-platinum",
                "8",
                "--reward-ducats",
                "15",
                "--reward-vaulted",
                "false",
                "--reward-name",
                "Braton Prime Receiver",
                "--reward-platinum",
                "42",
                "--reward-ducats",
                "45",
                "--reward-vaulted",
                "yes",
            ]
            .into_iter()
            .map(str::to_owned),
        );

        match args.debug_overlay {
            Some(overlay::DebugOverlay::Rewards {
                output_name,
                rewards,
            }) => {
                assert_eq!(output_name.as_deref(), Some("DP-1"));
                assert_eq!(rewards.len(), 2);
                assert_eq!(rewards[0].name, "Forma Blueprint");
                assert_eq!(rewards[0].platinum, Some(8));
                assert_eq!(rewards[0].ducats, Some(15));
                assert!(!rewards[0].vaulted);
                assert_eq!(rewards[1].name, "Braton Prime Receiver");
                assert_eq!(rewards[1].platinum, Some(42));
                assert_eq!(rewards[1].ducats, Some(45));
                assert!(rewards[1].vaulted);
            }
            _ => panic!("expected reward overlay"),
        }
    }
}
