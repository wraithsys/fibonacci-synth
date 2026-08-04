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
   text is Billy's, in the data files (`crates/fibonacci-gui/assets/
   voice.txt`, `integrity_low.txt`, `integrity_critical.txt`) — hot
   reloaded, blank-line-separated boxes, `#` paragraphs ignored,
   CRLF-safe. His register: mundane science + something extraordinary +
   a human reacting. Never florid ("not an Alex Kurtzman Star Trek show").
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

- HEAD `4f89ce0`: centerpiece layout rework (controls in a left column,
  Logalith full-height) — **awaiting Billy's verdict**.
- Discouragement mechanics live: dread bands (Normal/Low <47.5%/Critical
  <12.5%, hysteresis), pool cascade, log cadence 30 s/10 s, ±1 px tremble
  on rip+haunt at critical. Pools are comment-only templates — **Billy
  still has to write them**.
- Two-machine flow: sessions build+push; Billy pulls. Watch for Zed/CRLF
  phantom-dirtying `main.rs` on the laptop (stash it, don't fight it).
  `[profile.dev] opt-level = 2` so a bare `cargo run` still sounds right.
- Presets (`crates/fibonacci-gui/presets/`) and `state.json` are
  gitignored user data. Billy's presets so far: drama, throaty, nasty
  (on his machines).

## Open queue

Billy's items: voice pools + any voice.txt rewrites (his pen only);
layout verdict; 1-bit icon/sigil (heavy mercury-glyph register, drawable
at 16×16); **Xilla font license confirmation — blocks any public
release** (licensed alternates sit in `C:\Users\...\projects\fonts`);
starter-preset-bank decision; roster-to-8 decision.

Engineering next (in rough order):
1. **The recursive Room** — full spec in DESIGN.md: Fibonacci tree over
   8 comb leaves (series/parallel per node, 32-topology space, curated
   roster), ghost cross-feed becomes rotation-3 octagram, Haas pre-delays
   from Fibonacci milliseconds (1..34 = the Haas window), zero new
   controls. Do NOT land it without Billy's ears on standby.
2. Release engineering: public README with demo WAVs (regenerate via
   `cargo run --release --example render`), v1.0.0 tag, zipped portable
   Windows build, license, distribution decision (GitHub vs itch.io —
   undecided, Billy's call).

## Tone

Billy gushes when happy and says "kooky" when brilliant. He found every
audio bug by ear and described each one accurately before the mechanism
was known. Trust his ears, translate his words, keep the mathematics
honest, and the collaboration runs itself.
