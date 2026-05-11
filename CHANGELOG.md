# Changelog

All notable changes to stageLX are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.0] — 2026-05-10

### Added
- **Workspace refactor**: 8-crate workspace with clean dependency graph (`stagelx-core`, `stagelx-patch`, `stagelx-show`, `stagelx-gdtf`, `stagelx-dmx`, `stagelx-io`, `stagelx-render`, `stagelx-ui`).
- **DMX I/O**: Art-Net TX/RX with node discovery (ArtPoll/ArtPollReply), sACN TX/RX with multicast join, USB DMX (Enttec), MIDI input, OSC input.
- **Volumetric beam rendering**: Ray-marched cone shader with 3-tier LOD, front-to-back sorting, split-screen viewports (FOH + top + side).
- **MVR import/export**: Scene fixture placement, truss/structure geometry, embedded 3D model extraction (GLB/OBJ/3DS/FBX).
- **GDTF geometry loading**: Real fixture body/yoke/head geometry from embedded 3DS, async loading with deduplication cache.
- **Cue system foundation**: `CueStack`, `CuePlayhead`, cue panel UI, JSON persistence, keyboard shortcuts (`Enter` = GO).
- **Performance profiling**: Per-system CPU timing in `PerfDiagnosticsRes`, IO staleness timestamps, synthetic benchmarks (`spawn_500`, `flood_artnet`).
- **Test corpus**: 10 synthetic GDTF parser unit tests + optional corpus integration test.

### Changed
- `stagelx-state` mechanically split into `stagelx-patch` + `stagelx-show` (Phase 6.1).
- All IO transports now implement `IoSource` / `IoSink` traits with unified shutdown and drop counting.
- DMX projection moved from `stagelx-io` to `stagelx-dmx` with pre-computed `DmxChannelMap`.

### Fixed
- sACN/Art-Net RX `EAGAIN` (os error 35) after start/stop — `WouldBlock` + `Interrupted` treated as transient.
- Beam material change detection gating (eliminates per-frame GPU uniform uploads).
- Per-frame allocation ban enforced in LOD and DMX engine systems.
- All 56 UI audit items from Phase 5 resolved; zero compiler warnings.

### Changed
- **UI layout refactor**: DMX I/O panel moved from embedded bottom strip to dedicated `SidePanel::right` (420 px). Patch + Library promoted from inline `CentralPanel` scope to `TopBottomPanel::bottom` (248 px). CentralPanel now contains viewports only. `io_panel_system` merged into `ui_root_system` via new `IoParams` `SystemParam`.

### Fixed
- Detached windows no longer expand infinitely — `search_input` frame margin capped, `ScrollArea` uses inner width, toolbar chip sizing constrained, programmer/cue/IO node lists capped with `max_height`/`ScrollArea`.
- All detached windows receive `.default_height()` and `.min_width(240.0)` constraints.

### Known issues
- `paste` crate (transitive via `bevy` → `wgpu` → `metal`) is unmaintained per RUSTSEC-2024-0436. Accepted pending Bevy upgrade.
- wgpu timestamp queries for GPU pass timing deferred (requires custom render node).
