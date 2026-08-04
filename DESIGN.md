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

## The quartered centerpiece (Billy, 2026-08-04)

Billy's annotated screenshot (`docs/`) ruled the middle into three lettered
spaces plus a fourth cell, and the note read: *"seperate each element in the
middle as follows — something more like ss 2 for the fib spiral 8 bit (see
images 3–4). Space Z should be stylised as such (image 5 but with just one
portait in it). And then text box that generates."*

| Cell | Occupant | Reference / directive |
|------|----------|-----------------------|
| **X** top left | The performance controls — unfolding chips, then INDEX and RIP, then fb / master / glide / drone hz | Ordering confirmed unchanged; the column just gained a frame |
| **Y** top right | The Logalith | *ss 2*: a chunky 8-bit spiral with real arm thickness and staircased edges, in the register of the 1-bit pixel-art panels (images 3–4). Hairline vectors read as a diagram; the raster reads as an animal |
| **Z** bottom left | The portrait plinth | Image 5 — the striped pixel-art portraits — **but one portrait, not four**. Asset slot per dread band; the art is Billy's pen |
| bottom right | The voice | *"text box that **generates**"* — a teletype reveal, "but humanised": jittered per-character dwell and pauses at punctuation |

Both cuts are golden: top row `1/φ` of the height, left column the minor
section of the width (floored at the width the sliders need). Frames are the
standard 2 px, with a 5 px gutter so two adjacent frames never read as one
thick one.

Two consequences elsewhere, both Billy's call in the same pass:

- **The log becomes a marquee** in the header strip, in the gap he boxed
  between the wordmark and the device chips. Whole-pixel scroll only. The
  routine murmuring along the top is the same sphexish measurement log it
  always was; only its furniture changed.
- **The footer is the scopes alone** — the voice moved up into cell W and the
  log moved up into the header, so nothing else is left down there.

Still open in this area: whether the marquee's scroll rate should rise with
the dread band (the cadence idea, extended from the log's report interval to
its speed) — deliberately not built yet, since nothing should move in the
discouragement system without Billy's eyes on it.

## The mythology, and what the Logalith's panel is (Billy, 2026-08-04)

> "the fibonacci shell is a space entity using it's ability to emit resonant
> sound at targets to punish humans for doing experiments that locally
> subvert the law of physics it's form is completly bound to"

This is the oldest layer of the design and it retro-justifies the whole
instrument: **LOCAL φ INTEGRITY** is not a flavour readout, it is the measure
of how far the player has locally subverted the law the entity is bound to.
The discouragement system is that entity noticing. Nothing needs to change
mechanically — the meter, the dread bands and the voice pools were already
exactly this — but the *scene* should say it.

Built:

- **The sky.** 170 hash-placed stars behind the shell (deterministic, no RNG),
  1 px with some 2 px and occasional 4-point sparkles — the register of the
  1-bit space panels in Billy's images 3–4. The shell's silhouette is
  portrait-aspect by construction, so a landscape panel *keeps* room beside it
  for the scene rather than needing filler.
**Built, seen, and cut — do not re-propose:**

- **The planet.** Billy's Earth (`assets/earth.png`, 1000×1000, NASA MODIS
  imagery thresholded to 1-bit by him, with his own stars and a moon) was drawn
  lower-left at half the panel's shorter axis, with the shell's lower whorl
  crossing in front of it.
- **The beam.** Built exactly as chosen — integrity gating it, the live
  waveform travelling its length toward the target, fired from the spiral's
  terminus and drawn under the shell so it emerged from the animal.

Billy saw both in place and cut them the same session (2026-08-04): *"remove
the picture of a planet — remove the resonant line thing i requested."* The
mythology stays; the scene renders it with **stars only**. The reading worth
keeping is that the entity's panel needed atmosphere, not illustration — a
literal target and a literal weapon made the shell one element of a picture
instead of the thing you are looking at. The asset file is left in `assets/`
untracked, and nothing loads it.

Two things survive from that work and are load-bearing elsewhere: the 1-bit art
pipeline (area-averaged, Bayer-thresholded, whole-number reduction ratios) now
serves the portrait plinth, and the starfield stayed.

## The shell redesign (Billy, 2026-08-04: "audio reactive, biological, monolithic")

Billy asked for the option space rather than a proposal, and picked from it:

- **Form: the chambered cross-section.** A filled disc divided by whorl walls
  and septa — a nautilus actually cut open — instead of a ribbon following a
  spiral. The ribbon read as a *drawing of a spiral*; this reads as an animal,
  and it is why the middle no longer goes blobby. Rejected: solid-mass-with-
  structure-carved-out (most monolithic, but fine detail has nowhere to go),
  and keeping the ribbon.
- **Shading: engraved growth lines**, weight carrying operator level, thinning
  toward each chamber's inner edge as the depth cue. Rejected: dither patterns
  (the 4×4 grid reads as computer texture), flicker/temporal grey (true greys
  from two colours, but it shimmers, is unpleasant over large areas, and
  screenshots as noise — so reference shots would misrepresent it), and
  lines-plus-dither-in-the-shadows (richest, busiest, most tuning).
- **Reactivity: one chamber per operator.** The shell breathes in sections and
  the algorithm's structure is visible in it.

Consequences, both deliberate:

- The **emboss** Billy approved earlier is gone. It was a property of the arm,
  and there is no arm now; the engraved depth gradient does that job instead.
- The **side-signal edge ripple** is gone, since it was not among the
  reactivity choices. The `side` buffer went with it rather than being left
  collecting data nothing reads. Restoring it is a small change, not a rebuild.
- **Cracks were kept** even though they were not selected. They are the
  discouragement system's visual (see above), specified before this question was
  asked, and reading one unchecked box as "delete a documented mechanic" is the
  wrong call to make silently. Flagged to Billy rather than assumed.

### Second pass, same session: strokes, not pixels

Billy, against a watercolour of a flat many-whorled snail shell: *"more emphasis
on it being a fibonacci shell, i think we need more granularity... running it as
pixels is not the right idea."*

He was right, and the reason is arithmetic rather than taste. A raster cell was
11 px at his panel, so a whorl 14 px across **could not be drawn at all** — the
inner shell had to be faked as a solid core, and the coarse grid was the entire
obstacle to granularity. Strokes have no such floor.

What changed:

- **Filled → drawn.** Suture, septa, growth ribs, aperture; nothing filled. The
  reference is a drawing, so this is a drawing.
- **3 whorls → 5**, growth ≈3× → ×2. All five resolve now (innermost 15–31 px);
  the old rate only ever showed three, too few to say anything with.
- **Fibonacci made explicit.** Chambers per whorl are 34, 21, 13, 8, 5 —
  F(9)…F(5). This is the shell stating what it is, which is what Billy asked
  for; before, the Fibonacci was only in the growth law and invisible.
- **Ribs sweep.** 0.13 rad of lean, because a growth line is perpendicular to
  the direction of growth, not radial, and straight spokes read as a wheel.
- **Cheaper anyway:** ≈2,200 stroked segments against 8,360 per-cell tests.

### Third pass: it moves, and the sound reaches it

Billy: *"i meant how much they move... how much they are animated can be
increased in amplitude quite a bit"* — the waviness he wanted was the animation,
not the geometry — and *"instead of haunt/rip ruining the shape of it... convey
that some other way. i think fb, ratio mode and damp should have an effect on it
too."*

The shell now ripples on a travelling wave, and **the cracks are gone**: they
were costing more form than they were saying. Billy chose each mapping from a
menu, and the rule they all follow is that the visual is the engine's mechanic
*drawn*, never a mood assigned to a number:

| control | drawn as | because |
|---------|----------|---------|
| `rip` | the ripple agitates, faster and wider | rip folds the drone onto its own past: a wobble |
| `damp` | the shell's waves smooth out | damping is the removal of high frequencies, here in space |
| `fb` | every rib redrawn beside itself, dashed | feedback is a signal re-injecting its own past |
| `haunt` | ghost sutures at π/5, thinning by 0.92 a pass | that is precisely what the Ghost Line does to the sound |
| ratio mode | how many chambers divide each whorl | each mode's counts come from the sequence whose limit is its own constant |

**The seizure, found by accident.** Billy: *"whatever is happening to the shell
when haunt has triggered the shakes and my mouse picks up the rip slider is what
i want to happen at critical integrity."* What was happening was a bug — the
ripple phase was computed as `elapsed × rate`, so any change in rate threw the
phase by an amount proportional to session length; grabbing the trembling Rip
slider juddered its value a pixel at a time and the shell convulsed. The bug is
fixed (phases are integrated now, so a rate change only alters speed from that
moment) and the *state* is reproduced deliberately at critical dread: the phase is
re-thrown every frame across a full turn, one offset for the whole shell so the
ribs keep their relationships and the pattern flickers rather than dissolving.

Worth noting as a working pattern: the accident was worth keeping and the bug was
still worth fixing. Left as-is it would have behaved differently in minute one
than in hour two, and it could only ever be triggered by fighting a trembling
slider.

Rejected for rip/haunt: counter-rotating whorls (shear), agitation alone,
trembling-and-dashes. Rejected for the mode: growth rate per whorl following the
mode's constant — the biggest and truest option, still on the table, but it needs
the whorl count to adapt with it. Rejected elsewhere: fb as tremble or as ripple
frequency, damp as suture depth or rib reach.

### Fourth pass: a mass adrift, and the centre opens

Billy, on the shell at full master getting "right up in your face in a cool
aggressive way": *"how do we lean into that — i think more of it's moving around
the space would be cool."* Plus: smoothing on the integrity triggers, the shell
isn't visibly rotating, and *"it would be cool to have the middle dot empty space
of the shell to get larger with a parameter."*

- **Sway now grows with scale².** Billy's physical reasoning was right and I
  implemented it literally as linear — which is what a perspective projection
  actually does — and he could not see it, because ±22 px on a 717 px shell is
  under 3 % of its radius. Squaring is a deliberate exaggeration of a true effect.
  The sway is also no longer reserved for in the fit's headroom: budgeting it
  shrank the shell to keep the drift on-frame, which is the opposite of the point.
- **A continuous spin**, 377 s per turn at scale 1. The oscillating tilt was
  invisible for a reason worth remembering: a spiral leaned 4° looks like the same
  spiral, because the form is close to rotationally symmetric. Only a turn that
  keeps going gives the eye a marker (the aperture) to follow.
- **The umbilicus is INDEX**, inversely. Index 0 is pure sine carriers — the
  tree's depths are silent, so the shell has no interior. Raising INDEX blooms the
  tree from the shallow modulators *downward*, so the deepest operators engage
  last, and the deepest operators are the innermost whorls. The centre closes as
  the tree fills in. This is the mechanic, not a mood.
- **The critical band ramps** through a 0.9 s one-pole. The band still switches
  discretely with hysteresis; everything it drives fades.
- **The background stays the still thing.** Billy reasoned that if the shell is
  the thing choosing to approach, the background should be static — then said keep
  the warping because he likes it. Both hold: the sky has its own clock and does
  not slow with the shell, and that difference is what reads as parallax.

**13 tests now cover the shell** (`cargo test -p fibonacci-gui`). One of them
caught a false claim in these very docs: I had written that no chamber count is a
multiple of the operator count, and harmonic's 30 and fibonacci's 5 both are. The
claim was also not the property that mattered — chambers are numbered by one
running counter across the whole shell, so a radial stripe would need two whorls
sharing both a count *and* a starting operator. The test and the docs now assert
the true thing. Worth noting as the reason to test the geometry at all: the
invariants here (ribs never crossing, ends staying pinned, art reducing by whole
ratios) are ones I had been re-deriving by hand every time a constant moved.

Still on the table from the depth question: the directional cast shadow, and
the visible golden-rectangle construction lines. Billy is gathering more
reference images, so treat the current constants as a first pass — chamber
counts, growth, whorl count, sweep, ripple and rib behaviour are one line each.


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

## The recursive Room (Billy, 2026-08-03: "do the parallel/serial fib shit again")

The Room's density upgrade is not "more combs" — it is the instrument's own
grammar applied to its reverb. A Fibonacci recursion tree over comb-filter
leaves: each combination node chooses **series** (the sub-network feeds
through — cascaded echo density) or **parallel** (summed banks — classic
Schroeder). F(6) = 8 leaves (Fibonacci ✓) gives 5 combination nodes, a
32-point topology space, and a curated roster of Room algorithms — the
same unfolding idea as the synth. Ghost cross-feed generalizes: rotation
by 3 mod 8 is still coprime, the pentagram becomes an octagram. **Haas depth (Billy's nugget):** per-leaf pre-delays drawn from the
Fibonacci numbers *as milliseconds* — 1, 2, 3, 5, 8, 13, 21, 34 — which
sit exactly inside the Haas/precedence window (34 ms is the textbook
limit). L/R take adjacent Fibonacci values for precedence-effect depth,
and the mutually distinct offsets scatter coherent summing cancellations
instead of stacking them. **No new controls (Billy's constraint):** the recursive Room and Haas
depth add zero parameters. New behaviors ride the existing knobs —
haunt carries cross-feed and Haas spread, rt60/damp govern whatever
topology stands. The panel is part of the identity; it does not grow.
Open
design decisions for Billy: does the Room's tree follow the synth's
algorithm selection or get its own chips; and what the roster size is
(5 or 8). Slots after the bubble-chains and the current Room verdict —
the topology rebuild must not land while the sound is still being judged.

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
9. The quartered centerpiece: four framed cells, the 8-bit Logalith raster,
   the portrait plinth's slot, the generating voice box, the header marquee
   (done 2026-08-04 — see the section above)
10. Portrait art itself: a separate back-and-forth with Billy, whose mind has
   all the elements connected. The slot is built and waiting.
