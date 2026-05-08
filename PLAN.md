# stageLX — Project Plan

A real-time 3D stage lighting visualizer and DMX controller written in Rust.
Supports GDTF fixture definitions, MVR scene files, and full DMX I/O.

---

## Goals

- Visualize medium-scale rigs (50–500 fixtures) at 60 fps on desktop hardware
- Parse GDTF fixture files from [gdtf-share.com](https://gdtf-share.com/) for accurate fixture capabilities and geometry
- Import and export MVR (My Virtual Rig) scenes for interoperability with MA3, Depence2, etc.
- Built-in programmer for standalone use, plus live DMX input from external consoles
- Full DMX output via Art-Net, sACN, and USB dongles (Enttec / DMXking)
- Accept control input from Art-Net/sACN, MIDI, and OSC

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
│   ├── stagelx-gdtf/          # GDTF + MVR parser
│   ├── stagelx-dmx/           # DMX frame engine + HTP/LTP merge
│   ├── stagelx-state/         # shared Bevy Resources (Programmer, PatchRes, IoConfig)
│   ├── stagelx-io/            # Art-Net, sACN, USB, MIDI, OSC
│   ├── stagelx-render/        # Bevy plugin: 3D scene, beams, gobos, fog
│   └── stagelx-ui/            # egui panels: patch, programmer, scene, I/O
│
└── src/
    └── main.rs                # Bevy App wiring
```

### Crate Responsibilities

| Crate | Responsibility |
|---|---|
| `stagelx-core` | `FixtureInstance`, `Patch`, `Universe`, `DmxBuffer`, attribute model |
| `stagelx-gdtf` | Parse `.gdtf` (ZIP+XML), geometry trees, DMX modes, wheels, physicals |
| `stagelx-dmx` | DMX frame generation, merge strategies (HTP/LTP), `DmxEngine` |
| `stagelx-state` | Shared Bevy `Resource`s: `Programmer`, `PatchRes`, `FixtureLibraryRes`, `IoConfig` |
| `stagelx-io` | Art-Net Tx/Rx, sACN Tx/Rx, USB serial (Enttec), MIDI, OSC |
| `stagelx-render` | Bevy plugin: volumetric beams, gobo projection, color, fog medium |
| `stagelx-ui` | egui: patch editor, programmer, fixture library, DMX I/O panel |

### Dependency Graph

```
stagelx-core ──────────────────────────────────────────┐
stagelx-gdtf (→ core) ─────────────────────────────────┤
stagelx-dmx  (→ core) ─────────────────────────────────┤
                                                        ▼
stagelx-state (→ core, gdtf) ──► stagelx-io (→ state, dmx)
                              ──► stagelx-render (→ state)
                              ──► stagelx-ui    (→ state)
```

All feature crates (io, render, ui) are leaf nodes — none depend on each other.

---

## Key Crate Dependencies

| Purpose | Crate |
|---|---|
| App/ECS/rendering | `bevy` 0.18.1 |
| Low-level GPU | `wgpu` (via Bevy) |
| UI panels | `bevy_egui` 0.39.1 + `egui` |
| GDTF/MVR ZIP parsing | `zip`, `quick-xml` |
| 3D model loading (GDTF geometry) | `bevy_gltf` or `obj` |
| Art-Net | custom UDP (no external crate) |
| sACN (E1.31) | custom UDP per ANSI E1.31-2016 (no external crate) |
| I/O thread bridge | `crossbeam-channel` (bounded 256, no tokio) |
| USB/serial DMX | `serialport` |
| MIDI | `midir` |
| OSC | `rosc` |
| Image loading (gobos) | `image` (via Bevy asset loader) |

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

### Gobos
- Load gobo images from GDTF wheel slot assets
- Project as texture via **projected spotlight** in Bevy PBR + custom shader
- Rotate with GoboIndex + GoboRotation attributes

### Fixture Geometry
- Load GDTF geometry from embedded 3DS/GLB/OBJ → convert to Bevy mesh
- Articulate geometry nodes: yoke rotates with Pan, head rotates with Tilt
- Fallback: procedural geometry (box for body, cylinder for head) when no model

### Color
- Additive CMY/RGB mixing in shader
- CIE color space for mixed colors when GDTF emitter data available
- Gel/filter colors from ColorWheel slot CIE values

---

## DMX Engine

- **Universe buffer**: `[u8; 512]` per universe, up to 64 universes
- **Merge**: HTP (Highest Takes Precedence) across input sources by default; configurable per universe
- **Priority**: Built-in programmer > Art-Net in > sACN in (configurable)
- **Output tick**: 44 Hz (standard DMX refresh), driven by Bevy fixed timestep
- **Attribute → DMX**: GDTF channel functions map raw DMX values to physical attribute values bidirectionally

---

## I/O Protocol Details

### Art-Net (Output + Input) ✅ Phase 3
- UDP port 6454, custom ArtDMX implementation (no external crate)
- ArtDMX packet TX at 44 Hz via Bevy `FixedUpdate`
- RX via blocking socket cloned from TX socket (`try_clone` avoids EADDRINUSE)
- RX thread bridges to Bevy via `crossbeam-channel` (bounded 256)
- Source IP allowlist (cached, rebuilt only on config change)
- Universe cap: 64 universes per source (amplification attack mitigation)
- Configurable TX destination (default: limited broadcast 255.255.255.255)
- ArtPoll/ArtPollReply for node discovery (future)

### sACN / E1.31 (Output + Input) ✅ Phase 3
- UDP port 5568, full ANSI E1.31-2016 Data Packet (638 bytes)
- TX: rolling sequence counter, multicast 239.255.hi.lo or configurable unicast
- RX: same clone-and-block pattern as Art-Net
- Priority 100 default, configurable; universe cap 64
- ArtPoll/ArtPollReply for node discovery (future)

### USB DMX (Output)
- Enttec DMX USB Pro protocol over serial (`serialport` crate)
- Single universe per device
- Auto-detect by USB VID/PID

### MIDI (Input)
- `midir` for cross-platform MIDI device access
- Configurable mapping: CC → attribute, Note → cue trigger
- Run in `midir` callback, forwarded to Bevy event queue

### OSC (Input)
- UDP, default port 8000
- Path schema: `/fixture/{id}/{attribute}` → float value
- `/cue/{id}/go` for cue triggers

---

## Implementation Phases

### Phase 1 — Foundation ✅ Complete
**Goal**: A Bevy app that loads a GDTF file and renders placeholder fixtures in 3D.

- [x] Cargo workspace scaffold
- [x] `stagelx-gdtf`: parse GDTF ZIP + `description.xml` (fixture type, DMX modes, attributes)
- [x] `stagelx-core`: `FixtureType`, `FixtureInstance`, `Patch`, `Universe`, `DmxBuffer`
- [x] `stagelx-render`: Bevy plugin — procedural fixture geometry, static scene camera
- [x] Basic egui panels: fixture library, patch editor, programmer
- [x] `stagelx-dmx`: `DmxEngine` with HTP/LTP merge, multi-source priority stack

**Milestone**: Patch 10 moving heads, manually set Pan/Tilt/Dimmer values, see fixtures move in 3D. ✅

---

### Phase 2 — Programmer + Beam Rendering ✅ Complete
**Goal**: Real-time beam visualisation driven by the built-in programmer.

- [x] Programmer UI: dimmer, pan/tilt, RGB color, zoom, strobe, gobo index + spin
- [x] Beam cone mesh with additive blending (`BeamMaterial` — custom WGSL shader)
- [x] Color mixing (RGB) in shader, driven live from Programmer resource
- [x] Gobo projection: rotating texture sampled in the beam material
- [x] Zoom → XZ scale on beam cone geometry
- [x] Strobe simulation via time-based shutter open/close
- [x] Dimmer → beam intensity and point-light intensity
- [x] Keyboard programmer: arrow keys (pan/tilt), +/- (dimmer), R/G/B/W/X/C (color), Z (zoom)

**Milestone**: Full real-time beam visualization driven by programmer at 60 fps with 10 fixtures. ✅

---

### Phase 3 — DMX I/O ✅ Complete
**Goal**: Send and receive DMX from real hardware and consoles.

- [x] `stagelx-state`: extract shared Bevy Resources into dependency-inversion hub
- [x] `stagelx-io`: Art-Net output (ArtDMX packets, 44 Hz `FixedUpdate`)
- [x] Art-Net input listener (blocking RX thread, `crossbeam-channel` bridge)
- [x] sACN E1.31 output (638-byte Data Packet, rolling sequence, multicast/unicast)
- [x] sACN input (separate port 5568 socket, same clone-and-block pattern)
- [x] HTP merge across input sources via `DmxEngine` priority stack
- [x] Universe/port configuration UI (DMX I/O egui panel)
- [x] Security: source IP allowlist, universe cap (64), configurable TX destinations
- [ ] USB DMX output (Enttec USB Pro) — deferred to Phase 4

**Milestone**: Receive Art-Net or sACN from a console, visualize result, simultaneously output both protocols. ✅

---

### Phase 4 — MVR + GDTF Geometry + USB DMX ✅ Complete
**Goal**: Import a full show file from MA3 / Depence2 and see the venue + patch.

- [x] USB DMX output (Enttec USB Pro protocol over `serialport`, NonSend resource, 44 Hz)
- [x] `stagelx-gdtf`: MVR parser (scene positions, fixture placement, embedded GDTFs)
- [x] Import MVR: place fixtures in 3D from MVR transforms (Z-up mm → Y-up m)
- [x] `stagelx-3ds`: pure-Rust 3DS binary chunk parser with `to_bevy_buffers()` helper
- [x] Observer-based fixture lifecycle (`On<SpawnFixtureEvent>` / `On<DespawnFixtureEvent>`)
- [x] GDTF-driven DMX channel mapping (pan/tilt/dimmer/colour from channel offsets)
- [x] Patch add-fixture UI (GDTF type + mode selector, universe/channel form)
- [ ] Wire 3DS geometry → actual Bevy mesh in renderer (parser done, render hookup deferred)
- [ ] MVR export (scene + patch → `.mvr` file) — deferred to Phase 5
- [ ] Truss / structure geometry from MVR — deferred to Phase 5

**Milestone**: MVR import places fixtures from real show files; USB DMX output to Enttec dongles. ✅

---

## Architecture & Performance Decisions (Pre–Phase 5 Findings)

Output of a structured Performance vs Architect role-debate. These are binding design rules for Phase 5 and beyond.

---

### Immediate — Before Phase 5 Feature Work

#### 1. ✅ Freeze `stagelx-state`
- **Rule:** No new Bevy Resources added to `stagelx-state` in Phase 5.
- **Rationale:** The crate has become a dependency sink (io + render + ui all import it). Adding MIDI config, viewport state, and export staging here would cement it as a god-crate.
- **Routing for new state:**
  - MIDI/OSC config → `stagelx-io`
  - Viewport layout → `stagelx-render` (`ViewportLayout` Resource)
  - MVR export staging → `stagelx-export` (new module/crate)
- **Phase 6:** Mechanical extraction of `stagelx-state` → `stagelx-show` (Programmer, future cue stacks) + `stagelx-patch` (PatchRes, Universe routing). Code will already live in the right places; this becomes a Cargo.toml reorganisation.
- **Done:** Freeze doc comment added to `stagelx-state/src/lib.rs` (commit `fbb7aa5`).

#### 2. ✅ Replace `stagelx-3ds` with `ds3`; move Bevy adapter to `stagelx-render`
- **Rule:** Format crates must be runtime-agnostic. They expose `Vec<[f32; 3]>`, index buffers, and materials-as-data — no Bevy types.
- **Done (commit `fbb7aa5`):**
  - Vendored `stagelx-3ds` crate deleted; replaced with `ds3` path dep (`../3ds-rs`). The standalone crate adds smooth normals, transform matrix, material names, smooth groups, full test suite, serde, and no_std support.
  - `mesh_from_gdtf` moved to `stagelx-render::adapters::three_ds` — the only Bevy-aware geometry adapter.
  - `stagelx-render` now depends on `ds3` (runtime-agnostic) rather than the old vendored crate.

#### 3. ✅ Formalise the IO thread abstraction
- **Rule:** All IO transports share a common `IoSource` / `IoSink` contract. No per-transport bespoke thread management.
- **Done (commit `fbb7aa5`):**
  - `IoSource`, `IoSink` traits and `IoSupervisor` Resource defined in `stagelx-io::supervisor`.
  - Art-Net and sACN RX channel depth reduced from 256 → 8 slots.
  - MIDI and OSC must implement `IoSource` when wired in Phase 5; existing transports migrate in Phase 6.
- **Deferred:** `SO_RCVBUF` = 4 MB on UDP sockets (requires `socket2` crate — add when MIDI/OSC land).

---

### Phase 5 Implementation Rules

#### 4. MIDI & OSC input
- Implement `IoSource` trait — MIDI callback → crossbeam → Bevy Messages, OSC UDP → crossbeam → Bevy Messages.
- Config Resources (`MidiConfig`, `OscConfig`) live in `stagelx-io`, not `stagelx-state`.
- MIDI/OSC config UI panels source their data from `stagelx-io` Resources.

#### 5. ✅ Split-screen viewports
- Implement as `render::viewports` module inside `stagelx-render`.
- `ViewportLayout` Resource in `stagelx-render` (not `stagelx-state`).
- Do NOT extract to a `stagelx-viewport` crate yet — promote only if `stagelx-ui` needs to drive layout independently.
- Layout: FOH perspective (¾ width) + top ortho + side ortho.
- **Done (commit `2dc1d79`):** `FohCamera`, `TopCamera`, `SideCamera` components; `update_viewports_on_resize` system; egui separator lines + TOP/SIDE labels.

#### 6. MVR export
- `stagelx-mvr`: new format-pure crate (XML document model, no Bevy types).
- `stagelx-export`: orchestration module/crate that depends on `stagelx-mvr` + domain crates. This is where the domain→format projection lives.
- Does not depend on `stagelx-state`.

#### 7. Volumetric beam rendering — three-tier LOD
- Implement in `stagelx-render::lod` module. Not a new crate.
- LOD tiers evaluated per-frame by screen-space projected radius:
  - **Tier 0** (<50 px): billboard sprite — zero ray-march cost.
  - **Tier 1** (50–200 px): half-res offscreen (960×540), 16 ray-march steps, bilinear upsample + composite.
  - **Tier 2** (>200 px): full-res, hard cap 32 steps. 64+ steps reserved for single selected "hero" fixture only.
- **Hard cap:** 64 simultaneous ray-marched beams regardless of patch size. Remaining fixtures render Tier 0 until screen coverage qualifies them.
- **Lighting:** Enable `ClusteredForward` explicitly in `stagelx-render` pipeline. Reduces 500 point lights to ~4–8 per fragment per tile.
- **Blending:** Sort beam cones front-to-back within additive tier (preserves early-z on opaque venue geometry).
- **Target:** Beam pass ≤ 6 ms GPU at 500 fixtures, 1080p worst-case camera. Overdraw ≤ 8× framebuffer-averaged (wgpu timestamp queries).

#### 8. GDTF geometry loading — async + streaming
- Drive 3DS parsing + Mesh construction through Bevy's async `AssetLoader` (task pool, not render thread).
- **Deduplication:** Cache `Handle<Mesh>` by `(gdtf_uuid, sub_mesh_index)`. 500 fixtures across 50 unique models = 30–50 uploads, not 500.
- **Streaming:** Process ≤ 10 unique models per frame during patch load. 50 models = 5 frames ≈ 83 ms — invisible progressive geometry appearance.
- **Polygon budget:** Reject/simplify any 3DS sub-mesh >8k triangles via `meshopt_simplify` at import time.
- **Targets:** Main thread never exceeds 20 ms during patch load. Total load <2s wall-clock for 500 fixtures. GPU geometry memory ≤ 128 MB total.

#### 9. DMX tick — non-blocking IO contract
- `FixedUpdate` DMX tick uses `try_recv()` exclusively — never blocks waiting for channel data.
- On empty channel: hold last known universe snapshot (correct DMX hold-last behaviour).
- Emit `IoOverflowWarning` Bevy event if channel found full on 3+ consecutive `try_send()` calls (surface in IO panel UI).

---

### ECS Lifecycle Rule

- **Observers** (`On<SpawnFixtureEvent>`, `On<DespawnFixtureEvent>`): use for one-shot lifecycle work (spawn child entities, set initial transforms, attach geometry).
- **ECS query systems with explicit `SystemSet` ordering**: use for steady-state per-frame updates (`DmxIngest → DmxMerge → FixtureApply → Render`). Do not use observers for per-frame attribute updates.

---

### Success Metrics

| Domain | Target |
|--------|--------|
| Beam GPU pass | ≤ 6 ms at 500 fixtures, 1080p worst-case |
| Framebuffer overdraw | ≤ 8× averaged (wgpu timestamp queries) |
| DMX tick jitter | ≤ 1 ms std-dev over 10k ticks |
| IO snapshot staleness | 0 occurrences > 100 ms under 10× Art-Net flood |
| Patch load frame time | ≤ 20 ms main thread throughout |
| Patch load total | < 2 s wall-clock for 500 fixtures / 50 models |
| GPU geometry memory | ≤ 128 MB for 500-fixture rig |
| `stagelx-state` growth | 0 new Resources added in Phase 5 |

---

## Phase 5 Mid-Point Audit — 2026-05-08

Output of a parallel Performance + Architect multi-role analysis of the Phase 5 codebase. Violations of previously established rules are flagged. New binding rules (10–22) are added below and supersede any conflicting previous notes.

### Existing Rule Violations

| Rule | Status | Location |
|---|---|---|
| Rule 4: `MidiConfig`/`OscConfig` live in `stagelx-io` | **VIOLATED** — both still in `stagelx-state::IoConfig` | `state/src/lib.rs:149–171` |
| Dependency graph: leaf crates don't depend on each other | **VIOLATED** — `stagelx-ui` imports `stagelx-render::VenueLoadState` | `ui/src/lib.rs:17,108` |
| Rule 3: `IoSource`/`IoSink` contract; no bespoke thread management | **PARTIAL** — traits defined but no transport implements them; all transports still use ad-hoc `thread::spawn` | `io/src/supervisor.rs` |
| Rule 9: Emit `IoOverflowWarning` when channel found full | **NOT DONE** — event type missing; `IoSupervisor::rx_drops` never incremented | `io/src/supervisor.rs` |

### New Bindings (Rules 10–22)

#### 10. IO RX threads must use `try_send`, not `send`
- **Rule:** IO transport RX threads must use `try_send()` on the crossbeam channel. Blocking `send()` stalls the OS receive buffer under load, causing network-level packet loss invisible to the application.
- **Action:** `artnet.rs:147`, `sacn.rs:194` — change `tx.send(pkt)` → `tx.try_send(pkt)`. On `Err(TrySendError::Full)` increment `IoSupervisor::rx_drops` and continue.

#### 11. No `Arc<Vec<u8>>` on the UDP hot path — use `[u8; 512]`
- **Rule:** `ReceivedPacket.data` must be a fixed-size `[u8; 512]` array field. The `Arc<Vec<u8>>` pattern causes ~3,200 heap allocations/sec at high universe count with no benefit (data is consumed once and dropped).
- **Action:** `artnet.rs:141`, `sacn.rs:188`.

#### 12. All IO transport threads must have a shutdown path
- **Rule:** No transport thread may be permanently leaked when its protocol is disabled. Leaked threads hold bound OS ports, causing `EADDRINUSE` on re-enable.
- **Action:** `osc.rs:74` — wrap `UdpSocket` in `Arc`, store a clone in `OscState`, call `shutdown(Shutdown::Both)` to unblock `recv_from` on disable. Apply the same pattern to Art-Net/sACN using the `IoSupervisor` shutdown signal once Rule 3 is implemented.

#### 13. No per-frame heap allocation in `Update` or `FixedUpdate` systems
- **Rule:** ECS systems running at frame rate or the DMX tick rate must not allocate `Vec` or `HashMap` values that are discarded each tick. Use `Local<T>` to persist allocations across frames; rebuild only on structural changes.
- **Known violations:**
  - `lod.rs:209` — `Vec<(Entity, f32, BeamLodTier)>` allocated every frame → `Local<Vec<...>>`
  - `lod.rs:267` — `HashMap<FixtureId, Entity>` allocated every frame → `Local<HashMap<...>>`, rebuilt only on `Added<BeamSprite>` / `RemovedComponents<BeamSprite>`
  - `engine.rs:51` — `HashSet<u16>` + `Vec<u16>` allocated every 44 Hz tick → cache in `DmxEngine` with a dirty flag

#### 14. `articulate_beams` must gate material writes behind change detection
- **Rule:** `beam_materials.get_mut(handle.id())` marks the Bevy asset dirty unconditionally, triggering a GPU uniform upload for every fixture every frame. `Mat4::inverse()` must not run for static fixtures. Both must be gated on actual change.
- **Action:** `render/src/fixture.rs:303–316` — filter on `Changed<GlobalTransform>` and a cached last-programmed value; call `get_mut` only when a change is detected.

#### 15. MIDI port scan must be rate-limited to ≤ 1 Hz
- **Rule:** `MidiInput::new("stageLX-scan")` invokes platform MIDI subsystem APIs and must not be called in `Update` (60 Hz). Use a `Local<Timer>` or elapsed-time accumulator.
- **Action:** `io/src/midi.rs:34`.

#### 16. Release profile must enable LTO
- **Rule:** Workspace `Cargo.toml` must define `[profile.release]` with `lto = "thin"` and `codegen-units = 1`. The `merge_htp`/`merge_ltp` loops in `stagelx-core::universe` are 512-byte SIMD candidates; cross-crate inlining is required for the compiler to vectorize them.
- **Action:** Add to root `Cargo.toml`. Also add `[profile.dev.package."*"] opt-level = 2` to keep dependencies optimized during development builds.

#### 17. Remove the `tokio` workspace dependency
- **Rule:** The project correctly uses sync threads + crossbeam. `tokio` is declared in workspace deps but unused by any crate. It adds ~600 KB of compile cost and misleads readers about the runtime model. Do not add tokio.
- **Action:** Remove `tokio` from `Cargo.toml:27`.

#### 18. LOD tier transitions must use a ±10 px hysteresis band
- **Rule:** `evaluate_beam_lod` must not flip a fixture's tier on a single frame. Apply a ±10 px band around each threshold: promote when `radius > threshold + 10`, demote only when `radius < threshold - 10`. Prevents per-frame `commands.entity().insert()` churn and visible flickering at tier boundaries.
- **Action:** `lod.rs:225`. Store previous tier per entity (component or `Local<HashMap<Entity, BeamLodTier>>`).

#### 19. `BeamRenderTarget` must resize on `WindowResized`
- **Rule:** The half-res beam render target is created once at startup. It must be recreated whenever `WindowResized` fires, parallel to `update_viewports_on_resize`. Stale dimensions on HiDPI displays or after window resize produce incorrect compositing.
- **Action:** `render/src/lod.rs:84` — add a resize system listening on `EventReader<WindowResized>`.

#### 20. `programmer_to_dmx` belongs in `stagelx-dmx`, not `stagelx-io`
- **Rule:** Attribute→DMX channel projection is the DMX engine's responsibility. Its current location in `stagelx-io::artnet` is the reason `stagelx-io` has an unnecessary dep on `stagelx-gdtf`. Moving it enables pre-computation of a `DmxChannelMap` at patch load time, eliminating per-tick GDTF `&str` lookups (currently ~22,528 string comparisons/sec at 64 fixtures × 44 Hz).
- **Action:** Move `artnet.rs:228–306` → `dmx/src/projection.rs`. Add a `DmxChannelMap` cache (`[Option<u16>; 8]` per attribute) to `FixtureInstance`, computed once in `on_fixture_spawned`. Remove `stagelx-gdtf` from `stagelx-io/Cargo.toml`.

#### 21. `VenueLoadState` must move out of `stagelx-render`
- **Rule:** `VenueLoadState` is application state, not render state. `stagelx-ui` must not import from `stagelx-render` to drive venue-load logic — this is the only inter-leaf dependency in the workspace and directly violates the dependency graph invariant. UI must fire a `LoadVenueEvent` instead of querying render entities directly.
- **Action:** Move `VenueLoadState` to `stagelx-state`. Update `ui/src/lib.rs:17,108`.

#### 22. `IoConfig` must be split into per-protocol config + stats Resources
- **Rule:** `IoConfig` currently holds user intent fields and runtime telemetry counters for five protocols in one struct. Every `ResMut<IoConfig>` borrow is exclusive — all five protocol systems compete for it each frame, creating real Bevy scheduler contention. Split into:
  - Per-protocol `*Config` Resources (user intent: IPs, ports, enabled flags) — owned by UI, written rarely.
  - Per-protocol `*Stats` Resources (frame counters, status strings) — owned by the IO system, written per-frame.
  - Config Resources for MIDI and OSC must live in `stagelx-io`, not `stagelx-state` (aligns with Rule 4).
- **Action:** `state/src/lib.rs:107–219` → split across 5 new files in `io/src/`.

### Prioritized Fix List

Items 1–4 are under 45 minutes combined and must be done before any show call.

| # | Action | Files | Effort |
|---|---|---|---|
| 1 | `try_send` in Art-Net/sACN RX threads (Rule 10) | `artnet.rs:147`, `sacn.rs:194` | 30 min |
| 2 | Fix OSC socket leak on disable (Rule 12) | `osc.rs:74` | 1 h |
| 3 | Remove `tokio` workspace dep (Rule 17) | `Cargo.toml:27` | 5 min |
| 4 | Add `[profile.release]` with LTO (Rule 16) | `Cargo.toml` | 5 min |
| 5 | Split `IoConfig` into per-protocol `*Config` + `*Stats` (Rules 22 + 4) | `state/src/lib.rs:107–219`, IO crates | 1 day |
| 6 | Gate `articulate_beams` on `Changed<GlobalTransform>` (Rule 14) | `render/src/fixture.rs:303` | 2 h |
| 7 | `[u8; 512]` on `ReceivedPacket` (Rule 11) | `artnet.rs:141`, `sacn.rs:188` | 30 min |
| 8 | Move `programmer_to_dmx` to `stagelx-dmx` + `DmxChannelMap` cache (Rule 20) | `artnet.rs:228–306` → `dmx/src/projection.rs` | 4 h |
| 9 | Move `VenueLoadState` to `stagelx-state` (Rule 21) | `ui/src/lib.rs:17,108` | 1 h |
| 10 | Implement `IoSource` for Art-Net/sACN/OSC; wire `rx_drops` (Rule 3 + 10) | `supervisor.rs`, transports | 1 day |
| 11 | `Local<Vec>` + `Local<HashMap>` in LOD systems (Rule 13) | `lod.rs:209`, `lod.rs:267` | 2 h |
| 12 | Cache universe ID list in `DmxEngine` with dirty flag (Rule 13) | `engine.rs:51` | 1 h |
| 13 | LOD hysteresis ±10 px (Rule 18) | `lod.rs:225` | 1 h |
| 14 | Resize `BeamRenderTarget` on `WindowResized` (Rule 19) | `lod.rs:84` | 1 h |
| 15 | Rate-limit MIDI port scan to 1 Hz (Rule 15) | `midi.rs:34` | 15 min |

---

## Phase 5 Mid-Point Audit — Frontend UI Review — 2026-05-08

Output of a Frontend role analysis comparing the live `stagelx-ui` implementation against the `design_handoff_stagelx_ui` brief (JSX prototype + `tokens.css`). Violations and regressions are binding fixes for Phase 5 completion.

### Panel Scores vs. Design Brief

| Panel | Score | Primary blocker |
|---|---:|---|
| Layout shell + top bar | 72 | Status bar defaults off; viewport borders via `painter.line_segment` not layout primitives |
| Programmer | 68 | Fixture names always `"—"`; fader/encoder font sizes wrong; strobe Hz display bug |
| Patch | 62 | Search field double-renders; `ComboBox` id collision; live dot absent; shift-range select stub |
| DMX I/O | 58 | TX/RX counter always reads Art-Net data regardless of active protocol; MIDI CCs read-only |
| Library | 55 | Search input aliases GDTF import path (functional regression); tab active border wrong edge |

**Overall completeness: ~56 / 100. Visual token fidelity: ~85 %. Atom coverage: 8 / 11 (4 of 8 partial).**

### Priority 1 — Functional Regressions (must fix before Phase 5 milestone)

| # | Issue | Location |
|---|---|---|
| R1 | `res.import_path` used as Library search query — typing in search overwrites the GDTF load path | `library.rs:132` |
| R2 | Both Add-Fixture `ComboBox::from_label("")` share the same egui id — Type combo broken | `patch.rs:302,320` |
| R3 | TX/RX counter always reads `artnet_tx_count`/`artnet_rx_count` even when MIDI/OSC is active | `io_panel.rs:112,122` |
| R4 | `UiLayoutState::show_status_bar` defaults `false` — status bar invisible by default; spec requires always-on | `lib.rs:42,204` |
| R5 | Programmer selection bar fixture names always `"—"` — `PatchRes` not passed through | `programmer.rs:68` |

### Priority 2 — Spec Violations (visual fidelity)

| # | Issue | Location |
|---|---|---|
| V1 | Encoder hub readout uses `TextStyle::Body` — spec requires `FontId::monospace(18.0)` | `widgets.rs:521` |
| V2 | Fader fill is flat `accent * 0.8` — spec requires two-stop gradient mesh (`ACCENT → FADER_GRADIENT_BOTTOM`) | `widgets.rs:380` |
| V3 | Library tab active border drawn at `rect.min.y` (top) — spec requires `rect.max.y - 1.0` (bottom) | `library.rs:89` |
| V4 | Patch search is custom painted rect + overlapping `TextEdit` — double-renders, must be single widget | `patch.rs:60–75` |
| V5 | Colour active-row name hardcoded `"Custom"` — should match nearest preset name | `programmer.rs:163` |
| V6 | Minimize button wired in `panel_titlebar()` but docked rail headers don't call it — minimize unreachable from UI | `lib.rs:238–248` |
| V7 | Background neutrals (`BG_APP`, `BG_CHROME`) converted slightly too dark from oklch — depth contrast flatter than intended | `theme.rs` |
| V8 | Encoder/fader readout not calling monospace font — renders in proportional font at default body size | `widgets.rs:336,521` |

### Priority 3 — Missing Components

- Shift-click range select (stub only, no range computation) — `patch.rs:152`
- Live/idle `StatusDot` per fixture row in Patch list
- `dropzone()` Browse button must be inside the allocated rect, not below it — `widgets.rs:603–607`
- USB serial port enumeration chevron button — `io_panel.rs` USB section
- Protocol strip status dots read from hardcoded `DotState` instead of `IoConfig` live fields — `io_panel.rs:62`
- MIDI CC cells are read-only; Learn button is a TODO — `io_panel.rs` MIDI section
- PICK button absent from Colour active row — `programmer.rs:~170`
- `PanelChrome` shadow (`Frame::shadow`) not applied to any floating panel
- Detach/minimize icons are unicode fallbacks (`"⛶"` / `"━"`), not the spec SVG glyphs

### New Bindings (Rules 23–28)

#### 23. Library search query must be independent of the import path field
- **Rule:** The Library panel must maintain a separate `search_query` string in `ui.ctx().data_mut()` temp storage (keyed by a stable `egui::Id`). It must never alias `FixtureLibraryRes::import_path` or any other Resource field.
- **Action:** `library.rs:132`.

#### 24. All `ComboBox` widgets must have unique egui ids
- **Rule:** `egui::ComboBox::from_label("")` may not be used more than once in the same UI scope. Use `from_id_salt(unique_key)` or `from_label("unique visible label")`. Duplicate ids cause silent widget corruption.
- **Action:** `patch.rs:302,320` — change second to `ComboBox::from_id_salt("mode_combo")`.

#### 25. TX/RX counter must reflect the active protocol
- **Rule:** The IO panel TX/RX counter card must read counters from the Resource matching `IoState::active_protocol`, not always Art-Net. After the IoConfig split (Rule 22), each `*Stats` Resource provides its own counters.
- **Action:** `io_panel.rs:112,122`.

#### 26. `UiLayoutState::show_status_bar` must default to `true`
- **Rule:** The status bar is always visible per the design brief. Implement `Default` manually for `UiLayoutState` (do not use `#[derive(Default)]`) and set `show_status_bar: true`.
- **Action:** `lib.rs:42`.

#### 27. Fixture names must be resolved from `PatchRes` in the programmer selection bar
- **Rule:** The programmer panel must receive a reference to `PatchRes` and look up `FixtureInstance::name` for each selected id. Display as `"Name · Name · Name"` with a `"N/M"` count suffix. `"—"` is only shown when the selection is empty.
- **Action:** `programmer.rs:68`. Thread `patch: &PatchRes` through `programmer_panel_docked`.

#### 28. Encoder and fader readouts must use the spec font sizes
- **Rule:** Encoder hub value text must use `egui::FontId::monospace(18.0)`. Fader readout text must use `egui::FontId::monospace(14.0)`. Neither may fall back to `TextStyle::Body`.
- **Action:** `widgets.rs:521` (encoder), `widgets.rs:336` (fader).

### Frontend Fix Sequence (ordered)

| # | Regression/Rule | Files | Effort | Status |
|---|---|---|---|---|
| 1 | R4 — status bar default true (Rule 26) | `lib.rs:42` | 10 min | ✅ Done |
| 2 | R2 — ComboBox id collision (Rule 24) | `patch.rs:302,320` | 5 min | ✅ Done |
| 3 | R1 — Library search aliases import path (Rule 23) | `library.rs:132` | 30 min | ✅ Done |
| 4 | R5 — Fixture names in programmer bar (Rule 27) | `programmer.rs:68`, `lib.rs` call site | 45 min | ✅ Done |
| 5 | R3 — TX/RX counter active protocol (Rule 25) | `io_panel.rs:112,122` | 30 min | ✅ Done |
| 6 | V3 — Library tab border wrong edge | `library.rs:89` | 5 min | ✅ Done |
| 7 | V4 — Patch search double-render | `patch.rs:60–75` | 30 min | ✅ Done |
| 8 | V8 — Encoder/fader monospace font (Rule 28) | `widgets.rs:336,521` | 20 min | ✅ Done |
| 9 | V2 — Fader gradient fill | `widgets.rs:380` | 1 h | ✅ Done |
| 10 | V6 — Wire minimize button in docked rail headers | `lib.rs:238–248,264–272` | 30 min | ✅ Done |
| 11 | P3 — Protocol strip dots from `IoConfig` | `io_panel.rs:62` | 30 min | ✅ Done |
| 12 | P3 — `dropzone()` Browse button inside rect | `widgets.rs:603–607` | 30 min | ✅ Done (`widgets.rs`); library inline dropzones still use out-of-rect buttons — **in progress** |
| 13 | P3 — Shift-click range select | `patch.rs:152` | 1 h | ✅ Done |

### Remaining Open Items (post-audit)

| # | Item | Location | Rule | Status |
|---|---|---|---|---|
| A | Library GDTF/MVR/Venue inline dropzones place Browse outside allocated rect — replace with `widgets::dropzone()` | `library.rs:195–238, 287–315, 361–397` | P3 | ✅ Done |
| B | `library.rs` directly imports `stagelx_render::{VenueRoot, load_venue}` — add `LoadVenueEvent` to break cross-leaf dep | `library.rs:4`, `lib.rs:117` | Rule 21 | ✅ Done — `LoadVenueEvent` added to `stagelx-state`; render observer in `stagelx-render::on_load_venue` |
| C | Floating panels missing `Frame::shadow` | `lib.rs:497–545` | P3 | ✅ Done |
| D | Detach/minimize icons are unicode fallbacks (`"⛶"` / `"━"`) | `lib.rs`, `widgets.rs` | P3 (cosmetic) | ✅ Done — painter-drawn corners-out + bar glyphs |
| E | Compiler warnings cleanup (unused vars, dead assignments, unused imports) | multiple | warnings | ✅ Done |
| F | `allocate_ui_at_rect` deprecated → `allocate_new_ui` (~13 call sites across all panels) | `lib.rs`, `programmer.rs`, `io_panel.rs`, `widgets.rs` | deprecation | ✅ Done |

*Last updated: 2026-05-08 — All P1/P2/P3 audit items done (A–F); Rule 21 resolved via LoadVenueEvent; workspace builds clean with zero errors and zero warnings.*

---

### Phase 5 — Geometry, I/O Surfaces + Advanced Rendering (Weeks 17–20)
**Goal**: Real fixture/venue geometry, full input surface coverage, professional rendering.

**Geometry loading**
- [ ] Wire `stagelx_3ds::to_bevy_buffers()` into `on_fixture_spawned` (real GDTF models, cuboid fallback)
- [ ] OBJ venue loader (`tobj` → Bevy mesh, same pattern as 3DS)
- [ ] glTF/GLB venue loader (`gltf` crate → Bevy mesh)
- [ ] "Scene Assets" UI section: load venue files, place at configurable world offset

**I/O surfaces**
- [ ] MIDI input: `midir` callback → crossbeam → Bevy; CC → attribute mapping per fixture/group
- [ ] OSC input: `rosc` UDP → crossbeam → Bevy; `/fixture/{id}/{attr}` float messages
- [ ] MIDI + OSC config UI (device selector, port, CC mapping table)

**MVR export** (deferred from Phase 4)
- [ ] Write `GeneralSceneDescription.xml` from current patch + library
- [ ] Package GDTFs + XML into ZIP → save `.mvr` file

**Rendering upgrades**
- [x] Ray-marched volumetric fog cone in `BeamMaterial` WGSL shader (march view ray through cone volume, accumulate density)
- [x] Three-tier LOD: Tier 0 billboard sprite, Tier 1 half-res offscreen (16 steps), Tier 2 full-res (32 steps)
- [x] Hard cap 64 simultaneous ray-marched beams
- [x] Dynamic step count uniform in beam shader
- [ ] Front-to-back beam sorting (deferred to profiling phase)
- [ ] Split-screen viewports: primary FOH perspective (3/4 width) + top ortho + side ortho
- [x] Camera orbit/pan for FOH view; fixed orthographic cameras for top/side

**Milestone**: Load a real venue GLB, import an MVR with GDTF fixture models, control from a MIDI surface or OSC (TouchDesigner), see volumetric beams in three views simultaneously.

---

## Open Questions / Decisions Needed

1. **GDTF 3D model format**: GDTF v1.1 uses 3DS format for geometry. Need a 3DS loader or conversion pipeline (3DS → glTF at import time). Evaluate `three-d` or write a minimal 3DS loader.

2. **Bevy version**: Settled on Bevy 0.18.1. ✅

3. **Shader approach for beams**: Resolved — custom `BeamMaterial` via Bevy's `Material` trait + WGSL shader with additive blending. Gobo projection via rotating UV texture lookup in the beam material. ✅

4. **Cue system**: Not in scope for v1, but the data model should not foreclose adding a cue stack later. `DmxBuffer` should support named snapshots.

5. **GDTF-share API**: gdtf-share.com has a REST API for downloading fixture files by manufacturer/model. Worth integrating a fixture browser that can pull directly from the share.

6. **Test strategy**: GDTF files vary wildly in quality. Build a fixture file test corpus (grab 20–30 files from gdtf-share.com across manufacturers) and validate parser against them early.

7. **Art-Net node discovery**: ArtPoll/ArtPollReply not yet implemented. Nodes may need manual IP configuration until then.

8. **sACN multicast join**: `IP_ADD_MEMBERSHIP` not yet set — relies on IGMP snooping or broadcast fallback on managed LANs. Works on direct links; may need explicit join for complex network topologies.

---

## Non-Goals (v1)

- Cue stack / show playback (Phase 2+ of the project, after v1 ships)
- Video / media server integration
- Path-traced / offline rendering
- Mobile or web targets
- Network multi-user collaboration

---

## Repository Setup

```bash
git init
git remote add origin https://github.com/BlueJayLouche/stageLX.git
cargo init --name stageLX
# then convert to workspace and add crates/
```

Suggested `.gitignore`: standard Rust gitignore + `*.gdtf` test files (large binaries).

---

*Last updated: 2026-05-08 — Phase 5 mid-point audit complete; frontend UI review complete; Rules 10–28 added*
