use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, delegate_noop,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{wl_output, wl_registry},
};
use wayland_protocols::xdg::xdg_output::zv1::client::{
    zxdg_output_manager_v1::ZxdgOutputManagerV1, zxdg_output_v1, zxdg_output_v1::ZxdgOutputV1,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonitorInfo {
    pub pipe_wire_node_id: Option<u32>,
    pub id: Option<String>,
    pub mapping_id: Option<String>,
    pub position: Option<(i32, i32)>,
    /// Pixel count after display scaling has been applied.
    pub size: Option<(i32, i32)>,
    pub source_type: Option<String>,
    pub xdg_output_name: Option<String>,
}

impl MonitorInfo {
    pub fn display_name(&self) -> String {
        self.xdg_output_name
            .as_deref()
            .or(self.id.as_deref())
            .or(self.mapping_id.as_deref())
            .map(str::to_owned)
            .or_else(|| {
                self.pipe_wire_node_id
                    .map(|node_id| format!("PipeWire node {node_id}"))
            })
            .unwrap_or_else(|| "unknown output".to_owned())
    }

    pub fn layer_shell_output_name(&self) -> Option<&str> {
        self.xdg_output_name
            .as_deref()
            .or(self.id.as_deref())
            .or(self.mapping_id.as_deref())
    }

    pub fn summary_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("Output: {}", self.display_name()),
            format!(
                "Layer-shell target: {}",
                self.xdg_output_name.as_deref().unwrap_or("unknown")
            ),
            format!("Position: {}", format_pair(self.position)),
            format!("Size: {}", format_pair(self.size)),
            format!(
                "Source: {}",
                self.source_type.as_deref().unwrap_or("unknown")
            ),
        ];

        if let Some(pipe_wire_node_id) = self.pipe_wire_node_id {
            lines.push(format!("PipeWire node: {pipe_wire_node_id}"));
        }

        lines
    }

    pub fn matches_target(&self, target: &str) -> bool {
        let normalized_target = normalize_monitor_target(target);
        [
            self.xdg_output_name.as_deref(),
            self.id.as_deref(),
            self.mapping_id.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|value| normalize_monitor_target(value) == normalized_target)
    }

    pub fn logical_region(&self) -> Result<MonitorRegion, String> {
        let (x, y) = self.position.ok_or_else(|| {
            format!(
                "capture monitor {} does not have a known position",
                self.display_name()
            )
        })?;
        let (width, height) = self.size.ok_or_else(|| {
            format!(
                "capture monitor {} does not have a known size",
                self.display_name()
            )
        })?;

        if x < 0 || y < 0 || width <= 0 || height <= 0 {
            return Err(format!(
                "capture monitor {} has unsupported geometry: position={:?}, size={:?}",
                self.display_name(),
                self.position,
                self.size
            ));
        }

        Ok(MonitorRegion {
            x: x as u32,
            y: y as u32,
            width: width as u32,
            height: height as u32,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonitorRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl MonitorRegion {
    pub fn right(&self) -> u32 {
        self.x.saturating_add(self.width)
    }

    pub fn bottom(&self) -> u32 {
        self.y.saturating_add(self.height)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonitorCaptureRegion {
    pub region: MonitorRegion,
    pub desktop_bounds: MonitorRegion,
}

pub fn detect_monitor_info() -> Result<Vec<MonitorInfo>, String> {
    Ok(detect_wayland_outputs()?
        .into_iter()
        .map(|output| MonitorInfo {
            pipe_wire_node_id: None,
            id: Some(output.name.clone()),
            mapping_id: None,
            position: Some(output.position),
            size: Some(output.size),
            source_type: Some("Wayland xdg-output".to_owned()),
            xdg_output_name: Some(output.name),
        })
        .collect())
}

pub fn capture_region_for_target(
    monitors: &[MonitorInfo],
    target: &str,
) -> Result<Option<MonitorCaptureRegion>, String> {
    let target = target.trim();
    if target_uses_full_screenshot(target) {
        return Ok(None);
    }

    let Some(monitor) = monitors
        .iter()
        .find(|monitor| monitor.matches_target(target))
    else {
        let available = monitors
            .iter()
            .map(MonitorInfo::display_name)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "configured capture monitor {target:?} was not found; available monitors: {}",
            if available.is_empty() {
                "none".to_owned()
            } else {
                available
            }
        ));
    };

    Ok(Some(MonitorCaptureRegion {
        region: monitor.logical_region()?,
        desktop_bounds: logical_desktop_bounds(monitors)?,
    }))
}

pub fn capture_region_for_monitor(
    monitors: &[MonitorInfo],
    monitor: &MonitorInfo,
) -> Result<MonitorCaptureRegion, String> {
    Ok(MonitorCaptureRegion {
        region: monitor.logical_region()?,
        desktop_bounds: logical_desktop_bounds(monitors)?,
    })
}

pub fn logical_desktop_bounds(monitors: &[MonitorInfo]) -> Result<MonitorRegion, String> {
    let mut regions = monitors
        .iter()
        .map(MonitorInfo::logical_region)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter();
    let Some(first) = regions.next() else {
        return Err("no monitor geometry was available for capture scaling".to_owned());
    };

    Ok(regions.fold(first, |bounds, region| MonitorRegion {
        x: bounds.x.min(region.x),
        y: bounds.y.min(region.y),
        width: bounds.right().max(region.right()) - bounds.x.min(region.x),
        height: bounds.bottom().max(region.bottom()) - bounds.y.min(region.y),
    }))
}

pub fn target_uses_full_screenshot(target: &str) -> bool {
    let target = target.trim();
    target.is_empty() || target.eq_ignore_ascii_case("primary")
}

fn format_pair(pair: Option<(i32, i32)>) -> String {
    pair.map(|(x, y)| format!("{x}, {y}"))
        .unwrap_or_else(|| "unknown".to_owned())
}

fn normalize_monitor_target(target: &str) -> String {
    target.trim().to_ascii_lowercase()
}

#[derive(Clone, Debug)]
struct WaylandOutputInfo {
    name: String,
    position: (i32, i32),
    size: (i32, i32),
}

#[derive(Default)]
struct WaylandOutputState {
    outputs: Vec<(u32, wl_output::WlOutput)>,
    xdg_outputs: Vec<WaylandOutputInfoState>,
}

impl WaylandOutputState {
    fn track_output(
        &mut self,
        name: u32,
        output: wl_output::WlOutput,
        manager: &ZxdgOutputManagerV1,
        queue_handle: &QueueHandle<Self>,
    ) {
        self.outputs.push((name, output.clone()));
        self.xdg_outputs
            .push(WaylandOutputInfoState::new(manager.get_xdg_output(
                &output,
                queue_handle,
                (),
            )));
    }

    fn apply_xdg_output_event(&mut self, proxy: &ZxdgOutputV1, event: zxdg_output_v1::Event) {
        let Some(output) = self
            .xdg_outputs
            .iter_mut()
            .find(|output| output.matches_proxy(proxy))
        else {
            return;
        };

        output.apply_event(event);
    }

    fn into_detected_outputs(self) -> Vec<WaylandOutputInfo> {
        self.xdg_outputs
            .into_iter()
            .filter_map(WaylandOutputInfoState::into_detected_output)
            .collect()
    }
}

struct WaylandOutputInfoState {
    xdg_output: ZxdgOutputV1,
    details: WaylandOutputDetails,
}

impl WaylandOutputInfoState {
    fn new(xdg_output: ZxdgOutputV1) -> Self {
        Self {
            xdg_output,
            details: WaylandOutputDetails::default(),
        }
    }

    fn matches_proxy(&self, proxy: &ZxdgOutputV1) -> bool {
        self.xdg_output == *proxy
    }

    fn apply_event(&mut self, event: zxdg_output_v1::Event) {
        self.details.apply_event(event);
    }

    fn into_detected_output(self) -> Option<WaylandOutputInfo> {
        self.details.into_detected_output()
    }
}

#[derive(Default)]
struct WaylandOutputDetails {
    name: Option<String>,
    position: Option<(i32, i32)>,
    size: Option<(i32, i32)>,
}

impl WaylandOutputDetails {
    fn apply_event(&mut self, event: zxdg_output_v1::Event) {
        match event {
            zxdg_output_v1::Event::Name { name } => self.set_name(name),
            zxdg_output_v1::Event::LogicalPosition { x, y } => self.set_position((x, y)),
            zxdg_output_v1::Event::LogicalSize { width, height } => self.set_size((width, height)),
            _ => {}
        }
    }

    fn set_name(&mut self, name: String) {
        self.name = Some(name);
    }

    fn set_position(&mut self, position: (i32, i32)) {
        self.position = Some(position);
    }

    fn set_size(&mut self, size: (i32, i32)) {
        self.size = Some(size);
    }

    fn into_detected_output(self) -> Option<WaylandOutputInfo> {
        Some(WaylandOutputInfo {
            name: self.name?,
            position: self.position?,
            size: self.size?,
        })
    }
}

fn detect_wayland_outputs() -> Result<Vec<WaylandOutputInfo>, String> {
    let connection = Connection::connect_to_env().map_err(|err| err.to_string())?;
    let (globals, mut event_queue) =
        registry_queue_init::<WaylandOutputState>(&connection).map_err(|err| err.to_string())?;
    let queue_handle = event_queue.handle();
    let manager = globals
        .bind::<ZxdgOutputManagerV1, _, _>(&queue_handle, 1..=3, ())
        .map_err(|err| err.to_string())?;

    let output_globals = globals.contents().with_list(|globals| {
        globals
            .iter()
            .filter(|global| global.interface == wl_output::WlOutput::interface().name)
            .map(|global| (global.name, global.version))
            .collect::<Vec<_>>()
    });

    let mut state = WaylandOutputState::default();
    for (name, version) in output_globals {
        let output = globals.registry().bind::<wl_output::WlOutput, _, _>(
            name,
            version.min(4),
            &queue_handle,
            (),
        );
        state.track_output(name, output, &manager, &queue_handle);
    }

    event_queue
        .roundtrip(&mut state)
        .map_err(|err| err.to_string())?;

    Ok(state.into_detected_outputs())
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for WaylandOutputState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _data: &GlobalListContents,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZxdgOutputV1, ()> for WaylandOutputState {
    fn event(
        state: &mut Self,
        proxy: &ZxdgOutputV1,
        event: zxdg_output_v1::Event,
        _data: &(),
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
        state.apply_xdg_output_event(proxy, event);
    }
}

delegate_noop!(WaylandOutputState: ignore wl_output::WlOutput);
delegate_noop!(WaylandOutputState: ignore ZxdgOutputManagerV1);

#[cfg(test)]
mod tests {
    use super::{
        MonitorInfo, MonitorRegion, WaylandOutputDetails, capture_region_for_monitor,
        logical_desktop_bounds,
    };
    use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_v1;

    #[test]
    fn output_details_apply_xdg_events_through_methods() {
        let mut details = WaylandOutputDetails::default();

        details.apply_event(zxdg_output_v1::Event::Name {
            name: "DP-1".to_owned(),
        });
        details.apply_event(zxdg_output_v1::Event::LogicalPosition { x: 10, y: 20 });
        details.apply_event(zxdg_output_v1::Event::LogicalSize {
            width: 2560,
            height: 1440,
        });

        let output = details
            .into_detected_output()
            .expect("complete output details should produce monitor info");

        assert_eq!(output.name, "DP-1");
        assert_eq!(output.position, (10, 20));
        assert_eq!(output.size, (2560, 1440));
    }

    #[test]
    fn incomplete_output_details_are_not_detected() {
        let mut details = WaylandOutputDetails::default();

        details.apply_event(zxdg_output_v1::Event::Name {
            name: "DP-1".to_owned(),
        });

        assert!(details.into_detected_output().is_none());
    }

    #[test]
    fn monitor_geometry_becomes_logical_region() {
        let monitor = monitor_info("DP-3", (2649, 0), (2648, 1490));

        let region = monitor.logical_region().expect("monitor region");

        assert_eq!(
            region,
            MonitorRegion {
                x: 2649,
                y: 0,
                width: 2648,
                height: 1490
            }
        );
    }

    #[test]
    fn monitor_matches_wayland_output_name_case_insensitively() {
        let monitor = monitor_info("DP-1", (0, 0), (1920, 1080));

        assert!(monitor.matches_target("dp-1"));
    }

    #[test]
    fn desktop_bounds_cover_all_monitor_logical_regions() {
        let monitors = vec![
            monitor_info("DP-3", (0, 0), (2648, 1490)),
            monitor_info("DP-1", (2649, 0), (2648, 1490)),
        ];

        let bounds = logical_desktop_bounds(&monitors).expect("desktop bounds");

        assert_eq!(
            bounds,
            MonitorRegion {
                x: 0,
                y: 0,
                width: 5297,
                height: 1490
            }
        );
    }

    #[test]
    fn capture_region_for_primary_target_uses_full_screenshot() {
        let monitors = vec![monitor_info("DP-1", (0, 0), (1920, 1080))];

        let region =
            super::capture_region_for_target(&monitors, " primary ").expect("primary target");

        assert_eq!(region, None);
    }

    #[test]
    fn capture_region_for_known_monitor_reuses_existing_geometry() {
        let monitors = vec![
            monitor_info("DP-1", (0, 0), (1920, 1080)),
            monitor_info("DP-2", (1920, 0), (2560, 1440)),
        ];

        let region = capture_region_for_monitor(&monitors, &monitors[1]).expect("capture region");

        assert_eq!(
            region,
            super::MonitorCaptureRegion {
                region: MonitorRegion {
                    x: 1920,
                    y: 0,
                    width: 2560,
                    height: 1440,
                },
                desktop_bounds: MonitorRegion {
                    x: 0,
                    y: 0,
                    width: 4480,
                    height: 1440,
                },
            }
        );
    }

    fn monitor_info(name: &str, position: (i32, i32), size: (i32, i32)) -> MonitorInfo {
        MonitorInfo {
            pipe_wire_node_id: None,
            id: Some(name.to_owned()),
            mapping_id: None,
            position: Some(position),
            size: Some(size),
            source_type: Some("Wayland xdg-output".to_owned()),
            xdg_output_name: Some(name.to_owned()),
        }
    }
}
