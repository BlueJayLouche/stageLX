# stageLX

Real-time 3D stage lighting visualizer and DMX controller written in Rust + Bevy 0.18.

Targets medium-scale rigs (50–500 fixtures) at 60 fps on desktop hardware. Imports GDTF fixture definitions and MVR scene files; outputs DMX over Art-Net, sACN, and USB dongles.

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
    ├── stagelx-core/         # FixtureInstance, Patch, Universe, DmxBuffer
    ├── stagelx-gdtf/         # GDTF/MVR ZIP+XML parser
    ├── stagelx-dmx/          # DMX frame engine, HTP/LTP merge, DmxChannelMap
    ├── stagelx-state/        # Shared Bevy Resources + cross-crate events
    ├── stagelx-io/           # Art-Net, sACN, USB serial, MIDI, OSC I/O threads
    ├── stagelx-render/       # Bevy plugin: volumetric beams, gobos, fog, LOD
    └── stagelx-ui/           # egui panels: patch, programmer, library, DMX I/O
```

All feature crates (`io`, `render`, `ui`) are leaf nodes — none depend on each other. Cross-crate coordination goes through events in `stagelx-state`.

---

## Key dependencies

| Purpose | Crate |
|---|---|
| App / ECS / rendering | `bevy` 0.18 |
| UI panels | `bevy_egui` 0.39 + `egui` |
| I/O thread bridge | `crossbeam-channel` (bounded; no tokio) |
| GDTF/MVR parsing | `zip` + `quick-xml` |
| USB/serial DMX | `serialport` |
| MIDI | `midir` |
| OSC | `rosc` |
| File picker | `rfd` |

---

## UI panels

| Panel | Location |
|---|---|
| **Programmer** | Left rail — intensity, position, colour, gobo, effects |
| **DMX I/O** | Left rail — Art-Net / sACN / MIDI / OSC config |
| **3D Viewport** | Centre — FoH camera, beam/gobo render |
| **Patch** | Bottom — fixture list, address assignment, range select |
| **Library** | Bottom — GDTF/MVR/Venue import |

All panels support minimize and detach (float as independent windows).

---

## Docs

- [`PLAN.md`](PLAN.md) — architecture decisions, phase roadmap, audit findings
- [`REVIEW_NOTES.md`](REVIEW_NOTES.md) — UI implementation review: tiers 1–3 of issues and suggested fixes
