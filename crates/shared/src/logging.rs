use crate::{AppContext, config::LoggingConfig};

const WORKSPACE_LOG_TARGETS: &[&str] = &["application", "overlay", "shared", "wf_info"];

pub fn init(context: &AppContext) -> Result<(), String> {
    let settings = context.load_settings()?;
    init_with_settings(&settings.logging)
}

fn init_with_settings(settings: &LoggingConfig) -> Result<(), String> {
    let mut builder = env_logger::Builder::new();
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| settings.level.clone());

    builder.parse_filters(&workspace_scoped_filter(&filter));
    builder.format_timestamp_secs();
    builder.try_init().map_err(|err| err.to_string())
}

fn workspace_scoped_filter(filter: &str) -> String {
    let filter = filter.trim();

    if filter.is_empty() {
        return workspace_level_filter("info");
    }

    if is_level_directive(filter) {
        return workspace_level_filter(filter);
    }

    let directives = filter
        .split(',')
        .map(str::trim)
        .filter(|directive| !directive.is_empty())
        .flat_map(workspace_scoped_directive)
        .collect::<Vec<_>>()
        .join(",");

    if has_global_level(filter) {
        directives
    } else {
        format!("error,{directives}")
    }
}

fn workspace_scoped_directive(directive: &str) -> Vec<String> {
    let Some((target, level)) = directive.split_once('=') else {
        return if is_level_directive(directive) {
            workspace_level_filter_directives(directive)
        } else {
            vec![directive.to_owned()]
        };
    };

    if !level.eq_ignore_ascii_case("trace") || is_workspace_log_target(target) {
        return vec![directive.to_owned()];
    }

    vec![format!("{target}=error")]
}

fn workspace_level_filter(level: &str) -> String {
    workspace_level_filter_directives(level).join(",")
}

fn workspace_level_filter_directives(level: &str) -> Vec<String> {
    std::iter::once("error".to_owned())
        .chain(
            WORKSPACE_LOG_TARGETS
                .iter()
                .map(|target| format!("{target}={level}")),
        )
        .collect()
}

fn has_global_level(filter: &str) -> bool {
    filter
        .split(',')
        .map(str::trim)
        .any(|directive| is_level_directive(directive))
}

fn is_level_directive(directive: &str) -> bool {
    matches!(
        directive.to_ascii_lowercase().as_str(),
        "error" | "warn" | "info" | "debug" | "trace" | "off"
    )
}

fn is_workspace_log_target(target: &str) -> bool {
    WORKSPACE_LOG_TARGETS.iter().any(|workspace_target| {
        target == *workspace_target || target.starts_with(&format!("{workspace_target}::"))
    })
}

#[cfg(test)]
mod tests {
    use super::{init_with_settings, workspace_scoped_filter};
    use crate::config::LoggingConfig;

    #[test]
    fn logging_init_accepts_configured_level() {
        let settings = LoggingConfig {
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

    #[test]
    fn logging_filter_defaults_external_crates_to_error_and_workspace_to_info() {
        let filter = workspace_scoped_filter("");

        assert_eq!(
            filter,
            "error,application=info,overlay=info,shared=info,wf_info=info"
        );
    }

    #[test]
    fn logging_filter_scopes_trace_to_workspace_crates() {
        let filter = workspace_scoped_filter("trace");

        assert_eq!(
            filter,
            "error,application=trace,overlay=trace,shared=trace,wf_info=trace"
        );
    }

    #[test]
    fn logging_filter_downgrades_dependency_trace_directives_to_error() {
        let filter = workspace_scoped_filter("shared=trace,wgpu=trace,application::app=trace");

        assert_eq!(
            filter,
            "error,shared=trace,wgpu=error,application::app=trace"
        );
    }

    #[test]
    fn logging_filter_scopes_debug_to_workspace_crates() {
        let filter = workspace_scoped_filter("debug");

        assert_eq!(
            filter,
            "error,application=debug,overlay=debug,shared=debug,wf_info=debug"
        );
    }
}
