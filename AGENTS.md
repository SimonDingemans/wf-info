# Agent Instructions

This repository is a Rust rewrite of WFInfo for Linux desktops, with Wayland as
the primary target. Before making product or architecture decisions, read:

- `docs/FEATURE_LIST.md`
- `docs/ARCHITECTURE.md`

Keep those documents as the source of truth for the planned feature set,
workspace boundaries, service boundaries, configuration shape, OCR pipeline,
and phased product direction.

## Project Shape

The workspace is split into focused crates:

- `crates/wf-info`: binary entry point and startup wiring.
- `crates/shared`: shared domain, config, paths, logging, events, errors, and
  reusable infrastructure types. Keep this crate free of UI-framework code.
- `crates/ocr`: reusable capture/OCR pipeline types, Tesseract integration,
  and feature-specific OCR implementations such as reward-screen scanning.
- `crates/application`: main `iced` application UI.
- `crates/overlay`: reward overlay using `iced_layershell`.

Respect these boundaries. UI crates should not own OCR, Warframe log parsing,
market data loading, or long-running service logic. The OCR crate should keep
general pipeline code separate from feature-specific implementations and should
not own market data enrichment or UI state. The overlay should receive processed
reward scan data; it should not fetch market data or run OCR.

## Product Direction

The first product slice should stay focused on automatic end-of-mission reward
scanning:

- detect reward screens from Warframe logs;
- capture the configured screen/surface region;
- OCR reward names through a reusable Tesseract-backed pipeline;
- match and enrich rewards with market, ducat, vaulted, owned, and mastered
  data;
- display results in a reward-screen-specific overlay;
- report status and errors in the main app.

Future features such as relic browsing, equipment browsing, market automation,
Snap It, Search It, and Master It should be enabled through clean extension
points, not by coupling them into the initial reward scanner.

## Architecture Rules

- Use typed events and long-lived services for cross-crate communication.
- Prefer nested, subsystem-focused enums over broad loosely related event enums.
- Keep screen OCR reusable with clear pipeline stages:
  capture, region detection, preprocessing, recognition, parsing/matching, and
  feature-specific result construction.
- Persist configuration as TOML in the platform config directory.
- Cache remote WFInfo and warframe.market payloads separately from user-owned
  counts and mastered flags.
- Use `env_logger` for terminal and file logging.
- Model unsupported Wayland/compositor workflows as clear typed errors and
  user-visible status messages.
- Assume only borderless fullscreen Warframe at 16:9 unless the architecture
  document is updated.

## Rust Style

Write idiomatic Rust using standard Rust patterns. Do not import patterns from
other languages when Rust has a clear native shape.

- Prefer ownership, borrowing, enums, traits, pattern matching, iterators, and
  `Result`/`Option` over class-style hierarchies, null-like sentinel values,
  exception-style control flow, or stringly typed state.
- Use small, cohesive modules and explicit types at crate boundaries.
- Keep error handling typed and recoverable where appropriate.
- Avoid unnecessary abstraction; add traits or generic machinery only when they
  describe a real boundary or reduce meaningful duplication.
- Follow the existing workspace style and use `cargo fmt` before finishing code
  changes.
- lib.rs, mod.rs, and main.rs files must only be used to declare modules and re-export public APIs. 
- All business logic, services, and domain models must be separated into dedicated modules.
- Mutability should be limited to the direct structs or types of a function.
- An impl block may mutate the state of self.
- A function may not directly mutate the state of a used struct and should use an exposed function for that.

## Workflow

- Make small, focused changes that can become small commits.
- Do not commit changes yourself.
- At the end of each response that changes files, provide a suggested semantic
  commit message.
- Use semantic commit messages such as:
  - `docs: add agent contribution guidance`
  - `feat: add reward scan event model`
  - `fix: handle missing cached market data`
  - `refactor: split scanner pipeline traits`
  - `test: cover config validation defaults`
- Unit tests should be written as part of each commit.
- UI does not need automatic testing.

## Verification

For Rust changes, run the narrowest useful checks first, then broaden when the
blast radius calls for it:

- `cargo fmt`
- `cargo check`
- targeted tests when available
- `cargo test` for shared behavior or cross-crate changes

If a check cannot be run, explain why in the final response.
