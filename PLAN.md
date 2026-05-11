# stageLX — Project Plan

A real-time 3D stage lighting visualizer and DMX controller written in Rust.
Supports GDTF fixture definitions, MVR scene files, and full DMX I/O.

**Version:** 0.2.0-phase7  
**Last updated:** 2026-05-10 (Phase 7 complete — all sub-phases 7.1–7.6 done)

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Architecture Overview](#architecture-overview)
3. [Cargo Workspace Structure](#cargo-workspace-structure)
4. [Key Crate Dependencies](#key-crate-dependencies)
5. [GDTF Data Model](#gdtf-data-model-key-concepts)
6. [Rendering Strategy](#rendering-strategy)
7. [DMX Engine](#dmx-engine)
8. [I/O Protocol Details](#io-protocol-details)
9. [Architecture Decision Records](#architecture-decision-records)
10. [Cue System Architecture](#cue-system-architecture)
11. [Implementation Phase Archive](#implementation-phase-archive)
12. [Phase 6 — Production Hardening & Cue Foundation](#phase-6--production-hardening--cue-foundation)
13. [Phase 7 — Show Playback & Stage Capture](#phase-7--show-playback--stage-capture)
14. [Open Questions / Decisions Needed](#open-questions--decisions-needed)
15. [Non-Goals (v1)](#non-goals-v1)
16. [Repository Setup](#repository-setup)
17. [Changelog](#changelog)

---

## Executive Summary

stageLX is a desktop application for real-time 3D visualization of stage lighting rigs. It parses GDTF fixture definitions to understand DMX behaviour and 3D geometry, imports MVR scene files for show interoperability, and drives real DMX hardware via Art-Net, sACN, and USB protocols.

**Current state (end of Phase 7.6):**
- Workspace of 8 crates with clean dependency graph
- Full DMX I/O: Art-Net Tx/Rx, sACN Tx/Rx, USB (Enttec), MIDI In, OSC In
- Volumetric beam rendering with 3-tier LOD and split-screen viewports
- MVR import/export with real GDTF geometry loading (3DS/OBJ/GLB/FBX venue support)
- 56 UI audit items resolved; zero compiler warnings
- `stagelx-state` mechanically split into `stagelx-patch` + `stagelx-show`
- Cue system foundation: `CueStack`, `CuePlayhead`, cue panel UI, JSON persistence
- Show-ready playback: cross-fade interpolation (`CueValues::lerp`), per-fixture stage capture from live DMX output
- Full DMX attribute coverage: gobo, gobo rotation, color wheel forward/inverse projection
- Cue editing workflow: load into programmer, UPDATE button, inline fade-time + label editing
- MIDI/OSC external triggering: Note On → GO/Back, `/cue/go` and `/cue/back` OSC paths
- Unified `.slx` show file format: bundles patch, cues, venue path; backward-compat with legacy `show.json`
- Live status bar: FPS, CPU%, universe usage, venue name from real diagnostics
- Per-system CPU timing in `PerfDiagnosticsRes`; IO staleness timestamps; synthetic benchmarks
- Zero clippy warnings; `cargo audit` documented; README and CHANGELOG updated

**Phase 6 focus:** Production hardening (test corpus, profiling, protocol completeness), mechanical crate extraction (`stagelx-state` → `stagelx-show` + `stagelx-patch`), and cue-system foundation. All sub-phases 6.1–6.8 complete.

**Phase 7 focus:** Make cue playback show-ready with cross-fade interpolation, per-fixture capture from live DMX output, complete DMX attribute coverage (gobo/color wheel), cue editing workflow, MIDI/OSC external triggering, and unified `.slx` show file persistence. All sub-phases 7.1–7.6 complete.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                     stageLX (Bevy app)                  │
│                                                         │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────┐ │
│  │   3D Render  │  │  Programmer  │  │  Scene Editor │ │
│  │  (wgpu/Bevy) │  │  (egui)      │  │  (egui)       │ │
│  └──────┬───────┘  └──────┬───────┘  └───────┬───────┘ │
│         │                 │                  │         │
│  ┌──────▼─────────────────▼──────────────────▼───────┐ │
│  │                  Core ECS / Event Bus              │ │
│  │         (Bevy World — fixtures, patch, cues)       │ │
│  └──────┬────────────────────────────────────────────┘ │
│         │                                              │
│  ┌──────▼──────────────────────────────────────────┐  │
│  │               I/O Layer                         │  │
│  │  Art-Net Out │ sACN Out │ USB DMX │             │  │
│  │  Art-Net In  │ sACN In  │ MIDI In │ OSC In      │  │
│  └─────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
         ▲                              ▲
   GDTF / MVR files              Hardware / Network
```

---

## Cargo Workspace Structure

```
stageLX/
├── Cargo.toml                 # workspace root
├── PLAN.md
│
├── crates/
│   ├── stagelx-core/          # shared types: patch, universe, fixture instance
│   ├── stagelx-patch/         # patch data: PatchRes, PatchEditState, DmxAddress
│   ├── stagelx-show/          # show state: Programmer, CueStack, events, diagnostics
│   ├── stagelx-gdtf/          # GDTF + MVR parser + export
│   ├── stagelx-dmx/           # DMX frame engine + HTP/LTP merge + projection
│   ├── stagelx-io/            # Art-Net, sACN, USB, MIDI, OSC
│   ├── stagelx-render/        # Bevy plugin: 3D scene, beams, gobos, fog, viewports
│   └── stagelx-ui/            # egui panels: patch, programmer, cue, scene, I/O
│
└── src/
    └── main.rs                # Bevy App wiring
```

### Crate Responsibilities

| Crate | Responsibility |
|---|---|
| `stagelx-core` | `FixtureInstance`, `Patch`, `Universe`, `DmxBuffer`, `DmxChannelMap`, attribute model |
| `stagelx-patch` | `PatchRes`, `PatchEditState`, `DmxAddress`, `SpawnFixtureEvent`, `DespawnFixtureEvent` |
| `stagelx-show` | `Programmer`, `CueStack`, `Cue`, `PerfDiagnosticsRes`, `LoadVenueEvent`, `FixtureLibraryRes` |
| `stagelx-gdtf` | Parse `.gdtf` (ZIP+XML), geometry trees, DMX modes, wheels, physicals; MVR import/export |
| `stagelx-dmx` | DMX frame generation, merge strategies (HTP/LTP), `DmxEngine`, programmer→DMX projection, cue→DMX projection |
| `stagelx-io` | Art-Net Tx/Rx, sACN Tx/Rx, USB serial (Enttec), MIDI, OSC; per-protocol `*Config` + `*Stats` |
| `stagelx-render` | Bevy plugin: volumetric beams, gobo projection, color, fog medium, split-screen viewports, LOD |
| `stagelx-ui` | egui: patch editor, programmer, cue panel, fixture library, DMX I/O panel, venue loader |

### Dependency Graph

```
                         stagelx-core
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
   stagelx-gdtf          stagelx-dmx          stagelx-patch
   (→ core)              (→ core)             (→ core)
        │                     │                     │
        │                     ▼                     ▼
        │              stagelx-io (→ dmx)     stagelx-show (→ patch, gdtf)
        │                     │                     │
        └─────────────────────┴─────────────────────┘
                              ▼
                         stagelx-ui
              (→ core, gdtf, patch, show, io, render)
```

**Invariants:**
- `stagelx-core` has zero internal dependencies — it is the type foundation.
- `stagelx-gdtf` and `stagelx-dmx` are independent mid-layer crates.
- `stagelx-patch` depends only on `core`; it provides patch data and fixture lifecycle events.
- `stagelx-show` depends on `patch` + `gdtf`; it provides show control state, cue data, and diagnostics.
- `stagelx-io` and `stagelx-render` are independent leaf feature crates.
- `stagelx-ui` is the sole integrator; it may read from `io` and `render` but never vice-versa.
- No cycles. No `tokio`.

### Phase 6 Target Graph (Achieved)

```
                         stagelx-core
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
   stagelx-gdtf          stagelx-dmx          stagelx-patch
   (→ core)              (→ core)             (→ core)
        │                     │                     │
        │                     ▼                     ▼
        │              stagelx-io (→ dmx)     stagelx-show (→ patch, gdtf)
        │                     │                     │
        └─────────────────────┴─────────────────────┘
                              ▼
                         stagelx-ui
              (→ core, gdtf, patch, show, io, render)
```

`stagelx-state` was mechanically split into:
- `stagelx-patch` → `PatchRes`, `PatchEditState`, `DmxAddress`, `FixtureInstance`, `SpawnFixtureEvent`, `DespawnFixtureEvent`
- `stagelx-show` → `Programmer`, `CueStack`, `Cue`, `PerfDiagnosticsRes`, `LoadVenueEvent`, `FixtureLibraryRes`

`stagelx-render` depends on `stagelx-show` (for events) and `stagelx-patch` (for patch data).
`stagelx-ui` depends on both new crates.

---

## Key Crate Dependencies

| Purpose | Crate |
|---|---|
| App/ECS/rendering | `bevy` 0.18.0 |
| Low-level GPU | `wgpu` (via Bevy) |
| UI panels | `bevy_egui` 0.39.1 + `egui` |
| GDTF/MVR ZIP parsing | `zip`, `quick-xml` |
| 3D model loading (venue / GDTF geometry) | `tobj`, `gltf`, `ufbx` |
| 3DS format (fixture geometry) | `ds3` (path dep `../3ds-rs`) |
| Art-Net | custom UDP (no external crate) |
| sACN (E1.31) | custom UDP per ANSI E1.31-2016 (no external crate) |
| I/O thread bridge | `crossbeam-channel` (bounded 8, no tokio) |
| USB/serial DMX | `serialport` |
| MIDI | `midir` |
| OSC | `rosc` |
| Image loading (gobos) | `image` (via Bevy asset loader) |
| File dialogs | `rfd` |
| egui extras | `egui_extras` |

---

## GDTF Data Model (Key Concepts)

GDTF files are ZIP archives containing `description.xml` plus assets:

```
FixtureType
├── DMXModes[]           # e.g. "Basic 16ch", "Extended 32ch"
│   └── DMXChannels[]    # one per attribute instance
│       └── LogicalChannel → Attribute (Pan, Tilt, Dimmer, ColorAdd_R, Zoom…)
├── Geometries           # 3D hierarchy (Body → Yoke → Head for moving heads)
│   └── Beam             # defines beam angle, beam type, color temperature
├── Wheels[]             # ColorWheel, GoboWheel, EffectWheel
│   └── Slots[]          # individual colors/gobos with image assets
└── PhysicalDescriptions
    └── ColorRendering, Emitters, Filters
```

MVR files extend this with scene context:
- Fixture instances with 3D transform (position + rotation)
- Focus positions
- Truss / structure geometry
- Patch assignments

---

## Rendering Strategy

### Beams
- Render each beam as a **cone mesh** with additive blending
- Color = mixed output of ColorAdd_R/G/B channels (or color wheel slot color)
- Beam angle = Zoom attribute value mapped to `beam_angle` from GDTF physical
- Use **ray-marched volumetric cone shader** (wgpu custom shader) for fog effect
- Atmosphere/fog density controlled per-scene
- **LOD tiers** (see ADR-008):
  - Tier 0 (<50 px screen radius): billboard sprite — zero ray-march cost
  - Tier 1 (50–200 px): half-res offscreen (960×540), 16 ray-march steps, bilinear upsample
  - Tier 2 (>200 px): full-res, hard cap 32 steps. 64+ steps reserved for single selected "hero" fixture only.
- Hard cap: 64 simultaneous ray-marched beams regardless of patch size
- Front-to-back beam sorting (`sort_beams_front_to_back`) with `depth_bias` for correct additive blending

### Gobos
- Load gobo images from GDTF wheel slot assets
- Project as texture via **projected spotlight** in Bevy PBR + custom shader
- Rotate with GoboIndex + GoboRotation attributes

### Fixture Geometry
- Load GDTF geometry from embedded 3DS/GLB/OBJ → convert to Bevy mesh via `ds3::to_bevy_buffers()`
- Articulate geometry nodes: yoke rotates with Pan, head rotates with Tilt
- Fallback: procedural geometry (box for body, cylinder for head) when no model
- Async asset loading with deduplication cache by `(gdtf_uuid, sub_mesh_index)`

### Color
- Additive CMY/RGB mixing in shader
- CIE color space for mixed colors when GDTF emitter data available
- Gel/filter colors from ColorWheel slot CIE values

---

## DMX Engine

- **Universe buffer**: `[u8; 512]` per universe, up to 64 universes
- **Merge**: HTP (Highest Takes Precedence) across input sources by default; configurable per universe
- **Priority stack**: Programmer (200, LTP) > Cue playback (150, LTP) > External input (100, HTP). Higher priority wins.
- **Output tick**: 44 Hz (standard DMX refresh), driven by Bevy `FixedUpdate`
- **Attribute → DMX**: Pre-computed `DmxChannelMap` on each `FixtureInstance` eliminates per-tick string lookups (ADR-004). Projection lives in `stagelx-dmx::projection`.

### ECS Pipeline (per `FixedUpdate` tick)

```
programmer_to_dmx ──┐
cue_to_dmx ─────────┼→ dmx_engine_tick → artnet_send → sacn_send → usb_send
       │            │          │
       └─ LTP───────┘          └─ merges all sources (HTP/LTP), outputs to transports
```

---

## I/O Protocol Details

### Art-Net (Output + Input)
- UDP port 6454, custom ArtDMX implementation (no external crate)
- ArtDMX packet TX at 44 Hz via Bevy `FixedUpdate`
- RX via blocking socket cloned from TX socket (`try_clone` avoids EADDRINUSE)
- RX thread bridges to Bevy via `crossbeam-channel` (bounded 8)
- Source IP allowlist (cached, rebuilt only on config change)
- Universe cap: 64 universes per source (amplification attack mitigation)
- Configurable TX destination (default: limited broadcast 255.255.255.255)
- **ArtPoll/ArtPollReply:** Node discovery with `ArtNetNodeTable` Resource; UI toggle and node list in IO panel

### sACN / E1.31 (Output + Input)
- UDP port 5568, full ANSI E1.31-2016 Data Packet (638 bytes)
- TX: rolling sequence counter, multicast 239.255.hi.lo or configurable unicast
- RX: same clone-and-block pattern as Art-Net
- Priority 100 default, configurable; universe cap 64
- **Multicast join:** `socket2::join_multicast_v4()` for explicit `IP_ADD_MEMBERSHIP` on sACN RX sockets

### USB DMX (Output)
- Enttec DMX USB Pro protocol over serial (`serialport` crate)
- Single universe per device
- Auto-detect by USB VID/PID

### MIDI (Input)
- `midir` for cross-platform MIDI device access
- Configurable mapping: CC → attribute, Note → cue trigger
- Rate-limited port scan ≤ 1 Hz (ADR-009)
- Run in `midir` callback, forwarded to Bevy event queue via crossbeam

### OSC (Input)
- UDP, default port 8000
- Path schema: `/fixture/{id}/{attribute}` → float value
- `/cue/{id}/go` for cue triggers
- Shutdown path: `UdpSocket` wrapped in `Arc`, `shutdown(Shutdown::Both)` on disable

---

## Architecture Decision Records

Binding design rules for Phase 5 and beyond. Violations are treated as regressions.

---

### ADR-001: Freeze `stagelx-state` — No New Resources

**Status:** ✅ Adopted → Completed  
**Rule 1**

No new Bevy Resources may be added to `stagelx-state`. The crate became a dependency sink (`io` + `render` + `ui` all import it). Adding MIDI config, viewport state, or export staging here would cement it as a god-crate.

**Routing for new state:**
- MIDI/OSC config → `stagelx-io::config`
- Viewport layout → `stagelx-render`
- MVR export staging → `stagelx-gdtf::mvr_export`

**Resolution:** `stagelx-state` was mechanically extracted into `stagelx-patch` + `stagelx-show` (Phase 6.1). The old crate was deleted.

---

### ADR-002: Runtime-Agnostic Format Crates

**Status:** ✅ Adopted  
**Rule 2**

Format crates (`stagelx-gdtf`, `ds3`) must be runtime-agnostic. They expose `Vec<[f32; 3]>`, index buffers, and materials-as-data — no Bevy types.

`stagelx-3ds` was deleted and replaced with `ds3` (standalone `no_std` crate). The Bevy-aware adapter (`to_bevy_buffers`) lives in `stagelx-render::adapters::three_ds`.

---

### ADR-003: IO Thread Abstraction

**Status:** ✅ Complete  
**Rules 3, 9, 10, 12**

All IO transports must share a common contract. No per-transport bespoke thread management.

- `IoSource`, `IoSink` traits and `IoSupervisor` Resource defined in `stagelx-io::supervisor`.
- RX channel depth: 8 slots (reduced from 256).
- RX threads must use `try_send()`, not `send()`. On `TrySendError::Full`, increment `IoSupervisor::rx_drops` and continue.
- All transport threads must have a shutdown path (`Arc<AtomicBool>` + 100 ms recv timeout) to prevent `EADDRINUSE` on re-enable.
- `IoOverflowWarning` Bevy event must be emitted when `rx_drops` increments.

**Resolution:** All transports implement `IoSource`/`IoSink`. Art-Net, sACN, USB, OSC use common trait contract. `socket2` tunes UDP sockets (`SO_REUSEADDR`, `SO_RCVBUF=4MB`). RX threads use `try_send()` with drop counting. `IoOverflowWarning` emitted via `Commands::trigger()`. TX decoupled to background threads.

---

### ADR-004: DMX Projection Ownership

**Status:** ✅ Adopted  
**Rule 20**

Attribute→DMX channel projection is the DMX engine's responsibility. It was moved from `stagelx-io::artnet` to `stagelx-dmx::projection`, eliminating `stagelx-io`'s dependency on `stagelx-gdtf`.

Each `FixtureInstance` carries a pre-computed `DmxChannelMap` (`[Option<u16>; 8]` per attribute), computed once at patch load time. This eliminates ~22,500 string comparisons/sec at 64 fixtures × 44 Hz.

---

### ADR-005: Per-Frame Allocation Ban

**Status:** ✅ Adopted  
**Rule 13**

ECS systems running at frame rate or DMX tick rate must not allocate `Vec` or `HashMap` values that are discarded each tick. Use `Local<T>` to persist allocations across frames; rebuild only on structural changes.

Resolved violations:
- `lod.rs:209` — `Vec<(Entity, f32, BeamLodTier)>` → `Local<Vec<...>>`
- `lod.rs:267` — `HashMap<FixtureId, Entity>` → `Local<HashMap<...>>`
- `engine.rs:51` — `HashSet<u16>` + `Vec<u16>` → cached in `DmxEngine` with dirty flag

---

### ADR-006: Beam Material Change Detection

**Status:** ✅ Adopted  
**Rule 14**

`articulate_beams` must gate material writes behind actual change detection. `beam_materials.get_mut(handle.id())` marks the Bevy asset dirty unconditionally, triggering a GPU uniform upload for every fixture every frame.

Resolution: filter on `Changed<GlobalTransform>` and a cached last-programmed value; call `get_mut` only when a change is detected.

---

### ADR-007: Release Profile Optimisation

**Status:** ✅ Adopted  
**Rule 16**

Workspace root `Cargo.toml` defines `[profile.release]` with `lto = "thin"` and `codegen-units = 1`. The `merge_htp`/`merge_ltp` loops in `stagelx-core::universe` are 512-byte SIMD candidates; cross-crate inlining is required for vectorization.

Also: `[profile.dev.package."*"] opt-level = 2` keeps dependencies optimized during development builds.

---

### ADR-008: LOD Tier Stability

**Status:** ✅ Adopted  
**Rules 18, 19**

`evaluate_beam_lod` must not flip a fixture's tier on a single frame.

- **Hysteresis:** ±10 px band around each threshold. Promote when `radius > threshold + 10`, demote only when `radius < threshold - 10`.
- **Resize:** `BeamRenderTarget` must be recreated on `WindowResized`, parallel to `update_viewports_on_resize`.

---

### ADR-009: MIDI Rate Limiting

**Status:** ✅ Adopted  
**Rule 15**

MIDI port scan must be rate-limited to ≤ 1 Hz. `MidiInput::new("stageLX-scan")` invokes platform MIDI subsystem APIs and must not be called in `Update` (60 Hz). Use a `Local<Timer>` or elapsed-time accumulator.

---

### ADR-010: Fixed-Size Packet Buffers

**Status:** ✅ Adopted  
**Rule 11**

`ReceivedPacket.data` must be a fixed-size `[u8; 512]` array field. The old `Arc<Vec<u8>>` pattern caused ~3,200 heap allocations/sec at high universe count with no benefit (data is consumed once and dropped).

---

### ADR-011: UI Decoupling & Frontend Fidelity

**Status:** ✅ Adopted  
**Rules 21–28**

- **Rule 21 / 23:** `VenueLoadState` lives in `stagelx-show`. `LoadVenueEvent` breaks cross-leaf dependency. Library search query is independent of the GDTF import path field.
- **Rule 22:** `IoConfig` split into per-protocol `*Config` + `*Stats` Resources, eliminating scheduler contention.
- **Rule 24:** All `ComboBox` widgets use unique egui ids (`from_id_salt`).
- **Rule 25:** TX/RX counter reflects the active protocol (`*Stats` Resource), not hardcoded Art-Net.
- **Rule 26:** `UiLayoutState::show_status_bar` defaults to `true`.
- **Rule 27:** Programmer selection bar resolves fixture names from `PatchRes`.
- **Rule 28:** Encoder and fader readouts use spec font sizes (`FontId::monospace(18.0)` / `14.0`).

---

### ADR-012: ECS Lifecycle Discipline

**Status:** ✅ Adopted  

- **Observers** (`On<SpawnFixtureEvent>`, `On<DespawnFixtureEvent>`, `On<LoadVenueEvent>`): use for one-shot lifecycle work (spawn child entities, set initial transforms, attach geometry).
- **ECS query systems with explicit `SystemSet` ordering**: use for steady-state per-frame updates (`DmxIngest → DmxMerge → FixtureApply → Render`). Do not use observers for per-frame attribute updates.

---

## Cue System Architecture

*Not in scope for v1, but the data model must not foreclose adding a cue stack later.*

### Proposed Data Model (Phase 6 Foundation)

```
CueStack
├── Cue[]
│   ├── id: String
│   ├── fade_in_ms: u32
│   ├── fade_out_ms: u32
│   ├── delay_ms: u32
│   └── Snapshot (fixture_id → attribute values)
│
└── Playhead
    ├── current_cue_index: usize
    ├── next_cue_index: Option<usize>
    └── state: Idle | Fading { progress: f32 }
```

### Integration Points

1. **DMX Engine**: `DmxEngine` already supports named sources. A cue playback source is just another source in the priority stack (below programmer, above external input).
2. **Programmer**: "Record" button captures current programmer state into a cue snapshot. "Update" overwrites the active cue with current values.
3. **UI**: New "Cues" panel alongside Programmer. List view + GO/Back buttons. No timeline (v2 feature).
4. **Persistence**: Cue stacks serialize to a simple JSON/YAML format. Not tied to MVR (MVR has no cue concept).

### Phase 6 Scope

- ✅ `CueStack` and `Cue` types in `stagelx-show`
- ✅ `cue_to_dmx()` system in `stagelx-dmx` (priority 150, between programmer=200 and external=100)
- ✅ Basic UI: list cues, GO / BACK / LOAD buttons
- ✅ Keyboard shortcuts: `Enter` = GO, `Shift+Enter` = BACK
- ✅ Save/load cue stack to `.json`

### Deferred to v2

- Cross-fade timing (follow/hang times)
- Effect engine (chases, shapes)
- MIDI/OSC trigger mapping to cue GO

---

## Implementation Phase Archive

Completed phases are summarized here for historical reference. Detailed audit findings and fix lists are preserved in the git history (commits `fbb7aa5`, `2dc1d79`).

### Phase 1 — Foundation ✅
Cargo workspace scaffold; GDTF ZIP/XML parser; core types (`FixtureType`, `FixtureInstance`, `Patch`, `Universe`, `DmxBuffer`); procedural fixture geometry; basic egui panels; `DmxEngine` with HTP/LTP merge.

### Phase 2 — Programmer + Beam Rendering ✅
Programmer UI (dimmer, pan/tilt, RGB, zoom, strobe, gobo); beam cone mesh with additive blending and custom WGSL shader; gobo projection; zoom→beam scaling; keyboard programmer.

### Phase 3 — DMX I/O ✅
Art-Net Tx/Rx; sACN Tx/Rx; HTP merge across input sources; universe/port configuration UI; security hardening (source allowlist, universe cap); `stagelx-state` established as shared resource hub (later split in Phase 6).

### Phase 4 — MVR + GDTF Geometry + USB DMX ✅
USB DMX output (Enttec USB Pro); MVR parser (scene positions, fixture placement); `ds3` 3DS parser; observer-based fixture lifecycle; GDTF-driven DMX channel mapping; patch add-fixture UI; MVR export.

### Phase 5 — Geometry, I/O Surfaces + Advanced Rendering ✅
MIDI input (`midir` + crossbeam); OSC input (`rosc`); MIDI + OSC config UI; MVR export (ZIP writer); ray-marched volumetric fog cone; three-tier LOD with hard cap 64; front-to-back beam sorting; split-screen viewports (FOH + top + side); camera orbit/pan; FBX venue loader; async file dialogs; zero compiler warnings; all UI audit items resolved.

---

## Phase 6 — Production Hardening & Cue Foundation

**Goal:** Harden the application for real show use, complete the `IoSource` abstraction, split `stagelx-state`, and lay the cue-system foundation. Sub-phases 6.1–6.4 are complete; 6.5–6.8 remain open.

**Timeline:** Weeks 21–26

---

### 6.1 Crate Refactoring: `stagelx-state` → `stagelx-show` + `stagelx-patch` ✅

**Rationale:** `stagelx-state` mixed show control state and patch data. Splitting aligns with bounded contexts and reduces compile-time coupling.

**Migration (mechanical — no logic changes):**

| New Crate | Contents | Dependencies |
|---|---|---|
| `stagelx-patch` | `PatchRes`, `PatchEditState`, `SpawnFixtureEvent`, `DespawnFixtureEvent`, `DmxAddress`, `FixtureInstance` re-exports | `stagelx-core` |
| `stagelx-show` | `Programmer`, `PerfDiagnosticsRes`, `LoadVenueEvent`, `CueStack`, `Cue`, `CuePlayhead`, `FixtureLibraryRes` | `stagelx-core`, `stagelx-patch`, `stagelx-gdtf` |

**Completed:**
1. ✅ Created `crates/stagelx-patch/` and `crates/stagelx-show/`
2. ✅ Updated 6 `Cargo.toml` files, 13 `.rs` files
3. ✅ Deleted `stagelx-state` crate entirely
4. ✅ `cargo check --workspace` passes with zero warnings

---

### 6.2 IO Transport Formalisation ✅

**Goal:** Complete ADR-003. All transports implement `IoSource` / `IoSink`.

**Completed:**
- ✅ `IoSource` / `IoSink` trait definitions in `stagelx-io::supervisor` (instance methods, `&self`)
- ✅ `ArtNetRxSource` / `ArtNetTxSink` — full Art-Net TX/RX with trait impls
- ✅ `SacnRxSource` / `SacnTxSink` — full sACN TX/RX with trait impls
- ✅ `OscRxSource` / `OscTxSink` — OSC TX/RX with trait impls
- ✅ `UsbTxSink` — USB DMX TX with trait impl
- ✅ `IoSupervisor::rx_drops` / `tx_drops` increment on `TrySendError::Full`
- ✅ `IoOverflowWarning` Bevy event emitted via `Commands::trigger()`
- ✅ `socket2` for `SO_REUSEADDR` + `SO_RCVBUF=4MB` on UDP sockets
- ✅ TX decoupled to background threads (no frame drops on backpressure)
- ✅ USB converted from `NonSend` to `Resource`

**Success metric:** Enabling/disabling a protocol 100 times in a loop never produces `EADDRINUSE`.

---

### 6.3 Protocol Completeness ✅

**Art-Net node discovery:**
- ✅ `ArtPoll` broadcast every 3 s when `discovery_enabled`
- ✅ `ArtPollReply` parser with full node metadata (name, IP, ports, universes, status)
- ✅ `ArtNetNodeTable` Resource with version counter for change detection
- ✅ UI: discovery toggle + scrollable node list in IO panel

**sACN multicast join:**
- ✅ `socket2::join_multicast_v4()` for explicit `IP_ADD_MEMBERSHIP` on RX socket
- ✅ Multicast group `239.255.hi.lo` per configured RX universe
- ✅ Graceful fallback when join fails

---

### 6.4 Cue System Foundation ✅

**Data model (in `stagelx-show`):**
- ✅ `Cue` struct: id, label, fade_in_ms, snapshot (`HashMap<FixtureId, CueValue>`)
- ✅ `CueValue` enum covering dimmer/pan/tilt/color/zoom/strobe/gobo
- ✅ `CueStack` struct: ordered cue list + playhead state (`Idle` / `Fading`)
- ✅ `CuePlayhead` Resource tracking active cue index and fade progress
- ✅ `CuePlaybackRes` DMX source wrapper; `cue_to_dmx()` system in `stagelx-dmx` (priority 150, LTP)

**Events & observers (Bevy 0.18 observer pattern):**
- ✅ `RecordCueEvent`, `GoCueEvent`, `BackCueEvent`, `DeleteCueEvent`
- ✅ Observer handlers in `stagelx-show::cue`

**UI (in `stagelx-ui::cue`):**
- ✅ Cue panel in left rail alongside Programmer (dockable/minimizable)
- ✅ Cue list table with active-cue highlighting
- ✅ GO / BACK / RECORD buttons
- ✅ Keyboard shortcuts: `Enter` = GO, `Shift+Enter` = BACK
- ✅ `PanelKind::Cue` variant in layout system

**Persistence:**
- ✅ `serde` serialize/deserialize `CueStack` to `show.json`
- ✅ `load_cue_stack()` / `save_cue_stack()` helpers

---

### 6.5 Truss / Structure Geometry from MVR ✅

**Deferred from Phase 4.**

- ✅ Parse `Truss` and `SceneObject` elements from MVR `GeneralSceneDescription.xml`
- ✅ Parse `Geometry3D` references with `fileName` attribute
- ✅ Load associated 3D models (GLB/OBJ/3DS) referenced by MVR
- ✅ Extract geometry from MVR ZIP to temp directory during import
- ✅ Render as static opaque geometry under `VenueRoot`
- ✅ Reuse existing venue loaders (OBJ/GLB/FBX) via `spawn_*_meshes` helpers
- ✅ Apply per-object `Matrix` transform (MVR Z-up → Bevy Y-up, mm → metres)

**Architecture:** Follows `LoadVenueEvent` pattern — `LoadMvrStructureEvent` triggered by UI, observed by render plugin. Temp-file extraction avoids rewriting byte-based loaders.

---

### 6.6 Test Corpus & Fixture Validation ✅

**Goal:** Build confidence in GDTF parser robustness.

**Two-layer testing strategy:**

**Layer 1 — Synthetic unit tests (guaranteed CI coverage):**
- ✅ 10 unit tests in `stagelx-gdtf/src/gdtf.rs` covering:
  - Minimal valid GDTF (single mode, single channel)
  - Missing `description.xml` (expects `Err`)
  - Empty DMX modes
  - Multiple modes (3 modes, varying channel counts)
  - Nested geometry tree with beam (Body → Yoke → Head)
  - Wheels and slots (color wheel + gobo wheel)
  - Channel attributes (Dimmer, Pan, Tilt, Zoom, RGB)
  - Malformed XML (handled gracefully)
  - `parse_offset` edge cases
  - `parse_default_value` edge cases
- Tests use `zip::ZipWriter` over `Cursor<Vec<u8>>` to create synthetic GDTF ZIPs in memory

**Layer 2 — Optional corpus integration test:**
- ✅ `crates/stagelx-gdtf/tests/corpus.rs` scans `tests/fixture_corpus/*.gdtf`
- Parses each file, asserts `Ok` and `≥1 DMX mode`
- Prints structured per-file report (mode count, channel count)
- Skips gracefully when directory is empty or missing
- `tests/fixture_corpus/` created with `.gitkeep` and `README.md`
- `.gitignore` excludes `*.gdtf` and `*.zip` from corpus directory

**Note:** GDTF Share API requires authentication, so corpus files must be downloaded manually. The README documents the curl-based download workflow.

---

### 6.7 Performance Profiling & Optimisation ✅

**Targets (from Phase 5 success metrics):**

| Domain | Target | Verification |
|---|---|---|
| Beam GPU pass | ≤ 6 ms at 500 fixtures, 1080p | wgpu timestamp queries — **deferred** (needs custom render node) |
| Framebuffer overdraw | ≤ 8× averaged | wgpu timestamp queries — **deferred** |
| DMX tick jitter | ≤ 1 ms std-dev over 10k ticks | `PerfDiagnosticsRes` Welford online algorithm ✅ |
| IO snapshot staleness | 0 occurrences > 100 ms | `last_tx_at` / `last_rx_at` on all `*Stats` ✅ |
| Patch load frame time | ≤ 20 ms main thread throughout | `bevy_diagnostic` + per-system timing ✅ |
| Patch load total | < 2 s wall-clock for 500 fixtures / 50 models | `benches/spawn_500.rs` ✅ (~11 ms for 500 fixtures) |
| GPU geometry memory | ≤ 128 MB for 500-fixture rig | `estimate_gpu_memory` — geometry buffers only ✅ |

**Completed:**
- ✅ `FrameTimeDiagnosticsPlugin::default()` registered in `src/main.rs`
- ✅ `PerfDiagnosticsRes` extended with per-system CPU timing fields:
  - `beam_articulate_ms`, `beam_lod_eval_ms`, `beam_lod_apply_ms`, `beam_sort_ms`
- ✅ Timed wrapper systems in `stagelx-render` for `articulate_beams`, `evaluate_beam_lod`, `apply_beam_lod`, `sort_beams_front_to_back`
- ✅ `cue_to_dmx.before(dmx_engine_tick)` ordering fix in `src/main.rs`
- ✅ IO staleness timestamps: `last_tx_at` / `last_rx_at: Option<Instant>` on `ArtNetStats`, `SacnStats`, `UsbStats`, `MidiStats`, `OscStats`
- ✅ All protocol send/receive systems update timestamps on actual packet activity
- ✅ `benches/spawn_500.rs` — synthetic benchmark: 500 fixtures in ~11 ms (~45K fixtures/sec)
- ✅ `benches/flood_artnet.rs` — synthetic benchmark: 100K packet construction + UDP send

**Deferred to post-Phase 6:**
- wgpu timestamp queries for beam GPU pass timing (requires custom render node around half-res camera pass)
- `docs/profiling.md` documentation

---

### 6.8 Cleanup & Tech Debt ✅

- [x] Remove unused `stagelx-render` dependency from `stagelx-ui/Cargo.toml`
- [x] Verify all `#[allow(dead_code)]` have associated tickets or are removed (none found)
- [x] Run `cargo clippy --workspace -- -D warnings` and fix all lints
- [x] Run `cargo audit` and document any accepted advisories (`paste` / RUSTSEC-2024-0436)
- [x] Update `README.md` with current feature set and build instructions
- [x] Update `CHANGELOG.md` for v0.1.0 release

---

### Phase 6 Milestone

> Load a 200-fixture MVR from a real show file. Control beams via MIDI surface. Record 5 cues. Play back cue stack with cross-fade. Output Art-Net and sACN simultaneously. Zero crashes over a 4-hour session. CPU < 20 %, GPU beam pass < 6 ms.

---

## Open Questions / Decisions Needed

1. **GDTF 3D model format:** GDTF v1.1 uses 3DS format for geometry. `ds3` handles this. Future GDTF versions may use glTF — monitor DIN SPEC 15800 revisions.

2. **Bevy version:** 0.18.0. Evaluate 0.19 after Phase 6 (Bevy release cycle ~4 months). Do not upgrade mid-Phase 6.

3. **Shader approach for beams:** Resolved — custom `BeamMaterial` via Bevy's `Material` trait + WGSL shader. ✅

4. **Cue system:** Phase 6 foundation only. Full cue stack (effects, chases) is v2.

5. **GDTF-share API:** gdtf-share.com has a REST API for downloading fixture files by manufacturer/model. Worth integrating a fixture browser that can pull directly from the share. **Decision:** Defer to v2 — requires API key and caching strategy.

6. **Art-Net node discovery:** ArtPoll/ArtPollReply in Phase 6.

7. **sACN multicast join:** `IP_ADD_MEMBERSHIP` in Phase 6.

8. **Show file format:** MVR has no cue concept. Use a sidecar `.json` for cue stacks, or extend MVR unofficially? **Decision:** Sidecar `.json` for v1. Official MVR extension proposal for v2.

---

## Non-Goals (v1)

- Full cue stack / show playback (Phase 6 lays foundation; v2 completes)
- Video / media server integration
- Path-traced / offline rendering
- Mobile or web targets
- Network multi-user collaboration
- Fixture builder / GDTF authoring

---

## Repository Setup

```bash
git init
git remote add origin https://github.com/BlueJayLouche/stageLX.git
cargo init --name stageLX
# then convert to workspace and add crates/
```

Suggested `.gitignore`: standard Rust gitignore + `*.gdtf` test files (large binaries) + `tests/fixture_corpus/`.

---

## Changelog

### 2026-05-11 — UI layout refactor
- **Panel layout restructure:** DMX I/O moved from inline bottom strip to dedicated `SidePanel::right` (420 px). Patch + Library promoted from inline `CentralPanel` scope to `TopBottomPanel::bottom` (248 px). CentralPanel now contains viewports only.
- **System merge:** `io_panel_system` merged into `ui_root_system` via new `IoParams` `SystemParam` (keeps parameter count under Bevy's limit).
- **Detached window fixes:** Prevented infinite expansion via `search_input` margin fix, `ScrollArea` inner-width usage, toolbar chip sizing, and `max_height` caps on programmer/cue/IO node lists. All detached windows receive `.default_height()` and `.min_width(240.0)`.

### 2026-05-10 — Phase 6.1–6.7 complete
- **6.1 Crate split:** `stagelx-state` → `stagelx-patch` + `stagelx-show`. 8 crates in workspace.
- **6.2 IO formalisation:** All transports implement `IoSource`/`IoSink`. `socket2` tuning. Overflow warnings.
- **6.3 Protocol completeness:** ArtPoll/ArtPollReply node discovery. sACN multicast join. UI integration.
- **6.4 Cue foundation:** `CueStack`, `Cue`, `CuePlayhead`, `cue_to_dmx()`. Cue panel UI. JSON persistence to `show.json`.
- **6.5 MVR structure:** `Truss` / `SceneObject` parsing. Embedded geometry extraction. `LoadMvrStructureEvent`. Per-object transforms.
- **6.6 Test corpus:** 10 synthetic GDTF parser unit tests. Corpus integration test (`tests/corpus.rs`). 13 tests pass in CI.
- **6.7 Performance profiling:** `FrameTimeDiagnosticsPlugin`. Per-system CPU timing in `PerfDiagnosticsRes`. IO staleness timestamps. `benches/spawn_500.rs` + `benches/flood_artnet.rs`.
- **Bugfix:** sACN/Art-Net RX `EAGAIN` (os error 35) after start/stop — `WouldBlock` + `Interrupted` now treated as transient alongside `TimedOut`.
- Bevy 0.18 observer pattern used throughout (`On<Event>`, `commands.trigger()`).

### 2026-05-08 — v0.2.0-phase6 plan drafted
- Restructured PLAN.md with Table of Contents, ADRs, Phase Archive, and Phase 6
- Added Cue System Architecture section
- Updated dependency graph to reflect post-audit crate structure
- Archived Phases 1–5; detailed audit findings moved to git history
- Added success metrics table and profiling plan

### 2026-05-08 — Phase 5 mid-point audit complete
- 28 binding rules established (ADR-001 through ADR-012)
- 56 UI audit items resolved
- All P1/P2/P3 frontend fixes done
- Zero compiler errors, zero warnings

### 2026-05-05 — Phase 5 rendering fixes
- FOH beam blackness (shader cone-angle mismatch)
- Top/side ortho view beam visibility (perpendicular sprite cross)
- Async file dialogs (background thread + channel)
- Top-bar protocol pills wired to live I/O stats

---

## Phase 7 — Show Playback & Stage Capture

**Goal:** Transform the Phase 6 cue foundation into a show-ready playback system. Cross-fade interpolation, per-fixture capture from live DMX output, complete attribute coverage, external triggering, and a unified show file.

**Timeline:** Weeks 27–32

---

### 7.1 Fade Engine ✅

**Problem:** `cue_to_dmx()` writes cue snapshots instantly. `fade_in_ms`/`fade_out_ms` exist on `Cue` but are never read. The `CuePlayhead` tracks only `current_cue_index: Option<usize>` — no fade state machine.

**Implementation:**

1. Extend `CuePlayhead` with a fade state machine:
   ```rust
   pub enum PlayheadState {
       Idle,
       Fading {
           start: Instant,
           duration_ms: u32,
           from: HashMap<FixtureId, CueValues>,
           to: HashMap<FixtureId, CueValues>,
       },
   }
   ```
2. `on_go_cue` observer: if `fade_in_ms > 0`, enter `Fading` state; otherwise snap instantly.
3. `on_back_cue` observer: cross-fade from current cue back to previous (or to black if at cue 0).
4. `cue_to_dmx()` system: when in `Fading`, interpolate dimmer/pan/tilt/zoom/color per fixture using `start.elapsed()` and `duration_ms`. Write interpolated values to DMX engine source `"cue_playback"`.
5. Interpolation rules:
   - Dimmer: linear float lerp, then byte-round
   - Pan/Tilt/Zoom: linear lerp
   - Color (RGB): per-channel linear lerp
   - Strobe: hold `from` value until `progress >= 1.0`, then snap to `to`
6. UI: fade progress bar on active cue row with percentage readout.
7. Unit tests: `CueValues::lerp` midpoint, clamping, strobe snap; `CuePlayhead` go/back/snap.

**Files changed:** `stagelx-show/src/cue.rs`, `stagelx-dmx/src/cue_playback.rs`, `stagelx-ui/src/cue.rs`

**Success metric:** GO between two cues with 2-second fades produces smooth visible transition in 3D viewport and on DMX output. No frame drops.

---

### 7.2 Per-Fixture Stage Capture ✅

**Problem:** `Programmer` is global. `record_from_programmer()` applies the same `CueValues` to every fixture in the patch. A lighting tech cannot create a cue where Fixture 1 is at 50% and Fixture 2 is at 75%.

**Implementation:**

Instead of rebuilding the programmer architecture, added a **"Record Stage"** mode that captures the current merged DMX output per fixture:

1. Added `CaptureMode` enum (`Programmer` / `Stage`) as a Bevy Resource in `stagelx-show`.
2. Added `RecordStageCueEvent` event type.
3. `stagelx-dmx::stage_capture::on_record_stage_cue` observer reads `DmxEngineRes::output_buffer()` per fixture universe, inverse-projects channel values back to `CueValues` using `DmxChannelMap`:
   - Dimmer: `byte / 255.0`
   - Pan/Tilt: `(msb << 8 | fine) / 65535.0`
   - RGB: `byte / 255.0`
4. Stage-captured cues get default 2-second fade in/out for immediate playback testing.
5. UI toggle: PGM / STAGE buttons above the cue toolbar; RECORD button label changes to "● STAGE" in stage mode.
6. Unit tests: round-trip 8-bit, 16-bit pan/tilt, dimmer full/zero, color channel inverse projection.

**Files changed:** `stagelx-show/src/cue.rs`, `stagelx-dmx/src/stage_capture.rs` (new), `stagelx-dmx/src/lib.rs`, `stagelx-ui/src/cue.rs`, `stagelx-ui/src/lib.rs`, `src/main.rs`

**Success metric:** Patch two fixtures at different addresses. Set one to 50% dimmer via MIDI CC, the other to 75% via Art-Net input. Record Stage → cue contains different values per fixture. Playback restores both correctly.

---

### 7.3 Complete DMX Attribute Coverage ✅

**Problem:** `CueValues` has no gobo fields. `DmxChannelMap` has no gobo/color-wheel offsets. `cue_to_dmx()` and `programmer_to_dmx()` do not output gobo or color-wheel channels. Fixtures render gobos in 3D but never transmit them over DMX.

**Implementation:**

1. Extended `CueValues` with `gobo_index: u8` and `gobo_spin: f32`.
2. Extended `DmxChannelMap` with `gobo: Option<u16>`, `gobo_rotation: Option<u16>`, `color_wheel: Option<u16>`.
3. Updated GDTF parser (`stagelx-gdtf::channel_map`) to map `Gobo`, `GoboRot`, and `ColorWheel` attributes to channel offsets.
4. Updated `programmer_to_dmx()` and `cue_to_dmx()` to write gobo/gobo_rotation bytes when present in channel map:
   - `gobo_byte = (gobo_index * 32.0).clamp(0, 255)` — supports 8 discrete gobo slots
   - `gobo_spin_byte = (gobo_spin * 255.0).clamp(0, 255)`
5. Updated `CueValues::from_programmer()` to include gobo fields.
6. Updated `CueValues::lerp()` with gobo snap + gobo_spin linear interpolation.
7. Updated `stage_capture.rs` to inverse-project gobo channels:
   - `gobo_index = (byte / 32.0) as u8`
   - `gobo_spin = byte / 255.0`

**Files changed:** `stagelx-core/src/fixture.rs`, `stagelx-gdtf/src/gdtf.rs`, `stagelx-show/src/cue.rs`, `stagelx-dmx/src/projection.rs`, `stagelx-dmx/src/cue_playback.rs`, `stagelx-dmx/src/stage_capture.rs`

**Success metric:** Programmer gobo selection is visible on both 3D beams and DMX output (verified via Art-Net RX loopback or USB capture).

---

### 7.4 Cue Editing Workflow ✅

**Problem:** Cue panel had `// TODO: load cue into programmer for editing`. No "Update" button. Record-only workflow. No way to edit fade times from the UI.

**Implementation:**

1. **Load into programmer:** Click a cue row → emits `LoadCueIntoProgrammerEvent(i)` → observer reads the cue's snapshot and copies the first fixture's values into `Programmer` (pan, tilt, dimmer, zoom, strobe, color, gobo_index, gobo_spin).
2. **Update cue:** Added "UPDATE" button to cue panel toolbar (4-button layout: BACK | GO | REC | UPDATE) → emits `UpdateCueEvent` → observer overwrites the active cue's snapshot with current programmer values.
3. **Fade time editor:** Detail panel below the cue list shows `DragValue` editors for fade in / fade out (0–60s, 0.1s step). Updates are persisted to `show.json` on change.
4. **Cue label editing:** Text field in the detail panel edits the active cue's label. Changes persisted on edit.

**Files changed:** `stagelx-show/src/cue.rs`, `stagelx-ui/src/cue.rs`, `stagelx-ui/src/lib.rs`, `src/main.rs`

**Success metric:** User can: record cue 1, click cue 1 to load it, tweak dimmer, press UPDATE, GO — updated values playback correctly.

---

### 7.5 MIDI / OSC Cue Triggers ✅

**Problem:** MIDI only handled CC→programmer attribute mapping. No Note On→GO/Back. OSC had no `/cue/go` handler.

**Implementation:**

1. **MIDI triggers:**
   - Added `note_go: Option<u8>` and `note_back: Option<u8>` to `MidiConfig` (default `None`).
   - `midi_receive()`: on `0x90` Note On with velocity > 0, checks if note matches `note_go` or `note_back` and triggers `GoCueEvent` / `BackCueEvent` via `Commands`.
   - UI: two note-trigger rows with enable checkbox + `DragValue` (0–127) in the MIDI IO panel.

2. **OSC triggers:**
   - `osc_receive()`: added `/cue/go` and `/cue/back` path handlers that trigger `GoCueEvent` / `BackCueEvent`.
   - Updated OSC address pattern documentation in IO panel to show cue trigger paths.

**Files changed:** `stagelx-io/src/config.rs`, `stagelx-io/src/midi.rs`, `stagelx-io/src/osc.rs`, `stagelx-ui/src/io_panel.rs`

**Success metric:** Pressing a MIDI drum pad triggers GO. Sending `/cue/go` from TouchOSC triggers GO.

---

### 7.6 Unified Show File & UI Stubs ✅

**Problem:** `show.json` = cues only. Patch and venue were separate files. Status bar showed hardcoded stubs (BPM 128.0, FPS, CPU%).

**Implementation:**

1. **Unified show file format** (`.slx`):
   - `ShowFile` struct in `stagelx-show::show_file` bundles `patch: Patch`, `cue_stack: CueStack`, `venue_path: Option<String>`, and `name`.
   - `ShowFile::save()` / `ShowFile::load()` with version field (v1).
   - Backward-compat: `ShowFile::load()` falls back to legacy `show.json` (cue-only) and auto-migrates.
   - `SaveShowEvent` / `LoadShowEvent` events with `on_save_show` and `on_load_show` observers.
   - Auto-load on startup via `auto_load_show_on_startup`.
   - All cue observers (`on_record_cue`, `on_delete_cue`, `on_update_cue`, UI detail editor) now trigger `SaveShowEvent` instead of `stack.save_to_file("show.json")`.

2. **Keyboard shortcuts:**
   - `Ctrl+S` → Save Show
   - `Ctrl+O` → Open Show (async `rfd::FileDialog`, `.slx` filter)

3. **Status bar wired:**
   - FPS: `1000.0 / perf.frame_time_ms`
   - CPU%: sum of `beam_articulate_ms + beam_lod_eval_ms + beam_lod_apply_ms + beam_sort_ms + dmx_tick_last_ms` / 16.667ms budget
   - Universe usage: highest channel address per universe (U1, U2)
   - Venue name: filename from `VenueLoadState.import_path`
   - Removed hardcoded BPM stub

**Files changed:** `stagelx-core/src/patch.rs`, `stagelx-show/src/show_file.rs` (new), `stagelx-show/src/lib.rs`, `stagelx-show/src/cue.rs`, `stagelx-ui/src/lib.rs`, `src/main.rs`

**Success metric:** User can File → Save Show → quit → File → Open Show → patch, cues, and venue are all restored.

---

### Phase 7 Milestone

> Program a 10-cue show with per-fixture looks captured from MIDI control surface. Cross-fade between cues with 2-second fades. Trigger GO from a MIDI drum pad. Save and reload the complete show as a single file. Zero crashes over a 2-hour rehearsal. CPU < 25%, GPU beam pass < 6 ms.

---

*End of Plan*
