use shared::AppContext;

pub fn run() {
    let context = AppContext::new("wf-info");

    match parse_debug_overlay(std::env::args().skip(1)) {
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

fn parse_debug_overlay(args: impl Iterator<Item = String>) -> Option<overlay::DebugOverlay> {
    let mut mode = None;
    let mut output_name = None;
    let mut lines = Vec::new();
    let mut args = args.peekable();

    while let Some(arg) = args.next() {
        match arg.as_str() {
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

    match mode {
        Some("monitor-info") => Some(overlay::DebugOverlay::MonitorInfo { output_name, lines }),
        Some("test") => Some(overlay::DebugOverlay::Test { output_name }),
        _ => None,
    }
}
