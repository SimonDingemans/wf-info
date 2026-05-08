# wf-info Architecture Overview

This rewrite targets Linux desktops with Wayland as the primary session. The application UI is built with `iced`; the reward overlay is built with `iced_layershell`. The architecture should keep the first product slice focused on automatic end-of-mission reward scanning while leaving clean extension points for later features.

## Goals

- Detect end-of-mission reward screens automatically from Warframe logs.
- Capture the relevant screen/surface region and OCR it through Tesseract.
- Show reward value information in an overlay designed specifically for the reward screen.
- Keep the main application UI simple, stable, and easy to extend.
- Persist configuration as TOML on disk.
- Log to both the terminal and a log file through `env_logger`.
- Reuse the OCR pipeline for future screen-scanning features such as Snap It, Search It, and Master It.

## Workspace Shape

### Module Responsibility Constraints

- lib.rs, main.rs, and mod.rs files are strictly reserved for module declarations, structural exports and integration tests.
- These files must not contain business logic, implementation details, or complex data structures.
- Business logic, service definitions, and domain models must be implemented in dedicated files within the module hierarchy.

### `crates/wf-info`

Binary entry point.

Responsibilities:

- Build the startup `AppContext`.
- Initialize paths, logging, config, cache, and long-lived services.
- Start the `iced` application UI.
- Start or coordinate the `iced_layershell` overlay process/surface.
- Wire cross-crate channels for app events, scan results, overlay updates, and shutdown.

### `crates/shared`

Shared domain and infrastructure types.

Suggested modules:

- `context`: app name, version, paths, runtime handles.
- `config`: TOML-backed user configuration.
- `logging`: `env_logger` initialization and log file writer.
- `events`: app-wide messages exchanged between UI, overlay, log watcher, OCR, and data services.
- `domain`: reward, relic, equipment, market, and overlay models.
- `paths`: XDG config/data/cache/log path resolution.
- `errors`: common error/result types.

Keep `shared` free of UI framework code so application, overlay, OCR, data, and tests can reuse it.

### `crates/ocr`

Reusable OCR pipeline and feature scanners.

Responsibilities:

- Define capture frame, scan region, OCR image, OCR options, and text candidate types.
- Define reusable OCR pipeline traits for capture, region detection, preprocessing, recognition, and feature scanning.
- Keep Tesseract behind a `TextRecognizer` implementation.
- Keep general OCR pipeline and recognizer code separate from feature-specific scanners.
- Keep feature-specific scanners under an `implementations` module, with one subdirectory per implementation, starting with end-of-mission reward-screen scanning.
- Detect, preprocess, and OCR end-of-mission reward-name regions from supported 16:9 Warframe screenshots.
- Provide reward-screen OCR fixture coverage using crate-local assets.
- Keep market matching, price enrichment, user ownership data, and UI state outside this crate.

### `crates/application`

The main `iced` application.

Responsibilities:

- Display app status, startup state, scanner state, cache state, and recent errors.
- Expose settings that are relevant to Phase 1.
- Provide a settings window with tabs that map directly to config sections.
- Provide extension points for later feature pages:
  - relic browser
  - equipment browser
  - market automation
  - debug tools
  - Snap It/Search It/Master It
- Send commands/events to services instead of doing blocking work directly.
- Render status in a way that matches the log severity/status model.

The application should treat background services as event sources. It should not own OCR, Warframe log parsing, or market data loading logic.

### `crates/overlay`

The reward overlay built with `iced_layershell`.

Responsibilities:

- Render reward-screen-specific overlay entries.
- Position entries relative to detected reward card bounds.
- Show item name, platinum, ducats, volume, owned state, vaulted state, and highlight choice.
- Auto-hide or update according to scan state.
- Provide a fallback display mode when the compositor blocks click-through or precise positioning.

The overlay should receive already-processed `RewardScanResult` data. It should not run OCR or fetch market data.

## Service Boundaries

The app should be organized around long-lived services that communicate with typed messages.

```text
Warframe log watcher
        |
        v
Reward detection event
        |
        v
Screen capture -> OCR pipeline -> reward matching -> market enrichment
        |
        +--> overlay update
        |
        +--> application status/history update
        |
        +--> clipboard/log/debug output
```

Recommended services:

- `ConfigStore`: load/save TOML config.
- `LogWatcher`: locate and tail Warframe `EE.log`.
- `CaptureService`: capture the configured monitor/output or surface using Wayland-friendly APIs.
- `OcrService`: preprocess images and run Tesseract.
- `ScanCoordinator`: converts trigger events into OCR jobs and prevents duplicate/overlapping scans.
- `MarketDataService`: load/cache price, ducat, relic, equipment, and item-name data.
- `RewardMatcher`: match OCR text to known item names.
- `OverlayController`: send display updates to `iced_layershell`.
- `ClipboardService`: write formatted reward summaries when enabled.

## Configuration

Configuration must be persisted to disk as TOML.

Warframe monitor selection and Warframe UI theme are explicit configuration values. The app should not spend implementation effort on automatic monitor detection or automatic Warframe theme detection.

Only borderless fullscreen Warframe at a 16:9 aspect ratio is supported. Capture, OCR region detection, and overlay positioning may assume that display mode and aspect ratio. Other display modes or aspect ratios can fail with a clear unsupported-configuration status.

Suggested path:

- Config: `${XDG_CONFIG_HOME:-~/.config}/wf-info/config.toml`
- Data/cache: `${XDG_DATA_HOME:-~/.local/share}/wf-info/` and `${XDG_CACHE_HOME:-~/.cache}/wf-info/`
- Logs: `${XDG_STATE_HOME:-~/.local/state}/wf-info/wf-info.log`

Suggested config shape:

```toml
[app]
locale = "en"
start_minimized = false

[capture]
monitor = "primary"
capture_method = "portal"
display_mode = "borderless_fullscreen"
aspect_ratio = "16:9"

[scanner]
enabled = true
auto_delay_ms = 250
debug_images = false
debug_image_retention_hours = 12

[hotkeys]
activation = "F12"
dismiss_overlay = "F11"

[overlay]
enabled = true
x_offset = 0
y_offset = 0
duration_ms = 10000
high_contrast = false

[ocr]
language = "eng"
tesseract_data_path = ""
confidence_threshold = 0.0

[warframe]
log_path = ""
ui_theme = "lotus"

[logging]
level = "info"
file = ""
```

Implementation expectations:

- Use `serde` for config structs.
- Use a TOML crate for parsing/writing.
- Load defaults when the file is missing.
- Preserve valid user settings when adding new fields.
- Write config atomically by saving to a temporary file and renaming it.
- Validate paths and numeric ranges after load.
- Model `capture.monitor` as a user-selected target such as `primary`, a named monitor, or a stable configured output id.
- Validate `capture.display_mode` and `capture.aspect_ratio` against the supported values: borderless fullscreen and 16:9.
- Model `warframe.ui_theme` as a user-selected enum/string matching supported reward-screen preprocessing profiles.

## Settings UI

The `iced` application must include a settings window. Settings should be split into tabs that map to the TOML sections rather than one large form.

Initial tabs:

- App
- Capture
- Scanner
- Hotkeys
- Overlay
- OCR
- Warframe
- Logging

Requirements:

- Each tab edits one coherent config section.
- Invalid values should be rejected or repaired before saving.
- Save writes the full TOML config atomically.
- Cancel/close should leave the on-disk config unchanged.
- Applying capture, scanner, overlay, or OCR changes should notify the relevant services through typed events.
- Monitor and Warframe UI theme are chosen here; they are not auto-detected.

## Logging

Logging must use `env_logger`.

Requirements:

- Initialize logging once during binary startup.
- Respect `RUST_LOG`, with a sensible default such as `info`.
- Write logs to the terminal.
- Write logs to a file.
- Use color-coded status/severity in the terminal output.
- Include readable status/severity labels in the file output.
- Include timestamp, level/status, module target, and message.

Design notes:

- Create a `shared::logging::init(context)` function.
- Build an `env_logger::Builder` manually.
- Use a custom formatter for level/status colors in terminal output.
- Use a writer that tees formatted log lines to stderr and the configured log file.
- Prefer readable file logs without ANSI escape codes unless a later decision explicitly wants colored file output.
- Map app status to log levels consistently:
  - normal status: `info`
  - recoverable warning: `warn`
  - failed scan or failed fetch: `error`
  - verbose OCR/debug details: `debug` or `trace`

## OCR Architecture

OCR lives in `crates/ocr` and must be adaptable because multiple features scan the screen and feed images into Tesseract. Avoid a single reward-screen-only OCR function. Keep the general pipeline at the crate root and feature-specific scanners under `implementations`.

Use a pipeline with clear stages:

```text
CaptureRequest
    -> CapturedFrame
    -> RegionDetector
    -> ImagePreprocessor
    -> TesseractEngine
    -> TextCandidate
    -> Feature-specific parser/matcher
    -> Feature-specific result
```

Suggested traits:

```rust
pub trait CaptureProvider {
    fn capture(&self, request: CaptureRequest) -> Result<CapturedFrame>;
}

pub trait RegionDetector {
    fn detect_regions(&self, frame: &CapturedFrame) -> Result<Vec<ScanRegion>>;
}

pub trait ImagePreprocessor {
    fn preprocess(&self, frame: &CapturedFrame, region: &ScanRegion) -> Result<OcrImage>;
}

pub trait TextRecognizer {
    fn recognize(&self, image: &OcrImage, options: &OcrOptions) -> Result<Vec<TextCandidate>>;
}

pub trait FeatureScanner {
    type Output;

    fn scan(&self, frame: &CapturedFrame) -> Result<Self::Output>;
}
```

Feature scanners can then compose the same capture/OCR building blocks:

- `implementations::reward_screen::RewardScreenScanner`: detects reward cards and OCRs reward names into text candidates.
- `SnapItScanner`: OCRs a user-selected region.
- `MasterItScanner`: detects mastered equipment from a profile screenshot.
- `SearchIt` probably does not need screen OCR, but can reuse item matching and market lookup.

Tesseract integration:

- Keep Tesseract behind `TextRecognizer`.
- Do not expose raw Tesseract API types outside the OCR crate.
- Support per-feature OCR options:
  - language
  - whitelist/character set
  - page segmentation mode
  - confidence threshold
  - preprocessing profile
- Pool or serialize Tesseract usage if the binding is not safely reusable across threads.
- Save optional debug images at each pipeline stage when enabled.

## Reward Scan Flow

Phase 1 should use automatic log detection as the primary trigger.

1. `LogWatcher` finds and tails Warframe `EE.log`.
2. A known end-of-mission/reward-screen log pattern emits `RewardScreenDetected`.
3. `ScanCoordinator` waits for the configured delay.
4. `CaptureService` captures the relevant frame from the configured monitor/output.
5. `RewardScreenScanner` detects reward card regions.
6. OCR produces raw reward text candidates.
7. `RewardMatcher` maps candidates to canonical market item names.
8. `MarketDataService` enriches each reward.
9. The result is sent to the overlay and main application.
10. Optional clipboard/debug/log outputs are produced.

Manual activation should exist in Phase 1 as a fallback and debugging tool, not as the primary user workflow.

## Data and Cache

Market/relic/equipment data should be cached separately from user configuration.

Responsibilities:

- Fetch remote JSON from the known WFInfo and warframe.market endpoints.
- Store raw payload caches for offline fallback.
- Build normalized domain models for scanner and UI use.
- Keep user-owned counts/mastered flags in a user data file, not mixed into raw remote cache.
- Version cache schemas so future migrations are possible.

## Event Model

Use typed events rather than direct cross-module calls.

Example event categories:

- `AppEvent`: startup complete, shutdown requested, config changed.
- `ScannerEvent`: log trigger, scan started, scan completed, scan failed.
- `OverlayEvent`: show rewards, hide, update position, compositor fallback.
- `DataEvent`: cache load started, cache ready, cache failed, refresh requested.
- `StatusEvent`: user-visible status with severity.

The `iced` application can translate events into `Message` values. Background services can use async channels to avoid blocking the UI.

## Type Modeling

Prefer nested, tightly coupled enums over a single loose enum that tries to represent unrelated states.

Good shape:

```rust
pub enum ScannerEvent {
    Reward(RewardScanEvent),
    Log(LogWatcherEvent),
    Capture(CaptureEvent),
}

pub enum RewardScanEvent {
    Detected,
    Started,
    Completed(RewardScanResult),
    Failed(ScanError),
}
```

Avoid a broad enum such as `Event::RewardDetected`, `Event::CaptureFailed`, `Event::ConfigSaved`, and `Event::OverlayHidden` when those variants are only loosely related. Nested enums make ownership clearer, reduce invalid combinations, and keep match statements focused on the subsystem that actually understands the state.

## Extension Rules

- New screen-scanning features should implement `FeatureScanner` and reuse the OCR pipeline.
- New UI features should add application pages or panels without changing the overlay crate.
- Overlay changes should stay reward-screen-focused until another overlay use case is real.
- Config additions should be optional/defaulted and persisted through TOML.
- File/network/OCR failures should become typed errors, user-visible status events, and log entries.
