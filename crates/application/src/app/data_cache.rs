use iced::Task;

use crate::data_cache::{DataCacheRefresh, refresh_wfinfo_cache};

use super::message::Message;
use super::state::Application;

impl Application {
    pub(super) fn begin_data_cache_refresh(&mut self) -> Task<Message> {
        if self.data_cache_refresh_in_progress {
            self.status = "Data cache refresh is already running.".to_owned();
            return Task::none();
        }

        self.data_cache_refresh_in_progress = true;
        self.status = "Refreshing WFInfo data cache...".to_owned();
        let cache_dir = self.context.cache_dir().to_path_buf();

        Task::perform(
            refresh_wfinfo_cache(cache_dir),
            Message::DataCacheRefreshFinished,
        )
    }

    pub(super) fn record_data_cache_refresh_finished(
        &mut self,
        result: Result<DataCacheRefresh, String>,
    ) {
        self.data_cache_refresh_in_progress = false;
        self.last_data_cache_refresh = Some(result.clone());

        match result {
            Ok(refresh) => {
                self.status = format!(
                    "WFInfo data cache refreshed: {}, {}.",
                    refresh.prices_path.display(),
                    refresh.filtered_items_path.display()
                );
            }
            Err(err) => {
                self.status = format!("WFInfo data cache refresh failed: {err}");
            }
        }
    }
}
