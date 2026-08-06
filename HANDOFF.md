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
deliberately NOT the golden-spiral myth). **Public repo**:
`wraithsys/fibonacci-synth` — **v1.0.0 and v1.0.1 SHIPPED 2026-08-05**, MIT,
portable Windows zip + 32 demo renders on the GitHub release. Milestones
1–3, the full design pass, the controls sweep and release engineering are
all complete; the instrument works, sounds right, and Billy loves it. This
was Billy's Rust-over-C++ trial and his first release ambition, fulfilled:
"complete it and make it available to people."

**v1.0.2 is in progress (2026-08-06)** — Billy's "shareable release". It is
NOT released; five commits sit local and unpushed at his instruction, and he
wants more changes in before it goes. Section below.

Workspace: `fibonacci-dsp` (pure DSP, allocation-free audio path),
`fibonacci-app` (REPL shell), `fibonacci-gui` (the 1-bit egui face, binary
`blow-your-phase-off-gui`). Run: `cargo run --release -p fibonacci-gui`. It
drones immediately by design. **87 tests green** across the workspace.

## Before you touch anything, on any machine

Two traps that have each already cost a session's opening hour:

1. **Is this clone orphaned?** The Xilla history rewrite (2026-08-05)
   changed every commit hash. Run
   `git rev-list --left-right --count HEAD...origin/master` — a large
   ahead+behind split with *duplicate commit messages* on both sides means
   you are on the dead pre-rewrite line, which still carries the blob.
   Re-point with `git reset --hard origin/master`; never pull or merge.
   The laptop was exactly this on 2026-08-06 (41 ahead / 52 behind, tree
   byte-identical to its rewritten twin, so nothing was lost).
2. **Does `.cargo/config.toml` exist, and does its linker path name *this*
   machine's user?** It is **untracked as of 2026-08-06** and each machine
   keeps its own. Without it you get the default MSVC linker, which works
   — you only lose the LLD speedup. With a *stale* one you get
   `error: linker ... not found`, and rustc's follow-on note about
   `link.exe` and Visual Studio is a red herring: VS and the SDK are
   installed and fine on both machines. Desktop user is `akind`, laptop
   `bbwra`.

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
   (fade ~2 ms then zero phase/state), tested. ⚑ **Under challenge by
   Billy himself** — the "fluidity" ask in the v1.0.2 note wants a natural
   envelope on MIDI note-on, "more like a compressor bloom than anything
   sharp". It is his law to amend and he has not amended it. Do not build
   toward it until he says so explicitly.
5. **Determinism**: bit-identical output for identical control sequences,
   chunk-size invariant, tested.
6. **Sound is sacred**: no compression/saturation/filtering on the dry
   path. The Room's wet bus may growl (tanh), never click. The
   discouragement system is presentational ONLY — nothing resists,
   nothing degrades.
7. **No new controls**: new behaviors ride existing knobs (haunt carries
   cross-feed/Haas in the future Room). ⚑ Three deferred v1.0.2 asks — the
   audio pane, the MIDI page, HARMONY mode — add surface. The presets pane
   is the precedent for how: a *pane*, opened from a header chip, not new
   knobs on the face. Raise the tension with Billy rather than resolving it
   quietly.
8. **Strict 1-bit UI, no anti-aliasing** (feathering off; OS theme pinned
   — never let system light mode in). Dither = brightness.

## State right now

The centerpiece is **quartered**: X controls (with a fake `PARAMETERS`
window title bar), Y the Logalith, Z the record card, W the voice. Both
cuts golden. 87 tests green.

## The v1.0.2 campaign — Billy's "shareable release"

His brief is a note in his Obsidian vault:
`C:\Users\bbwra\Documents\Obsidian Vault\v1.0.2.md`, with screenshots
beside it. He asked for it to be prioritised **with** him, and chose a
scope: trust + UI polish only. Everything else in that note is explicitly
out of scope for this release, not forgotten (list below).

**Done (2026-08-06), all verdicted by Billy:**

- **Preset banks.** A preset is `(bank, name)` now, never a name alone. Root
  of `presets/` is the player's own saves (`MINE` chip); every folder under
  it is a bank. The folder name is identity, chip label *and* contributor
  credit at once — rename the folder and all three follow, so nothing in the
  source knows a contributor's name. **Why it exists:** Billy intends to ask
  testers who like the instrument for presets and ship them credited in a
  folder of their own. One `.gitignore` negation per bank replaced 32
  per-preset lines — a form that had already failed once, with `init.json`
  un-negated for a whole release. Stock bank is `presets/BYPO/`, 42 presets.
  Filter chips (`ALL` / `MINE` / one per non-empty bank) fire on a *single*
  press, the only pane control that does: the double-usage principle guards
  actions, and a filter is a view. Saves always land in `MINE`; deletes
  reach anything, Billy's call — "the copy on someone's machine is theirs."
- **`init` is the default preset**, living in the stock bank as an ordinary
  preset. Startup: `state.json` → `init` → hardcoded. Resaving `init`
  changes what a fresh install opens on.
- **The PRESETS pane lost its window buttons.** They stay on `PARAMETERS`,
  which is *pretending* to be a utility window; the presets pane genuinely
  is one you opened and can close, so a dead cross on it is a lie not a
  costume.
- **The PITCH readout** (melody panel): is pitch being quantised, and to
  what. Full model in README. `Tuning::quantization()` is in the **DSP**,
  beside the tunings, and is tested against what they emit rather than what
  they declare.
- **The Defender answer** — README section, below.
- **`examples/ppm_to_png.rs`** — `BYPO_SHOT` writes PPM, which no storefront
  takes. This converts. Use a **Windows-style path** in `BYPO_SHOT`; the
  seconds split at the *first* colon so a drive letter survives.

**Left in scope: the new logo, and nothing else.** Billy's art is
`Documents\Obsidian Vault\Untitled design (2).png` — a **greyscale** dithered
square, so it is not 1-bit and cannot go inside the interface without being
dithered down first (Law 8). He wants it for **the exe/window icon and the
itch.io cover**, explicitly *not* in-app. Two open questions he has not
answered: whether it replaces the Logalith disc or gets composed into the
same ringed treatment, and how a square becomes a 630×500 cover — pad
(letterboxed) or crop (loses its edges). The existing icon path captures the
Logalith from the live renderer via `BYPO_SHOT` and composes with
`tools/logo_compose.py`; the new art is not a capture, so it needs a
different route into `icon/icon.ico` and `icon_256.rgba`.

**Deferred out of v1.0.2 by Billy, from the same note** — these are real
asks, not dead ones:

1. **"Fluidity"** — a natural envelope on MIDI note-on with sustain, or
   paired to the melody rate; "more like a compressor bloom than anything
   sharp", Fibonacci-derived, drone-compatible. **This collides head-on with
   Law 4 (no envelopes, ever) and Law 7 (no new controls).** Do not build it
   without Billy explicitly amending Law 4 — it is his law and his call, and
   the "bloom not attack" framing may make it legal as a level gesture.
2. **HARMONY mode** — in THE ROOM's empty lower box. Delays pitch changes
   *between carriers*, same machinery as melody but range defines the gap
   between intervals, Fibonacci-integrated. Must read as unusable in
   algorithms without multiple carriers.
3. **Audio pane** — output device selection and recording, in the same
   header-pane idiom as presets, **with ASIO**. Billy says to copy ASIO from
   other applications "we've made" — nobody in this session's context knows
   which; ask him for the repo. Constraint worth raising early: cpal's
   `asio` feature needs the Steinberg SDK plus LLVM/bindgen, and **the SDK
   cannot be redistributed**.
4. **MIDI page**, mono only, with MIDI control switched off whenever melody
   or harmony is active. `midir` is already a dependency.
5. **OBS** — make the app reachable for demo recording, automated into a
   scene he has already built.

## Going public: Defender, and how v1.0.2 actually ships

**The exe is deleted on sight by Windows Defender as
`Trojan:Win32/Wacatac.C!ml`.** Confirmed on Billy's own machine
(`Get-MpThreatDetection`), not merely reported. This is a false positive and
the detection name is the evidence: `!ml` means no signature matched — a
classifier looked at an unsigned binary from an unknown publisher and
guessed, `Wacatac` is the bucket guesses land in, and the "executes commands
from an attacker" line is boilerplate for the family. New Rust binaries trip
it constantly (rust-analyzer; a DLL inside rustc's own std).

Distribution channel does **not** fix this — that is a SmartScreen lever, and
Defender is a different system that scans the file wherever it came from.

- **The fix is a false-positive submission** to
  <https://www.microsoft.com/wdsi/filesubmission>, **as a software
  developer**, with the SHA-256 and the detection name. Free. Only Billy can
  send it. **Analysis is per file hash**, so clearing v1.0.1 does nothing for
  the v1.0.2 binary: build the release artifact, submit *that* exe, wait for
  it to clear, then publish. v1.0.1's exe is
  `D7A7B97FA649201532CE58CC48E890759EA6AD9486AF1E29B140627CDDDDEF7D`, intact
  inside the zip in Downloads (Defender ate the extracted copy, not the
  archived one).
- **Signing is off the table.** Microsoft's Azure Artifact Signing (renamed
  from Trusted Signing) is $9.99/month and now takes individual developers —
  but individual validation is **USA and Canada only**, and Billy is in the
  **UK**. A traditional OV/EV certificate is still priced for companies.
- **`package.ps1` writes `<name>-SHA256SUMS.txt` every build**, listing the
  zip and the exe, in the `<hash>  <name>` coreutils format. The README's
  safety section is built around it: everything else there asks the reader
  to trust a claim, and the hash is the one line they can check.
- **Distribution: itch.io, keeping GitHub Releases** (Billy's call). Page
  copy, settings, assets and a checklist are drafted at `docs/itch-page.md`
  — **`docs/` is gitignored**, so it is not in history. The words are a
  draft for Billy to own. The four **CC BY 4.0** fonts must be credited on
  the store page too, not just in the zip; the minimum wording is in
  `assets/fonts/NOTICE.md` and is already in the draft.

**Before publishing**: bump the crates to 1.0.2 (they are at 1.0.1),
`pwsh tools/package.ps1 -Smoke`, submit the hash, wait, then upload.

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
  suffix); DELETE double-confirms against the highlighted preset. The
  pane grew bank chips and lost its window buttons in v1.0.2, above.
- **Layout**: STRUCTURE panel and the footer scope are gone (scope
  deleted for good on Billy's verdict; drawing code in history at
  `1b80f75^`). SCALE is a view toggle — second press flips the scale
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
- **Space Z is the record card** (the rethink landed 2026-08-05, section
  below). The portrait pipeline — dithered
  1-bit text grids drawn as animated point clouds, ordered by ink density
  and ping-ponged — is in history at `d4a326d`; the grid parser, frame
  loader, batch converter (`examples/png_to_grid.rs`) and Billy's 14 grids
  all survive. The zone is **574×412, aspect 1.39**, and the app logs its
  own figure on startup and resize. Briefs: `assets/portraits/README.md`
  and `IMAGE_BRIEF.md`.
- `[profile.dev] opt-level = 2` so a bare `cargo run` still sounds right.
  Presets and `state.json` are gitignored user data.

## Open queue

**The live queue is the v1.0.2 section above** — the logo is the only item
left in scope, and Billy wants further changes in before it ships.

**Billy's items**: roster-to-8. The starter bank is **done** (42 presets in
`presets/BYPO/`) and the 1-bit icon **exists** — the open logo question is
about replacing it, not creating one. **Portraits are deferred indefinitely past
v1** (Billy, 2026-08-05: left "unless I decide I want to overcome the
hurdles of it") — the four unassigned grids and the three unwritten
characters wait with them, and nothing in the app currently draws them, so
the deferral costs zero visible surface. Note for anyone reading old lists:
"Billy's pools" (`integrity_low/critical.txt`) is a retired item — the
relic-log restructure replaced the pool mechanism entirely; his text lives
in `relic_log.json` and there is nothing called pools left to write.

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
side is at `d4a326d` and has been restored from there once already, so
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

**Portraits — deferred indefinitely (Billy, 2026-08-05).** The state below
holds if he ever picks it back up: four delivered portraits are unassigned
— `human1`, `human2`, `human3`, `non_human1` — and only he knows which
archetype each is; rename to the slot filename and they are live. Three
more (`cultist`, `schism_leader`, `borg_cube_easter_egg`) are characters
the relic log has never heard of and need entries, which is his text. Full
status table in `assets/portraits/README.md`. Not a v1 item.

**Release engineering — SHIPPED (2026-08-05).** v1.0.0 is tagged and
public: MIT (fonts separately licensed, NOTICE.md rides every
redistribution), roster stays 5 (Billy's call — dormant tree nodes keep
waiting), the starter bank is his seven presets tracked by explicit
gitignore negation (new saves stay private; growing the bank = adding a
negation line on purpose), and `tools/package.ps1 -Smoke` stages, launches
the exe from a foreign cwd (assets/presets are exe-anchored now), and
zips. Demo renders attach to releases, never the repo.

Three things about that script were **wrong until 2026-08-06**, all fixed:
`.cargo/config.toml` was described here as untracked but was in fact still
tracked — the `.gitignore` line existed and nobody had ever run
`git rm --cached`, so it did nothing; the bank was gathered by globbing
`presets\*.json` off the *filesystem* rather than asking git, so a zip built
on a working machine would have shipped whatever the player saved that
afternoon; and `smoke-stderr.txt` **shipped inside the v1.0.1 zip**, because
`Stop-Process -Force` returns before Windows releases the redirect handle,
the delete failed, and `-ErrorAction SilentlyContinue` swallowed it. The
capture now lives outside the staging directory, the bank comes from
`git ls-files`, and a check throws if anything unexpected is staged at all.

**The icon exists (2026-08-05, Billy: "that's it").** The Logalith at
integrity 0%, captured from the real renderer by the new `BYPO_SHOT`
env tool (`seconds:path` → binary PPM of the window, then exit — also
your promo-shot tool), composed by `tools/logo_compose.py` into a
ringed black disc, and wired twice from one art: `icon/icon.ico` into
the exe resource via build.rs + winresource, `icon_256.rgba` raw into
the window icon. Capture recipe that matters: **master 0.5**, so the
shell renders whole at scale 1.0 — anything larger leans into the
panel clip and wears a flat edge no recrop can heal.

**v1.0.1 SHIPPED (2026-08-05 evening)**: the icon build plus the full
bank — Billy's laptop delivered 25 more presets (all made that evening
on the post-sweep engine, every value already in range), 32 total,
every algorithm and all five ratio modes represented, each tracked by
its own gitignore negation line. v1.0.0 stands beneath it.

**Engineering next**:
1. **Finish v1.0.2** — the logo, then whatever else Billy pulls forward from
   the deferred list. See the campaign section at the top.
2. **The recursive Room** — full spec in DESIGN.md: Fibonacci tree over
   8 comb leaves, ghost cross-feed becomes rotation-3 octagram, Haas
   pre-delays from Fibonacci milliseconds, zero new controls. **Parked by
   Billy's explicit call (2026-08-05)**; a future campaign, and a v1.x
   sound change when it comes. Do NOT land it without Billy's ears on
   standby.
3. Post-release: whatever the public surfaces — issues are open now.

## Asset convention (Billy, 2026-08-04)

- `assets/fonts/<slug>/` — a font plus its `LICENSE.txt`. Nothing else.
- `assets/pool/` — confirmed, cleared assets. **Does not exist yet.**
- Anything unconfirmed stays untracked and gets **deleted** once tested,
  not committed "just in case".

Currently untracked and awaiting that sort (**counted 2026-08-06**, the
previous figures here had drifted): 17 loose `.jpg` sources in `assets/`,
`earth.png`, and 25 `.txt` files in `assets/portraits/` — 24 grids plus
Billy's original `zboxtxtportraits.txt` bundle. Also `docs/`, which now
holds the itch.io page draft. All are derivatives of material whose licence is not
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
  anything at box cadence. Same move landed again on 2026-08-06: the
  quantisation readout is tested by *firing 600 notes per tuning* and
  counting distinct pitches, not by asserting the labels match.

New on 2026-08-06:

- **`allocate_ui` inherits the parent's layout direction.** Inside
  `ui.horizontal(...)`, a child allocated for a column lays its contents out
  *left to right*. The presets pane's first cut put the bank list and the
  filter chips in one strip running off the edge of the window. Both columns
  now say `Layout::top_down` explicitly. Billy diagnosed it with a
  screenshot and the words "the pane is a mess", which was enough.
- **`cargo test` does not run an example's own tests** unless the target is
  declared in `Cargo.toml` with `test = true`. Four tests in
  `examples/ppm_to_png.rs` would otherwise have run only when someone
  remembered to ask.
- **Don't put transient files in a directory you are about to archive.** See
  `smoke-stderr.txt` above — the cleanup raced a file handle and lost, and
  `-ErrorAction SilentlyContinue` made it silent.
- **Ask git what ships, not the filesystem.** Same section. Anything that
  globs a working directory will eventually sweep up user data.
- **Verify a claim before writing it into public documentation.** Two lines
  drafted for the README's safety section were pulled on checking: "no
  registry write" (unprovable across eframe and winit) and a promise of
  published hashes (not true until it was implemented). The network claim
  survived *because* it was checked — no networking crate in `Cargo.lock`,
  no `std::net` in any source, nothing that opens a URL.
- **Billy may be running the app while you work.** He saved ten presets
  during the v1.0.2 session. Check timestamps before moving anything under
  `presets/`, and expect `state.json` to change under you. If a build fails
  with "failed to remove file ... .exe", that is him: say so and wait.
- **Downsampled screenshots lie about 1-bit fill.** An "unhighlighted"
  active button turned out to be 88% white when the pixels were actually
  measured. Crop and sample before reporting a rendering bug.

## Tone

Billy gushes when happy and says "kooky" when brilliant. He found every
audio bug by ear and described each one accurately before the mechanism
was known. Trust his ears, translate his words, keep the mathematics
honest, and the collaboration runs itself.
