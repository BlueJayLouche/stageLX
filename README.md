# stageLX

Real-time 3D stage lighting visualizer and DMX controller written in Rust + Bevy 0.18.

Targets medium-scale rigs (50–500 fixtures) at 60 fps on desktop hardware. Imports GDTF fixture definitions and MVR scene files; outputs DMX over Art-Net, sACN, and USB dongles. MIDI and OSC input for control surfaces.

---

## Quick start

```bash
cargo run --release
```

Minimum window size: 720 × 480. Tested on macOS (Apple Silicon). Linux/Windows not yet validated.

---

## Workspace

```
stageLX/
├── src/main.rs               # Bevy App wiring
└── crates/
    ├── stagelx-core/         # FixtureInstance, Patch, Universe, DmxBuffer, DmxChannelMap
    ├── stagelx-patch/        # PatchRes, PatchEditState, DmxAddress, fixture lifecycle events
    ├── stagelx-show/         # Programmer, CueStack, CuePlayhead, PerfDiagnosticsRes
    ├── stagelx-gdtf/         # GDTF/MVR ZIP+XML parser, MVR import/export
    ├── stagelx-dmx/          # DMX frame engine, HTP/LTP merge, cue→DMX projection
    ├── stagelx-io/           # Art-Net, sACN, USB serial, MIDI, OSC I/O threads
    ├── stagelx-render/       # Bevy plugin: volumetric beams, gobos, fog, LOD, viewports
    └── stagelx-ui/           # egui panels: patch, programmer, cue, library, DMX I/O
```

All feature crates (`io`, `render`) are leaf nodes — none depend on each other. Cross-crate coordination goes through events in `stagelx-patch` and `stagelx-show`.

---

## Key dependencies

| Purpose | Crate |
|---|---|
| App / ECS / rendering | `bevy` 0.18 |
| UI panels | `bevy_egui` 0.39 + `egui` |
| I/O thread bridge | `crossbeam-channel` (bounded; no tokio) |
| GDTF/MVR parsing | `zip` + `quick-xml` |
| 3D model loading | `tobj`, `gltf`, `ufbx` |
| USB/serial DMX | `serialport` |
| MIDI | `midir` |
| OSC | `rosc` |
| File picker | `rfd` |

---

## Features

### Rendering
- Volumetric beam cones with additive blending and custom WGSL shader
- Three-tier LOD (billboard sprite → half-res offscreen → full-res ray-march)
- Gobo projection via projected spotlight
- Split-screen viewports (FOH + top + side cameras)
- Async GDTF geometry loading (3DS/GLB/OBJ/FBX venue support)

### DMX Engine
- 64 universes, 512 channels each
- HTP/LTP merge with priority stack (Programmer > Cue > External)
- Pre-computed `DmxChannelMap` per fixture — no per-tick string lookups
- 44 Hz output tick via Bevy `FixedUpdate`

### I/O Protocols
- **Art-Net** — TX/RX, node discovery (ArtPoll/ArtPollReply), source allowlist
- **sACN (E1.31)** — TX/RX, multicast join, configurable priority
- **USB DMX** — Enttec USB Pro protocol
- **MIDI** — CC/Note input, rate-limited port scan
- **OSC** — path-based control (`/fixture/{id}/{attribute}`, `/cue/{id}/go`)

### Cue System (v1 foundation)
- Cue stack with GO / BACK / RECORD
- Keyboard shortcuts: `Enter` = GO, `Shift+Enter` = BACK
- JSON persistence to `show.json`
- Cue playback as priority-150 DMX source

---

## UI panels

| Panel | Location |
|---|---|
| **Programmer** | Left rail — intensity, position, colour, gobo, effects |
| **Cue** | Left rail — cue list, GO / BACK / RECORD |
| **DMX I/O** | Right rail — Art-Net / sACN / USB / MIDI / OSC config |
| **3D Viewport** | Centre — FoH + top + side cameras |
| **Patch** | Bottom strip — fixture list, address assignment, range select |
| **Library** | Bottom strip — GDTF/MVR/Venue import |

All panels support minimize and detach (float as independent windows).

---

## Security advisories

| Advisory | Crate | Status |
|---|---|---|
| [RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436) | `paste` (via `bevy` → `wgpu` → `metal`) | **Accepted** — `paste` is unmaintained but functional. Transitive dependency; resolution requires Bevy upgrade. |

---

## Docs

- [`PLAN.md`](PLAN.md) — architecture decisions, phase roadmap, ADRs
- [`CHANGELOG.md`](CHANGELOG.md) — release history
