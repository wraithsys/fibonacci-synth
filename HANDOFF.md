# HANDOFF — Blow Your Phase Off

For a fresh Claude session continuing this project (likely on Billy's
laptop, `C:\Users\bbwra\fibonacci-synth`). Read this, then README.md (all
the mathematics) and DESIGN.md (visual language, specs, constraints).
The commit history narrates the whole build honestly — trust it.

## What this is

**Blow Your Phase Off** (BYPO): a monophonic FM drone instrument in Rust.
Fibonacci recursion generates the patch architecture; golden-ratio
inharmonicity is the identity ("endlessly ghostly, not big and bassy").
Entity/mascot: **the Logalith** (a chambered-nautilus logarithmic spiral —
deliberately NOT the golden-spiral myth). Private repo:
`wraithsys/fibonacci-synth`. Milestones 1–3 plus the full design pass are
complete; the instrument works, sounds right, and Billy loves it.
This is also Billy's Rust-over-C++ trial and his first release ambition:
"complete it and make it available to people."

Workspace: `fibonacci-dsp` (pure DSP, 34 tests, allocation-free audio
path), `fibonacci-app` (REPL shell), `fibonacci-gui` (the 1-bit egui face,
binary `blow-your-phase-off-gui`). Run: `cargo run --release -p
fibonacci-gui`. It drones immediately by design.

## How to work with Billy (this made the session excellent)

- **Roles**: Billy is design lead and final owner; Claude directs the
  engineering pipeline and proposes next steps. Flag decisions before
  they're irreversible.
- **One audible/visual change per verdict.** Ship, let him test, wait.
  Audition sound via rendered WAVs or live builds; audition visuals via
  his screenshots — which are diagnostic gold. Take the scope literally.
- **His "kooky rambles" are real signal in Billy-speak.** The Rip, the
  Haas idea, the recursive Room, the discouragement design — all came
  from throwaway messages. Translate; never dismiss.
- **Measure before theorizing.** `examples/verb_probe.rs` and
  `verb_probe2.rs` exist because ear-reports beat hypotheses; reproduce
  exact user configs offline. The Room took SEVEN ear-driven fixes
  (clipping, inverted normalization, zipper, low makeup, static-source
  stillness, DC catastrophe, CRLF text parsing) — every complaint was a
  distinct real bug.
- Asks go through dialog-box questions (AskUserQuestion) when possible.
- If the exe is locked, Billy is running it: say so and wait — never
  build to a second directory.

## Laws (do not break; most are documented in README/DESIGN)

1. **Docs contract**: every constant/model that reaches ears or eyes is
   documented in README's "mathematical models" section. Undocumented
   magic numbers are bugs.
2. **Program-authored text = verifiable measurements only.** All *voiced*
   text is Billy's, in `crates/fibonacci-gui/assets/relic_log.json` —
   hot reloaded, characters own their entries, format documented in
   `assets/RELIC_LOG.md`. His register: mundane science + something
   extraordinary + a human reacting. Never florid ("not an Alex Kurtzman
   Star Trek show"). Windows-authored content needs meeting halfway: strip
   BOMs and CRLF, or the file silently parses as nothing.
3. **Roster counts are Fibonacci**: algorithms 5 (→8→13), ratio modes 5,
   melodic tunings 5, scales 8, comb leaves in the future Room 8.
4. **No envelopes, ever.** It drones. Off switches are true resets
   (fade ~2 ms then zero phase/state), tested.
5. **Determinism**: bit-identical output for identical control sequences,
   chunk-size invariant, tested.
6. **Sound is sacred**: no compression/saturation/filtering on the dry
   path. The Room's wet bus may growl (tanh), never click. The
   discouragement system is presentational ONLY — nothing resists,
   nothing degrades.
7. **No new controls**: new behaviors ride existing knobs (haunt carries
   cross-feed/Haas in the future Room).
8. **Strict 1-bit UI, no anti-aliasing** (feathering off; OS theme pinned
   — never let system light mode in). Dither = brightness.

## State right now

The centerpiece is **quartered**: X controls (with a fake `PARAMETERS`
window title bar), Y the Logalith, Z the record card, W the voice. Both
cuts golden. 75 tests green.

**The Controls & UI sweep — DONE (2026-08-05, all 13 items, every one
ear/eye-verdicted by Billy).** His notes live at
`projects\shared\billy-notes\Controls & UI Sweep.md`. What it changed:

- **Knob tapers, all documented in README**: MASTER renders
  `position^φ` (the golden ease-in) and *glides* per-sample at a
  Fibonacci 13 ms — it was stepping per-block, which was the "clicky".
  INDEX travel is `position^φ`, chosen because it exactly linearizes
  feedback's concave `x^(1/φ)` (infinite slope at 0 = the old cliff);
  the whole tree now re-levels per-sample through the same 13 ms glide
  (`PARAM_GLIDE_S`), because the taper made the always-there per-block
  stepping audible. GLIDE is `2·position^φ⁴` — ~65% of travel below
  0.1 s. The pattern to remember: **a taper that speeds the knob
  uncovers the zipper underneath; taper and glide travel together.**
- **Master gates the Room's input** (`Frame.master`): the Room used to
  hear pre-master ops, so master-down + wet-up still sounded. Tested.
- **rt60 is honest now**: `examples/rt60_probe.rs` (charge, cut, time
  the fall to −60 dB) measured real decay saturating near 7 s — the
  feedback cap plus the tap-wobble's interpolation loss — so the knob
  runs 0.05–8 s log. And full in-loop damp was shaving ~20% off the
  tail; the loops now keep 1/φ² of the knob.
- **damp is an MS-20**: resonant 4-pole (SVF + plain 2-pole) on the wet
  bus straight into the tanh. Cutoff falls ten golden steps; resonance
  wakes at the 1/φ knee, peaks at Q = φ⁴. *Outside* the comb loops on
  purpose — a resonant peak inside recirculation is an oscillator.
- **Presets moved to a header pane** (the `presets` chip): one box
  filters and names; everything double-confirms (select = dotted bevel,
  confirm = invert until release); saves never overwrite (`~X`
  suffix); DELETE double-confirms against the highlighted preset.
- **Layout**: STRUCTURE panel and the footer scope are gone (scope
  deleted for good on Billy's verdict; drawing code in history at
  `8d13d36^`). SCALE is a view toggle — second press flips the scale
  bank away and gives the phase image back. The phase image owns the
  left panel's remaining height. Resize-down priority: bottom row
  hides first (<220 px), the Logalith second (<200 px wide), controls
  never; window floors at 800×600. Marquee wears Pixeloid Sans
  (font_probe-vetted) so the log reads apart from the device text.

- **The Logalith** is a five-whorled Fibonacci shell drawn in strokes, not
  pixels. Chamber counts per whorl come from the ratio mode's own integer
  sequence (fibonacci → 34/21/13/8/5, golden → Lucas, plastic → Padovan),
  so switching tuning makes it a different creature. Ripples on a
  travelling wave; `rip` sets speed, `damp` smooths, `fb` echoes each rib,
  `haunt` raises ghost sutures at π/5, `master` scales it. Convulses at
  critical dread. All of it in README.
- **The voice** is `relic_log.json`: 30 entries, 11 characters, ids
  auto-assigned as Fibonacci numbers. Witnesses speak while integrity
  holds; the entity intrudes on **agitation** (a leaky integral of
  max(rip,haunt) — the forgiveness rule made mechanical) plus a standing
  8% decay chance. Intrusions render in inverted type at console speed
  (90 cps, dead even); humans type at 30 cps with jitter and punctuation
  holds. The old `voice.txt`/`integrity_*.txt` pools are retired.
- **Space Z is empty pending a rethink.** The portrait pipeline — dithered
  1-bit text grids drawn as animated point clouds, ordered by ink density
  and ping-ponged — is in history at `52c5a99`; the grid parser, frame
  loader, batch converter (`examples/png_to_grid.rs`) and Billy's 14 grids
  all survive. The zone is **574×412, aspect 1.39**, and the app logs its
  own figure on startup and resize. Briefs: `assets/portraits/README.md`
  and `IMAGE_BRIEF.md`.
- `[profile.dev] opt-level = 2` so a bare `cargo run` still sounds right.
  Presets and `state.json` are gitignored user data.

## Open queue

**Billy's items**: the eleven portrait images (brief written, he is on it);
Space Z's rethink; 1-bit icon/sigil; starter-preset bank; roster-to-8.

**THE GATE IS CLEARED (2026-08-05).** The Xilla history rewrite is done, on
Billy's instruction: git-filter-repo stripped the blob from all 46 commits,
the old GitHub repo was deleted (Billy, web UI) and a fresh private one
pushed under the same name — because a force-push alone does not purge
GitHub. Verified: path in zero commits, blob 404s from GitHub's API, tip
tree hash byte-identical across the rewrite. Full record in README's "The
Xilla history rewrite — DONE". **Any clone from before 2026-08-05 evening
is orphaned: re-clone, never pull** — that includes Billy's laptop.
Publication now waits only on content and release engineering.

**Fonts — DONE (2026-08-05).** Twelve faces in `assets/fonts/<slug>/`, each
with its `LICENSE.txt`, roster and credits in `assets/fonts/NOTICE.md`.
`install_fonts` loads all of them with named families, Unifont Ex Mono as
the fallback beneath every one, and **every witness speaks in their own
face** — archetype → face by normalised prefix, so the Monk's non-breaking
hyphen (U+2011) and the eight `Logalith Intrusion A–H` all resolve. Sizes
land on each face's measured pixel grid. `assets/font.otf` and the four
loose `Xilla *.otf` sources are deleted. Model documented in README's type
roster section; three new tests gate it.

What the vetting found, because it matters for any future asset:
- dafont's "Public domain / GPL / OFL" filter is a **self-declaration**.
  dafont holds no document and verifies nothing, and the three-licence
  bundle isn't actionable anyway — OFL *requires* you ship its text, so
  "one of these three" cannot be complied with.
- 7 of 12 shipped a licence file; 3 more declared it in the font's own
  `name` table (ids 13/14) with no file; 4 had nothing at all. Three of
  those four were jeti/fontenddev.com, whose About page states CC BY 4.0 —
  quoted and dated inside each `LICENSE.txt`, since that page is the only
  record. `Minitel` had nothing anywhere and was rejected. `vhs-vcr-osd`
  was CC BY-SA 3.0 and was dropped: Share-Alike in a shipped binary is a
  lawyer's question.
- **`examples/font_probe.rs`** reports embedded licence + glyph coverage
  for any font. Run it before proposing a face, not after.
- Only **Unifont Ex Mono** and **Better VCR** have complete coverage of
  φ ρ π Δ • ◦ ¤ and the subscripts. Pixel Operator does not, which is why
  it lost the UI role to Pixeloid Mono. Unifont is 13.7 MB — earns its
  place as the fallback, but subsetting is an option nobody has built.
- Four fonts are CC BY 4.0 and **must** be credited. NOTICE.md carries both
  the minimum wording and Billy's requested over-the-top version (credits
  typeset in the fonts they credit).

One thing the grid measurement corrected: **Modern DOS is 16 px native, not
8** — the 8 in `ModernDOS8x16` is its width. Ten of the twelve faces measure
16, both Pixeloids 9, PixAntiqua has no grid at all. Pixeloid Mono keeps the
UI role, which puts UI sizes at 9 and 18 (Billy chose this over moving the
whole interface to a single 16 px grid).

**Space Z — DONE (2026-08-05). The record card, across the whole zone**
(Billy: "its coverage can be the whole of that zblock"). `RELIC <id>` on an
inverted strip, the archetype in that character's own face, then ERA /
TSTAMP / ALIAS / EXTRA in a fixed label column, then rarity as pips out of
eight. Rows never change shape — blanks show `——` — because most entries
carry no metadata and a card that resized every 45 s would read as a
glitch. Reads the same relic the voice does, so card and words agree.

The plinth shared the zone for about an hour on a golden cut and was then
**parked**: Billy's call, image work is not the thread now. The drawing
side is at `52c5a99` and has been restored from there once already, so
bringing it back is a known move, not a rewrite.

**Portraits — Billy delivered ten (2026-08-05), 5 of 11 slots load.**
His bundle `assets/portraits/zboxtxtportraits.txt` (sections marked
`/name`) is split into grid files beside it. **Two format bugs it exposed,
both fixed**: `.` and `0` used to count as empty, so art using `0` as a tone
was hollowed out (one portrait was 28 marks out of 749) — **only a space is
empty now**, and the 14 older grids were migrated losslessly. And the width
cap was 96 while nine of ten were 98–120 wide, so they were silently
refused — cap is now 128. A test parses every grid in the folder and fails
under 3% ink, because both failures look exactly like art nobody drew yet.

**Still Billy's**: four delivered portraits are unassigned — `human1`,
`human2`, `human3`, `non_human1` — and only he knows which archetype each
is; rename to the slot filename and they are live. Three more
(`cultist`, `schism_leader`, `borg_cube_easter_egg`) are characters the
relic log has never heard of and need entries, which is his text. Full
status table in `assets/portraits/README.md`.

**Engineering next**:
1. **The recursive Room** — full spec in DESIGN.md: Fibonacci tree over
   8 comb leaves, ghost cross-feed becomes rotation-3 octagram, Haas
   pre-delays from Fibonacci milliseconds, zero new controls. **Parked by
   Billy's explicit call (2026-08-05)** — the sweep's decay rework
   ("do my note now, park recursive Room") is landed and approved; the
   recursive spec stays a separate future campaign. Do NOT land it
   without Billy's ears on standby.
2. Release engineering: public README with demo WAVs (`cargo run --release
   --example render`), v1.0.0 tag, portable Windows build, license,
   distribution decision (Billy's call). **If any of that means making the
   repo public, the history rewrite above happens first.**

## Asset convention (Billy, 2026-08-04)

- `assets/fonts/<slug>/` — a font plus its `LICENSE.txt`. Nothing else.
- `assets/pool/` — confirmed, cleared assets. **Does not exist yet.**
- Anything unconfirmed stays untracked and gets **deleted** once tested,
  not committed "just in case".

Currently untracked and awaiting that sort: 18 loose `.jpg` sources in
`assets/`, `earth.png`, 14 converted portrait grids in `assets/portraits/`,
and `docs/`. All are derivatives of material whose licence is not
established. They were deliberately left out of every commit — the same
discipline that makes Xilla a blocker applies to pictures.

## Traps this session actually hit

- **Never restructure source with PowerShell string surgery.** `Get-Content
  -Raw` without `-Encoding UTF8` reads UTF-8 as ANSI and turns every φ, π
  and — into mojibake across the whole file. Use the Edit tool. (Recovered
  losslessly by re-encoding through CP1252, but don't repeat it.)
- **A panel drawn in "whatever height is left" will silently vanish.** The
  phase scope stopped drawing when app-wide padding was added. Reserve
  space; don't scavenge it.
- **Phases must be integrated, not `elapsed × rate`.** Multiplying
  accumulated time by a rate makes the phase jump whenever the rate
  changes, by an amount proportional to session length. Billy found this
  by dragging a trembling slider.
- **Test the mechanic, not the number.** A threshold-with-discharge design
  for agitation was killed by a test proving the discharge could never gate
  anything at box cadence.

## Tone

Billy gushes when happy and says "kooky" when brilliant. He found every
audio bug by ear and described each one accurately before the mechanism
was known. Trust his ears, translate his words, keep the mathematics
honest, and the collaboration runs itself.
