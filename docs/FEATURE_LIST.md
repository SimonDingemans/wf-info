# wf-info Wayland/Linux Feature List

Source baseline: the existing `WFInfo` app behavior. This document is a compact feature inventory for the Rust rewrite targeting Linux desktops, with Wayland as the primary supported session.

## MVP

### App Shell

- Start a desktop application with shared app context, application UI, and overlay support.
- Keep the application UI modular and open to extension by later features such as relic browsing, equipment browsing, market automation, Snap It, Search It, and Master It.
- Persist user settings in the platform config/data directory.
- Show startup/loading status while OCR, market data, relic data, and equipment data initialize.
- Provide a main control surface with:
  - current app version
  - current data load state
  - reload/force-update data action
  - settings entry point
  - relic browser entry point
  - equipment browser entry point
  - exit/minimize behavior
- Write diagnostic logs to disk with timestamps and app version.
- Keep logging non-blocking where possible.

### Data Loading and Caching

- Download and cache Warframe/WFInfo data:
  - filtered relic/equipment data from `https://api.warframestat.us/wfinfo/filtered_items`
  - price sheet from `https://api.warframestat.us/wfinfo/prices`
  - warframe.market item metadata from `https://api.warframe.market/v2/items`
- Build local datasets for:
  - market item names and slugs
  - prime part platinum values
  - ducat values
  - trade volume
  - relic rewards
  - equipment sets and part counts
  - game-name to market-name translations
- Cache remote payloads and fall back to local cache when network fetches fail.
- Support forced refresh without corrupting existing usable cache.
- Preserve user-owned counts and mastered flags across data refreshes.
- Detect and expose vaulted relic/part state.
- Include ignored or special reward items from the upstream data.

### OCR Reward Processing

- Capture the Warframe surface or screen area on activation.
- Detect the void fissure reward area automatically.
- Extract one to four reward name regions.
- Run OCR over reward names.
- Match OCR output to known prime part names using fuzzy matching.
- Reject low-confidence or junk matches.
- For each detected reward, calculate/display:
  - corrected item name
  - platinum price
  - full-set price when available
  - ducat value
  - recent trade volume
  - vaulted marker
  - mastered marker
  - owned count versus required count
  - ducat-per-platinum efficiency
- Highlight best reward choices:
  - best platinum value
  - best ducat value
  - unowned/unmastered needed item
- Prevent overlapping reward processing runs.
- Save debug screenshots/crops when debug mode is enabled.
- Retain debug images only for the configured retention period.

### Overlay Display

- Provide an overlay specifically designed for the end-of-mission reward screen.
- Show one overlay entry per reward, positioned over or near reward cards.
- Support transparent/click-through overlay surfaces where the compositor allows it.
- Auto-hide overlays after a configurable delay.
- Allow dismissing overlays with the activation key plus Delete-equivalent shortcut.
- Support offset settings for overlay positioning.
- Support minimum/maximum overlay width settings.
- Support high-contrast overlay background.
- Hide price/ducat details for rewards with missing or non-standard market data.

### Hotkeys and Input

- Configurable activation key or mouse button.
- Modifier hotkeys for:
  - debug screenshot loading
  - Snap It
  - Search It
  - Master It
- Track recent user activity for AFK/status behavior.
- Route keyboard input to active search boxes/selection overlays.
- Close Snap It overlay when the user presses any key.

Wayland note: global hotkeys, pointer tracking, screenshots, and click-through overlays must use Wayland-friendly APIs, portals, or compositor-specific integrations. When the compositor blocks a workflow, provide a clear fallback and an actionable message.

### Settings

- Store user settings equivalent to the current app where useful for the Wayland/Linux product:
  - display mode
  - activation key
  - modifier keys
  - debug mode
  - locale
  - clipboard output
  - auto OCR delay
  - overlay delay
  - reward highlighting
  - vaulted clipboard marker
  - automatic reward detection/listing/counting toggles
  - high contrast
  - overlay X/Y offsets
  - configured capture monitor/output
  - OCR double-check option
  - efficiency thresholds
  - Snap It tuning values
  - HDR handling preference
  - configured Warframe UI theme
  - ignored item list
- Provide a settings window with tabs for each config section.
- Validate settings on load and repair invalid hotkey names to defaults.
- Keep settings backward-compatible within the Rust app once released.

### Relic Browser

- Present relics grouped by era:
  - Lith
  - Meso
  - Neo
  - Axi
  - Vanguard
- Toggle between era grouping and flat "all relics" view.
- Hide/show vaulted relics.
- Search/filter relics and rewards.
- Expand/collapse all.
- Sort by:
  - name
  - average intact platinum value
  - average radiant platinum value
  - radiant-minus-intact difference
- Show reward rarity buckets and value-derived info needed to choose relics.

### Equipment Browser

- Present prime equipment grouped by type:
  - Warframes
  - Primary
  - Secondary
  - Melee
  - Archwing
  - Companion
  - any additional upstream equipment categories
- Show each prime item, its parts, vaulted state, mastered state, owned count, required count, platinum value, and ducat value.
- Hide/show vaulted equipment.
- Toggle grouped versus flat equipment view.
- Search/filter equipment and parts.
- Sort by:
  - name
  - platinum value
  - missing parts
  - owned count
  - owned platinum value
  - owned ducat value
- Allow opening/clicking a known item in relevant lookup/listing flows.
- Reload visible owned/mastered information after count changes.

### Clipboard Output

- Optionally copy reward summaries to clipboard after OCR.
- Include item name, platinum value, optional set price, optional ducat/vaulted markers, and configurable footer/template.
- Keep output suitable for Warframe chat formatting.

### Localization

- Load localized item names from warframe.market where available.
- Keep English names as canonical market/data keys.
- Match OCR/user search against localized names when locale is not English.
- Support at least the language families represented in the original app:
  - English
  - European Latin-script languages
  - Cyrillic
  - Polish/Turkish special casing
  - Chinese
  - Japanese
  - Korean
  - Thai
- Use language-specific OCR whitelists or preprocessing where needed.

## Important Parity Features

### warframe.market Account Integration

- Login with warframe.market credentials/token flow if still supported by the current API.
- Store the JWT or replacement credential securely using the desktop secret service/keyring where possible.
- Remember-me support.
- Sign out and clear stored credentials.
- Validate saved session on startup.
- Set online/in-game/invisible status through warframe.market WebSocket/API.
- Reconnect WebSocket with backoff.
- Respect proxy environment settings.
- Set status invisible when Warframe closes.
- Set status invisible after configured inactivity/AFK period.
- Restore previous status when the user returns.

### Listing Helper

- Collect detected prime rewards from OCR results.
- Let the user choose the selected reward manually or from clicked reward location.
- Fetch current competing listings for each candidate item.
- Suggest a listing price from market context.
- Place a sell listing through warframe.market.
- Show listing success/failure state.
- Page through queued reward screens.
- Block known banned/unlistable items.

### Auto List

- After reward detection and user selection, prepare or post a listing automatically depending on settings.
- Use detected click position to choose the reward index.
- Queue multiple reward screens safely.

### Auto Count

- Queue detected reward choices for owned-count updates.
- Let the user increment all queued counts.
- Let the user remove all queued count updates.
- Save updated equipment JSON after count changes.
- Refresh equipment browser values after count updates.

### Snap It

- Capture the current screen or Warframe surface.
- Show a full-screen selection overlay.
- Let the user drag-select an arbitrary rectangle.
- OCR the selected area.
- Match recognized text to known market items.
- Show or export detected item/count information.
- Optional CSV/export support.
- Optional count update integration.
- Expose tuning settings for row/column density and number-box width.

### Search It

- Open a compact search box with a hotkey.
- Fuzzy-match typed user input against known market items.
- Open the listing helper for the best match.
- Require/login prompt for market actions that need authentication.

### Master It

- Capture/profile-process a screenshot to detect mastered equipment.
- Update equipment mastered flags.
- Support debug image loading for offline testing.

### Automatic Reward Detection

- Monitor Warframe log output and trigger reward OCR automatically after reward screen events.
- Apply configurable automatic delay.
- Support fixed delay fallback.
- Avoid duplicate processing for the same reward screen.

Wayland/Linux note: automatic detection should read Warframe logs directly, likely by tailing `EE.log` under the Proton/Steam prefix, native log path detection, or a user-configured log path.

### HDR and Theme Handling

- Detect or configure HDR behavior.
- Configure Warframe UI theme/color profile for reward-box extraction.
- Support built-in theme profiles and custom color filter ranges.
- Keep high-contrast mode independent from the configured Warframe theme.

### Debug/Test Utilities

- Load reward screenshots from files for offline OCR tests.
- Load Snap It screenshots from files.
- Load Master It screenshots from files.
- Save intermediate OCR crops.
- Spawn error dialogs with relevant log timestamp context.
- Keep OCR test runner style fixtures for regression testing.

## Wayland/Linux Product Requirements

- Surface and process discovery:
  - find the Warframe process and visible surface under native Linux and Proton
  - handle multiple monitors
  - handle per-monitor scaling and DPI
  - use the configured monitor/output instead of trying to auto-detect where Warframe is displayed
  - support only borderless fullscreen Warframe at 16:9
  - show a clear unsupported-configuration status for other display modes or aspect ratios
- Screenshot capture:
  - support Wayland capture through portals or compositor integrations
  - provide useful failure messages when compositor permissions block capture
  - account for HDR/color-space differences
- Overlay:
  - Wayland-native overlay behavior where available
  - graceful fallback to a separate result surface when click-through overlay is unavailable
- Hotkeys:
  - global shortcut support through desktop portals or compositor-specific integrations
  - documented limitations when a compositor does not expose the required capability
- Storage:
  - use XDG config/data/cache locations
  - use desktop keyring/secret service for credentials
- Packaging:
  - ship OCR language data or document installation
  - package for common Linux formats later, such as AppImage/Flatpak/deb/rpm

## Suggested Priority

### Phase 1: Usable Core

- Basic extensible application UI
- Settings persistence
- Data download/cache/fallback
- Warframe log discovery/tailing
- Automatic end-of-mission reward detection from logs
- Screenshot capture
- OCR reward processing for end-of-mission rewards
- Reward-screen-specific overlay
- Manual activation/debug trigger as a fallback
- Clipboard summary
- Debug image loading

### Phase 2: Quality and Parity

- Better localization
- Relic browser
- Equipment browser
- Auto Count
- Snap It
- Search It
- Master It
- Improved configured theme/HDR handling
- More robust multi-monitor scaling

### Phase 3: Market Automation

- warframe.market login/session storage
- WebSocket status management
- AFK status restore
- Listing helper
- Auto List
- Competitive listing lookup and posting

## Non-Goals for Early MVP

- Exact visual parity with the source application.
- Full Wayland compositor coverage on day one.
- Posting market listings before credential storage and API behavior are verified.
