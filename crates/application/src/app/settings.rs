use super::state::Application;

impl Application {
    pub(super) fn set_clipboard_output_enabled(&mut self, enabled: bool) {
        self.settings.clipboard.enabled = enabled;

        match self.context.save_settings(&self.settings) {
            Ok(()) if enabled => {
                self.status = "Clipboard summaries enabled.".to_owned();
            }
            Ok(()) => {
                self.status = "Clipboard summaries disabled.".to_owned();
            }
            Err(err) if enabled => {
                self.status = format!(
                    "Clipboard summaries enabled for this session, but could not save: {err}"
                );
            }
            Err(err) => {
                self.status = format!(
                    "Clipboard summaries disabled for this session, but could not save: {err}"
                );
            }
        }
    }
}
