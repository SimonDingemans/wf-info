use crate::{config::ClipboardConfig, rewards::RewardOverlayEntry};

pub fn reward_summary(config: &ClipboardConfig, rewards: &[RewardOverlayEntry]) -> Option<String> {
    if !config.enabled || rewards.is_empty() {
        return None;
    }

    let mut lines = rewards
        .iter()
        .map(|reward| reward_summary_line(config, reward))
        .collect::<Vec<_>>();

    let footer = config.footer.trim();
    if !footer.is_empty() {
        lines.push(footer.to_owned());
    }

    Some(lines.join("\n"))
}

fn reward_summary_line(config: &ClipboardConfig, reward: &RewardOverlayEntry) -> String {
    let mut details = Vec::new();

    if let Some(platinum) = reward.platinum {
        details.push(format!("{platinum}p"));
    }

    if let Some(ducats) = reward.ducats {
        details.push(format!("{ducats} ducats"));
    }

    if let Some(volume) = reward.volume {
        details.push(format!("{volume} trades"));
    }

    if config.include_vaulted_marker && reward.vaulted {
        details.push("vaulted".to_owned());
    }

    if details.is_empty() {
        reward.name.clone()
    } else {
        format!("{}: {}", reward.name, details.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use crate::{clipboard::reward_summary, config::ClipboardConfig, rewards::RewardOverlayEntry};

    #[test]
    fn disabled_clipboard_config_suppresses_reward_summary() {
        let config = ClipboardConfig::default();
        let rewards = vec![RewardOverlayEntry::name_only("Forma Blueprint")];

        assert_eq!(reward_summary(&config, &rewards), None);
    }

    #[test]
    fn reward_summary_formats_known_reward_values() {
        let config = ClipboardConfig {
            enabled: true,
            include_vaulted_marker: true,
            footer: "via wf-info".to_owned(),
        };
        let reward = RewardOverlayEntry::name_only("Braton Prime Receiver")
            .with_platinum(42)
            .with_ducats(45)
            .with_volume(7)
            .with_vaulted(true);

        assert_eq!(
            reward_summary(&config, &[reward]),
            Some(
                "Braton Prime Receiver: 42p, 45 ducats, 7 trades, vaulted\nvia wf-info".to_owned()
            )
        );
    }

    #[test]
    fn reward_summary_can_omit_vaulted_marker() {
        let config = ClipboardConfig {
            enabled: true,
            include_vaulted_marker: false,
            footer: String::new(),
        };
        let reward = RewardOverlayEntry::name_only("Paris Prime String").with_vaulted(true);

        assert_eq!(
            reward_summary(&config, &[reward]),
            Some("Paris Prime String".to_owned())
        );
    }
}
