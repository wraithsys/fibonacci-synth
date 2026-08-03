# Design language — Blow Your Phase Off

Compiled from Billy's reference notes (`projects\shared\billy-notes\fibonacci\`,
Obsidian vault, 2026-08-03). Billy is design lead; this file is the working
spec the implementation follows. The entity is **the Logalith** (renamed from
Monologalith — too close to the Korg Monologue).

**One sentence:** occult laboratory equipment, rendered in strict 1-bit.

## Hard rules (already committed or in force)

- **Strict 1-bit**: two colors, white on black. All shading is dithering.
- **No anti-aliasing**: crisp deliberate pixels (egui feathering off, strokes
  pixel-snapped, integer font sizes). Reference: World of Horror screens.
- **Hard rectangles**, thick borders, panels packed edge to edge.
- Every displayed number/formula stays true (README docs contract).

## Directives from the references → implementation map

| Reference | Directive | Where it lands |
|-----------|-----------|----------------|
| Analog scope photo (green phosphor Lissajous) | The L/R scope becomes a proper instrument: graticule grid, thick beam with dither falloff (phosphor persistence, monochrome) | scopes panel |
| Schlappi THREE BODY panel | **Bubble-chain connectors**: chains of small circles of varying radius instead of plain lines | tree edges, pentagram arcs, panel flow lines |
| WoH combat screen | The unaliased chunky look; slot-box controls; log formatting | global |
| WoH top strip | Header bar: small chip left (session clock), title center, stat chips + version right | title strip |
| Nautilus photo + shadow figure | **The Logalith is a chambered shell**: the centerpiece spiral gains septa (chamber-wall arcs between whorls); consider the golden-rectangle construction (subdivided squares + quarter arcs) as the drawing method — diagram, not smooth curve | centerpiece |
| Golden-spiral rosette constructions | Composite/overlaid spiral constructions as ornament; construction lines left visible | centerpiece, background texture |
| Sacred geometry chart + annotated Metatron page | Glyph language for panel headers and markers; real equations as marginalia around the geometry (the annotated-notebook look — our formulas, styled like field notes) | panel headers, φ-INT meter, margins |
| Red string-art sigil | Density-as-value: guilloche/string-art line density rising with a parameter | the Room panel as haunt rises |
| Bold mercury-like glyph | Icon direction: one heavy geometric sigil, drawable at 16×16. Candidate: construct a BYPO sigil from the pentagram + spiral + φ. **Billy owns the final mark.** | app icon, wordmark |
| Game Boy poster, Lain terminal, Vhikk X / GRONE panels (gold, maroon) | **Mood + website/packaging palette**, not the app: the app stays 1-bit. Chunky display type and warm accent palettes go to the release site/artwork. | website, packaging |

## Open items (Billy's calls)

- **Font**: Billy picked **Xilla** — installed as
  `crates/fibonacci-gui/assets/font.otf`, loaded at startup with the
  built-in monospace kept as glyph fallback (Xilla is a small display face;
  φ, ×, — and friends may fall back). **License TODO**: Xilla arrived
  without a license file (Cozette/Press Start 2P/Terminus in
  `projects\fonts` all have theirs) — terms must be confirmed before
  release, or we swap to one of the licensed alternates. Font *sizes* may
  need retuning to Xilla's native size in the design pass.
- **Icon**: final sigil — I can prototype constructions, Billy decides.
- Whether the centerpiece uses the smooth polar spiral, the rectangle
  construction, or construction-lines-over-curve (my lean: construction
  lines + curve, since the diagrams are the reference that got the "umm
  this").

## The instrument disapproves (Billy's discouragement design)

BYPO takes the opposite of the usual synth ethos: build complete
infrastructure for pushing limits, then *actively discourage* using it.
Esoteric and malleable, but reluctant. Fear of pushing past the limit is a
designed feeling, delivered diegetically.

**The prime rule: discouragement is presentational, never functional.**
No control resists, no audio degrades, nothing is gated. Sound is sacred.
The player is always allowed everything; the instrument just *notices*.

Mechanics (all data-driven, Billy authors the content):

- **Graduated integrity states**: the voice pools switch by measured state.
  `integrity_low.txt` (< 50%) is live; further bands to add:
  `integrity_critical.txt` (< 15%), possibly a mild `integrity_warn.txt`
  (< 75%). Thresholds get hysteresis so states don't flicker cheaply.
- **The log stays sphexish**: measurements only, but their *cadence* rises
  with violence — the maintenance routine reporting more often is the
  closest a routine gets to alarm.
- **Visual dread**: already partly built (the Logalith cracks, φ-INTEGRITY
  drains). Extend: slider/control trembles at critical integrity (the
  controls that caused it), pentagram densification. Purely visual.
- **Forgiveness**: integrity recovers when the player relents — the
  instrument never holds a grudge, which is precisely what makes pushing
  it again feel like a choice.

Slots after the Room verdict + bubble-chains; content (the pools) belongs
to the content-lock phase.

## Implementation order (design pass)

1. Feathering off + pixel-snap pass (done with the rename)
2. Header strip rework (WoH top bar)
3. Bubble-chain connector primitive; apply to tree + pentagram
4. Logalith rebuild: golden-rectangle construction + septa + side-signal
   displacement (keep the crack-out behavior)
5. Scope rework: graticule + thick dithered beam
6. String-art densification of the Room panel with haunt
7. Font loading from assets; marginalia styling for the formulas
8. Icon + wordmark once Billy's mark exists
