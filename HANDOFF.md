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
window title bar), Y the Logalith, Z **deliberately empty** ("clear the
zone"), W the voice. Both cuts golden. 59 tests green.

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
Space Z's rethink; **the font conflict — blocks any public release**;
1-bit icon/sigil; starter-preset bank; roster-to-8.

**Fonts — files are IN, plumbing is NOT.** Twelve faces sit in
`assets/fonts/<slug>/`, each with its `LICENSE.txt`, roster and credits in
`assets/fonts/NOTICE.md`. **Nothing loads them yet** — `install_font` still
loads the single `assets/font.otf` (Xilla). That is the next job: a
multi-font system with named roles, the archetype mapping from NOTICE.md,
and Unifont as the fallback family so gaps resolve cleanly.

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

Xilla itself is untouched: still `assets/font.otf`, still in history since
`0419ee7`, still the release blocker. Billy's call is **rewrite history with
git-filter-repo once the swap lands**, in one pass, with him watching.

**Engineering next**:
1. **Multi-font plumbing** — roles, archetype mapping, Unifont fallback.
   Then delete `font.otf` and schedule the history rewrite.
2. **Space Z = the record's header** (Billy's pick): archetype, era,
   tstamp, alias, extra, id — the found-document furniture, beside the log
   text in W. Zone is 574×412, aspect 1.39; the app logs its own figure.
3. **The recursive Room** — full spec in DESIGN.md: Fibonacci tree over
   8 comb leaves, ghost cross-feed becomes rotation-3 octagram, Haas
   pre-delays from Fibonacci milliseconds, zero new controls. Do NOT land
   it without Billy's ears on standby.
4. Release engineering: public README with demo WAVs (`cargo run --release
   --example render`), v1.0.0 tag, portable Windows build, license,
   distribution decision (Billy's call).

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
