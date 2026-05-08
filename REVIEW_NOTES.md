# Review notes — stageLX UI implementation v1

The implementation has the **right bones** — token map, panel structure, widget vocabulary, plugin shape — but a handful of **systemic issues** are stopping it from looking and feeling like the spec. Don't start over. Fix the items in Tier 1, and ~80% of the "off" feeling will go away.

Numbers in `[brackets]` reference files the agent uploaded.

---

## TIER 1 — Why it doesn't look like the design (fix these first)

### 1. No fonts are registered

There's no `egui::FontDefinitions` setup anywhere — egui falls back to its default proportional/monospace fonts (Ubuntu Light / ProggyClean). Every type-scale entry in the spec assumes IBM Plex Sans + IBM Plex Mono. Without them, the whole UI reads as "default egui app."

**Fix.** In `lib.rs` (`StageLxUiPlugin::build`), before the first frame:

```rust
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("plex_sans".into(),
        egui::FontData::from_static(include_bytes!("../assets/IBMPlexSans-Regular.ttf")).into());
    fonts.font_data.insert("plex_mono".into(),
        egui::FontData::from_static(include_bytes!("../assets/IBMPlexMono-Regular.ttf")).into());
    fonts.families.entry(egui::FontFamily::Proportional).or_default()
        .insert(0, "plex_sans".into());
    fonts.families.entry(egui::FontFamily::Monospace).or_default()
        .insert(0, "plex_mono".into());
    ctx.set_fonts(fonts);
}
```

Add `assets/` to the crate, ship the TTFs there. (Bold/Medium weights via additional `FontData` entries.)

### 2. No global spacing override

egui's default `style.spacing.item_spacing` is `(8, 3)` and `button_padding` is `(4, 1)`. None of those match the 4-px grid in the spec. **Every horizontal gap, every vertical rhythm, every button height is being driven by egui defaults right now**, which is why panels look "loosely arranged" instead of "console-tight."

**Fix.** Right next to where `style.visuals.*` is set in `ui_root_system`:

```rust
style.spacing.item_spacing = Vec2::new(6.0, 4.0);
style.spacing.button_padding = Vec2::new(8.0, 4.0);
style.spacing.interact_size = Vec2::new(0.0, 24.0); // default control height
style.spacing.icon_width = 12.0;
style.spacing.menu_margin = egui::Margin::same(6);
style.spacing.window_margin = egui::Margin::same(0);
```

Then **delete** the `ui.add_space(N)` calls that were compensating for the defaults — many of them are no longer needed once spacing is correct.

### 3. Custom `painter.text()` calls collapse the type scale

In `widgets.rs`, `programmer.rs`, `patch.rs`, `library.rs`, `io_panel.rs`, you have ~40 `painter.text(...)` calls that pass `egui::TextStyle::Body.resolve(ui.style())`. **All of them render at the same size** — egui's body size, ~14 px proportional. The spec calls for 9 / 10 / 11 / 12 / 14 / 16 / 18 px depending on the role.

**Fix.** Anywhere you call `painter.text`, pass an **explicit `FontId`**, not `TextStyle::Body`. Add helpers next to the existing typography functions in `theme.rs`:

```rust
pub fn font_eyebrow()        -> egui::FontId { egui::FontId::monospace(9.0) }
pub fn font_hint()           -> egui::FontId { egui::FontId::monospace(9.0) }
pub fn font_status()         -> egui::FontId { egui::FontId::monospace(10.0) }
pub fn font_field_label()    -> egui::FontId { egui::FontId::proportional(10.0) }
pub fn font_body()           -> egui::FontId { egui::FontId::proportional(11.0) }
pub fn font_address()        -> egui::FontId { egui::FontId::monospace(11.0) }
pub fn font_panel_title()    -> egui::FontId { egui::FontId::proportional(11.0) }
pub fn font_show_name()      -> egui::FontId { egui::FontId::proportional(12.0) }
pub fn font_fader_readout()  -> egui::FontId { egui::FontId::monospace(14.0) }
pub fn font_wordmark()       -> egui::FontId { egui::FontId::proportional(14.0) }
pub fn font_big_counter()    -> egui::FontId { egui::FontId::monospace(16.0) }
pub fn font_encoder_readout()-> egui::FontId { egui::FontId::monospace(18.0) }
```

Then in painter calls: `painter.text(pos, align, text, font_eyebrow(), FG_MUTED)`.

### 4. `BG_CHROME` and `BG_PANEL` are inverted in tone

Spec: panel body (0.190) is **lighter** than chrome (0.165). Your tokens make panel `rgb(18,20,22)` and chrome `rgb(19,22,26)` — chrome is lighter than panel, so titlebars look brighter than the bodies they introduce.

**Fix.** Swap or recompute. Quick visual fix:
```rust
pub const BG_CHROME: Color32 = Color32::from_rgb(22, 25, 28);   // was 19,22,26
pub const BG_PANEL : Color32 = Color32::from_rgb(28, 31, 34);   // was 18,20,22
pub const BG_RAISED: Color32 = Color32::from_rgb(36, 39, 42);   // was 26,28,30
```

### 5. The wordmark is two labels with a space gap

```rust
ui.label(wordmark("stage"));
ui.label(wordmark_accent("LX"));
```

This renders as `stage LX` because `style.spacing.item_spacing.x` puts a gap between siblings.

**Fix.** Render both halves in one painted galley (`LayoutJob`) so it's a single text run with the accent color applied to "LX" only:

```rust
let mut job = egui::text::LayoutJob::default();
job.append("stage", 0.0, egui::TextFormat { font_id: font_wordmark(), color: FG, ..Default::default() });
job.append("LX",    0.0, egui::TextFormat { font_id: font_wordmark(), color: ACCENT, ..Default::default() });
ui.label(job);
```

### 6. Top-bar vertical dividers are positioned by `ui.cursor()` and drift

```rust
ui.painter().line_segment(
    [Pos2::new(ui.cursor().min.x, ui.cursor().min.y),
     Pos2::new(ui.cursor().min.x, ui.cursor().min.y + ui.available_height())],
    Stroke::new(1.0, BORDER_SOFT));
ui.add_space(14.0);
```

`ui.cursor()` returns a *rect of available space*, and `add_space(14)` after the paint moves it — but the divider was already drawn at the pre-space position, then 14 px of empty space follows it. The divider is also drawn at the top of the available height, not vertically centered in the 36-px bar.

**Fix.** Allocate a `Vec2::new(1.0, 24.0)` rect and paint the line in the center of *that* rect:

```rust
fn vertical_divider(ui: &mut Ui, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, height), Sense::hover());
    ui.painter().line_segment(
        [Pos2::new(rect.center().x, rect.min.y), Pos2::new(rect.center().x, rect.max.y)],
        Stroke::new(1.0, BORDER_SOFT));
}
```

Same issue exists in `programmer.rs` (the Quick-Actions top divider) and at every panel's titlebar bottom border.

### 7. `panel_titlebar` ends with `ui.separator()`

`ui.separator()` is egui's chunky default divider — it's ~6 px tall, gray, with built-in vertical padding. The spec wants a **1-px hairline** in `BORDER`.

**Fix.** Replace `ui.separator()` with:
```rust
let p = ui.available_rect_before_wrap();
ui.painter().line_segment(
    [Pos2::new(p.min.x, p.min.y), Pos2::new(p.max.x, p.min.y)],
    Stroke::new(1.0, BORDER));
```
…or wrap the whole titlebar in `egui::Frame::new().stroke(Stroke::new(1.0, BORDER)).show(...)` with `inner_margin` controlling the border placement.

### 8. Toggle label is outside the pill

In `widgets.rs::toggle`, the track is inside the rect at x=0–32, but the label is painted at `rect.min.x + 34`. The spec said the label sits *inside* the pill, after the thumb.

**Fix.** Either:
- (a) Reduce the rect width to just the track (32 px) and move the label outside as a sibling `ui.label()` — keep current visual but make it explicit; or
- (b) Make the rect wide enough for both, paint the label inside the pill bg fill, not outside it. The current state — label outside the rect's filled area, but counted as part of the rect — is the worst of both.

### 9. Magnifier is a hint-text emoji

```rust
TextEdit::singleline(&mut filter.query).hint_text("🔍  Filter by name, type, address…")
```

Spec calls for a **drawn icon at left padding 7 px**. The emoji renders inline as text and disappears the moment the user starts typing. Cargo also doesn't have an emoji font registered, so on most platforms this'll render as a tofu box.

**Fix.** Wrap the input in a horizontal layout: paint a 12-px magnifier glyph (two line segments + circle), then the `TextEdit` next to it, both inside a single `Frame::new()` with `BG_INPUT` fill so it reads as one control. (Or, simpler: use a dedicated `search_icon(ui)` widget that paints the icon and returns a Response that focuses the next sibling on click.)

### 10. Strobe fader math is wrong

`programmer.rs` (Intensity section):

```rust
let strobe_norm = prog.strobe;            // 0..1
let mut strobe_pct = strobe_norm * 100.0; // 0..100
ui.add(Fader::new(&mut strobe_pct, "Strobe").unit("Hz")…);
```

Then inside `Fader::ui` (`widgets.rs`):

```rust
let readout = if self.unit == "Hz" && *self.value < 0.01 {
    "OFF".to_string()
} else {
    format!("{:.0}", *self.value * 100.0)   // <-- multiplies by 100 AGAIN
};
```

So at full strobe (`prog.strobe = 1.0`, `strobe_pct = 100.0`), the readout reads "10000". At "no strobe" (`prog.strobe = 0.0`, `strobe_pct = 0.0`), readout reads "0" but spec wanted "OFF" — and the threshold check uses 0.01 against a 0..100 number, so it never triggers.

Also: spec says strobe is 0–25 Hz, not 0–100 Hz.

**Fix.** Have `Fader` take a `range: RangeInclusive<f32>` and a value-to-display closure. Or simpler — make `Fader` always operate on the **caller's natural units**:

```rust
pub struct Fader<'a> {
    pub value: &'a mut f32,           // already in display units
    pub min: f32,
    pub max: f32,
    pub label: &'a str,
    pub unit: &'a str,
    pub format: fn(f32) -> String,    // owns the readout text
    pub accent: Color32,
    pub height: f32,
}
```

Then dimmer is `0..=100`, strobe is `0..=25`, and the fader doesn't have to guess scales.

---

## TIER 2 — Interaction bugs

### 11. Encoder drag is way too sensitive

`widgets.rs::Encoder::ui`:
```rust
*self.value = (*self.value + delta * range * 0.01).clamp(self.min, self.max);
```

For a Pan encoder with range = 540°, that's **5.4° per pixel of drag**. A casual mouse twitch sweeps the full range.

**Fix.**
- Default sensitivity: ~0.2% of range per pixel.
- Hold `Shift` for fine control (×0.1).
- Hold `Ctrl/Cmd` for coarse (×5).
- Use Y-axis (up = increase) instead of X — it matches both physical encoders and other DAW/CAD encoders.
- Double-click: reset to default (you have this, but it resets to mid-range, which is wrong for Zoom — make the default a config field).

### 12. Patch row layout is broken

`patch.rs` defines column widths in one place:
```rust
let cols = [32.0, available_width * 0.25, available_width * 0.30, available_width * 0.18, 78.0, 32.0];
```
…then *manually* increments `x` while painting each cell **with different offsets that don't match `cols`**:
```rust
x += 40.0;                       // after index, but cols[0] = 32 + 8 spacing = 40 ✓
x += full_width * 0.25 + 8.0;    // after name
…
painter.text(Pos2::new(x + 70.0, …), Align2::RIGHT_CENTER, addr_text, …); // hardcoded 70
```

The address column ends up rendered at an x-position that has nothing to do with the column boundaries. On wide rails the address overlaps the mode column; on narrow ones it's off-screen.

**Fix.** Compute the column **rects** once, then paint into them. Better: use `ui.columns(6, …)` or `egui_extras::TableBuilder` (egui_extras is the standard egui table widget — pin to your `bevy_egui` version's egui).

```rust
use egui_extras::{TableBuilder, Column};

TableBuilder::new(ui)
    .striped(false)  // we draw our own stripes
    .column(Column::exact(32.0))                           // #
    .column(Column::remainder().at_least(80.0))             // Name
    .column(Column::remainder().at_least(100.0))            // Type
    .column(Column::auto().at_least(60.0))                  // Mode
    .column(Column::exact(78.0))                            // Address
    .column(Column::exact(32.0))                            // Status
    .header(20.0, |mut row| { row.col(|ui| { ui.label(…) }); … })
    .body(|mut body| { for f in &fixtures { body.row(24.0, |mut row| {…}); } });
```

Add `egui_extras = { version = "...", features = ["all_loaders"] }` to `Cargo.toml` (match the version your `bevy_egui` ships).

### 13. Patch list scroll area paints background at the wrong place

```rust
let (list_rect, _) = ui.allocate_exact_size(Vec2::new(available_width, list_height), …);
if ui.is_rect_visible(list_rect) {
    painter.rect_filled(list_rect, 3.0, BG_INPUT);
    painter.rect_stroke(list_rect, 3.0, …);
    egui::ScrollArea::vertical().max_height(list_height).show(ui, |ui| { /* rows */ });
}
```

You allocated a rect (advancing the layout cursor by `list_height`), painted into it, then *also* called `ScrollArea` which allocates **its own rect at the new cursor position**. So the painted "list bg" sits empty above the actual rows. Net effect: a stripe of `BG_INPUT` followed by transparent rows.

**Fix.** Either:
- Wrap the `ScrollArea` in a `Frame::new().fill(BG_INPUT).stroke(…).show(ui, |ui| ScrollArea::vertical().show(ui, |ui| {…}))`; or
- Use `ui.allocate_new_ui(UiBuilder::new().max_rect(list_rect), |ui| ScrollArea::vertical().auto_shrink([false,false]).show(ui, |ui| {…}))` so the scroll content is constrained to the rect you painted.

### 14. `StrokeKind::Middle` everywhere causes blurry borders

Most `rect_stroke` calls use `StrokeKind::Middle` — egui draws the 1-px stroke straddling the pixel boundary, so each border is two half-pixels at 50% alpha → blur.

**Fix.** Switch every panel/card/control border to `StrokeKind::Inside`:
```rust
painter.rect_stroke(rect, 3.0, Stroke::new(1.0, BORDER_SOFT), StrokeKind::Inside);
```

### 15. Library tabs allocate `RichText` then never use it

```rust
let mut rich = RichText::new(label).size(11.0).color(…);
if active { rich = rich.strong(); }
…
painter.text(rect.center(), Align2::CENTER_CENTER, label, TextStyle::Body.resolve(…), …);
```

The rich text is built and dropped; the actual paint uses raw `label` with `TextStyle::Body`. So size 11/strong are never applied. Remove the dead code; pass a `FontId` to `painter.text` per Tier-1 issue #3.

### 16. `status_to_dot` parses the status string

```rust
fn status_to_dot(s: &str) -> widgets::DotState {
    if s.contains("bound") || s.contains("TX") || …
}
```

This is brittle and locale-fragile. Add an enum to the existing `IoConfig` resource (`ProtocolStatus { Idle, Live, Warn, Error }`) and update the supervisor that owns the runtime to write it. Strip the string once, at the source.

### 17. Mode tabs hardcode active = "Program"

`let active = i == 2;` in the top bar. Add a Resource:

```rust
#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub enum AppMode { Setup, Patch, #[default] Program, Run }
```

Read/write it in the top bar.

---

## TIER 3 — Architecture / cleanup

### 18. Replace the "allocate rect → paint card → allocate_new_ui into rect" pattern with `egui::Frame`

This pattern appears ~12 times across the panels:
```rust
let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
painter.rect_filled(rect, 3.0, BG_INPUT);
painter.rect_stroke(rect, 3.0, Stroke::new(1.0, BORDER_SOFT), StrokeKind::Middle);
ui.allocate_new_ui(UiBuilder::new().max_rect(rect), |ui| {
    ui.add_space(8.0);
    /* contents */
});
```

Equivalent, far cleaner:
```rust
egui::Frame::new()
    .fill(BG_INPUT)
    .stroke(Stroke::new(1.0, BORDER_SOFT))
    .corner_radius(3.0)
    .inner_margin(egui::Margin::symmetric(10, 8))
    .show(ui, |ui| { /* contents */ });
```

Make a `card(ui, |ui| {…})` helper and use it everywhere. This alone deletes ~150 lines.

### 19. Two sibling separators on every panel titlebar

`panel_titlebar` ends with `ui.separator()` AND each rail's titlebar code adds an extra `painter.line_segment` underneath. That's a double border. Pick one. Recommendation: remove `ui.separator()` from `panel_titlebar` (per Tier-1 #7) and let the explicit `Frame` border handle it.

### 20. Remove the obsolete legacy entry points

`programmer_panel`, `patch_panel`, `library_panel`, `io_panel` are now empty stubs marked "kept for API compat." The plugin no longer registers them. Delete them — anyone still calling them was already broken.

### 21. Hardcoded copy

Top bar: `"tour-2026-mainstage"`, `"SAVED 12s ago"`, `"FPS 60.0"`, `"CPU 14%"`, `"U1 81/512"`, `"BPM 128.0"`. All placeholder.

Add resources for each: `ShowMeta { name, last_saved: Instant }`, `RuntimeStats { fps: f32, cpu_pct: f32 }`, derive universe usage from `PatchRes`, etc. Keep them in placeholder form for now if that's faster, but **mark them with `// TODO(stub)`** so it's clear what's not real yet.

### 22. Encoder `arc_points` always sweeps min→max

```rust
let start = start_deg.min(end_deg);
let end = start_deg.max(end_deg);
```

In current usage `start < end` always (track is -135 → +135, fill is -135 → angle), so this works. But it'll silently break if you ever try to draw a fill on a "negative" axis (e.g., a +/- centered encoder). Either remove the swap, or make the function explicit-direction.

---

## What's working — don't change

- **Token map.** The oklch-to-Color32 conversions are close enough to the spec, and the structure (surfaces / borders / text / accents / semantics / derived) matches.
- **Panel + Plugin shape.** `StageLxUiPlugin`, `UiLayoutState`, `PatchSelection`, `IoPanelState`, `ActiveProtocol` — exactly the right resources to add.
- **Detach/dock state machine.** Floating windows that re-dock on click is right; keep this.
- **Patch selection model.** Cmd/Ctrl-click-toggle + Shift-range with `anchor_id` is correct.
- **Encoder/Fader struct shape.** Builder-pattern fluent API with `.range()`, `.unit()`, `.sub()`, `.accent()` is good — the bugs are inside, not around.
- **GDTF/MVR import wiring.** Pulling `rfd::FileDialog` into the dropzone is the right move; loaders are unchanged.

---

## Suggested order of attack for the implementer

1. Tier-1 issues #1, #2, #3 (fonts, spacing, font scale) — **largest visual swing for least code.** Do all three in one pass.
2. Tier-1 #6, #7, #8, #9, #10 (dividers, separator, toggle, search, strobe) — small fixes, each visible.
3. Tier-2 #11 (encoder sensitivity) and #12 (patch table via `egui_extras::TableBuilder`) — biggest interaction wins.
4. Tier-3 #18 (`Frame` refactor) — pure cleanup, but unlocks fast iteration after.
5. Everything else as polish.

After step 1+2, take screenshots and put them next to `prototype/index.html` artboards side-by-side. The remaining gaps will be obvious.

---

## Should you switch approaches?

**No.** The agent picked the right architecture (custom widgets in `widgets.rs`, panel modules per-area, single root system, resource-backed state). The issues above are **execution bugs and missing config**, not strategy errors. A second pass with this review pinned should land it.

The one thing I'd suggest changing: **stop doing manual `painter.text` + `painter.rect_*` for layout-flow content**. Reserve raw painter calls for *true custom-painted widgets* (encoder dial, fader, status dot, swatch chip, gobo glyph). Everything else — toolbars, rows, forms, banners — should be `egui::Frame` + `ui.horizontal/vertical` + `ui.label/add_sized`. You'll write half the code, and the layout will adapt to spacing changes for free.
