use ashpd::desktop::{
    CreateSessionOptions, PersistMode,
    screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType, StartCastOptions},
};

use crate::config::Settings;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortalMonitorStream {
    pub pipe_wire_node_id: u32,
    pub id: Option<String>,
    pub mapping_id: Option<String>,
    pub position: Option<(i32, i32)>,
    /// Pixel count after display scaling has been applied.
    pub size: Option<(i32, i32)>,
    pub source_type: Option<String>,
}

pub async fn select_monitor_streams(
    settings: &mut Settings,
) -> Result<Vec<PortalMonitorStream>, String> {
    let proxy = Screencast::new().await.map_err(|err| err.to_string())?;
    let session = proxy
        .create_session(CreateSessionOptions::default())
        .await
        .map_err(|err| err.to_string())?;

    let mut select_options = SelectSourcesOptions::default()
        .set_cursor_mode(CursorMode::Metadata)
        .set_sources(Some(SourceType::Monitor.into()))
        .set_multiple(false)
        .set_persist_mode(PersistMode::ExplicitlyRevoked);

    if let Some(restore_token) = settings.capture_portal_restore_token() {
        select_options = select_options.set_restore_token(restore_token);
    }

    proxy
        .select_sources(&session, select_options)
        .await
        .map_err(|err| err.to_string())?;

    let response = proxy
        .start(&session, None, StartCastOptions::default())
        .await
        .map_err(|err| err.to_string())?
        .response()
        .map_err(|err| err.to_string())?;

    if let Some(restore_token) = response.restore_token() {
        settings.set_capture_portal_restore_token(restore_token);
    }

    Ok(response
        .streams()
        .iter()
        .map(|stream| PortalMonitorStream {
            pipe_wire_node_id: stream.pipe_wire_node_id(),
            id: stream.id().map(str::to_owned),
            mapping_id: stream.mapping_id().map(str::to_owned),
            position: stream.position(),
            size: stream.size(),
            source_type: stream.source_type().map(|source| format!("{source:?}")),
        })
        .collect())
}
