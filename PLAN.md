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
- [ ] Ray-marched volumetric fog cone in `BeamMaterial` WGSL shader (march view ray through cone volume, accumulate density)
- [ ] Split-screen viewports: primary FOH perspective (3/4 width) + top ortho + side ortho
- [ ] Camera orbit/pan for FOH view; fixed orthographic cameras for top/side

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

*Last updated: 2026-05-07 — Phase 5 in progress*
