# Font notice

Every font in this folder has its licence beside it in `<font>/LICENSE.txt`. That
is the gate: **no font enters `assets/fonts/` without its licence file.** It is the
one check that would have caught Xilla.

## The roster

| role | font | licence | attribution |
|---|---|---|---|
| UI chrome, labels, marginalia | Pixeloid Mono | SIL OFL 1.1 | no |
| fallback + the entity's voice | Unifont Ex Mono | SIL OFL 1.1 | no |
| Overworked Acoustic Engineer | Modern DOS | CC0 1.0 | no |
| Archivist Historian | PixAntiqua | SIL OFL 1.1 | no |
| Government Auditor | European Teletext | CC0 1.0 | no |
| Field Psychologist | Pixeloid Sans | SIL OFL 1.1 | no |
| Cosmic Acoustician | Pixel Operator | CC0 1.0 | no |
| Survivor | Better VCR | Open Font License | no |
| Junior Technician | Enter Command | **CC BY 4.0** | **yes** |
| Shell-Listening Monk | Scriptorium | **CC BY 4.0** | **yes** |
| Future Witness | Cyborg Sister | **CC BY 4.0** | **yes** |
| MycelliAI | Alkhemikal | **CC BY 4.0** | **yes** |

## Minimum required credit

Four fonts are CC BY 4.0 and **must** be credited. This is the smallest wording
that discharges the obligation — it belongs in the release README and in any
in-app credits:

```
Fonts: Enter Command, Scriptorium, Cyborg Sister and Alkhemikal by jeti
(fontenddev.com), licensed CC BY 4.0. Pixeloid Sans and Pixeloid Mono by
GGBotNet, SIL OFL 1.1. PixAntiqua by Gerhard Grossmann, SIL OFL 1.1.
Unifont Ex Mono by stgiga, built on GNU Unifont, SIL OFL 1.1. Better VCR by
artdzyk, Open Font License. Pixel Operator, Modern DOS and European Teletext by
Jayvee Enaguas, CC0 1.0.
```

That is the lazy version and it is sufficient. Do not ship less than it.

## The over-the-top version

Billy asked for both, so: the credits should be **set in the fonts they credit.**

Each line is typeset in the face it names. The Junior Technician's credit appears
in Enter Command, the Monk's in Scriptorium, the entity's in Unifont — so the
credits page is simultaneously a specimen sheet and a cast list. It reads as the
instrument introducing its own voices, and it is impossible to fake, because a
missing font shows immediately as a fallback.

Fuller sketch, in the register of the rest of the instrument:

- Present it as a **relic entry of its own** — a thirteenth character whose log
  happens to be the credits, appearing in the voice box like any other. `id` would
  be the next Fibonacci number. The entity's intrusion lines could be the CC0
  fonts, which owe nothing and therefore say nothing.
- Or a dedicated **CREDITS panel** in the WoH idiom: an inverted strip header, one
  row per font, each row showing the font's own name in its own face at its own
  native pixel size (8 px for Modern DOS, 9 px for the Pixeloids, 16 px for the
  rest), with the licence in the UI face beside it. The size differences become the
  design rather than a problem to normalise.
- The four attribution-required fonts get a marker — a small filled square, say —
  so the *required* credit is visually distinct from the courtesy ones. Honest, and
  it makes the obligation legible rather than buried.

Whichever, the rule holds: a font's own name is always shown in that font.

## Provenance notes

- **jeti's four** shipped no licence file and no embedded metadata. dafont's
  "Public domain / GPL / OFL" category is a self-declaration and is not evidence.
  The terms come from the author's own site, quoted and dated in each
  `LICENSE.txt`. If fontenddev.com ever goes down, those quotes are the record.
- **Unifont Ex Mono, PixAntiqua and Better VCR** declared their licence in the
  font's `name` table but shipped no file; their `LICENSE.txt` reproduces the
  declaration plus the OFL 1.1 text.
- **Better VCR** declares "Open Font License" with no version. OFL 1.1 is the only
  published version, so that is what is reproduced.
- **Rejected:** `Minitel` — no licence in the archive, none embedded, and no author
  statement found anywhere. `vhs-vcr-osd` — CC BY-SA 3.0, dropped because
  Share-Alike inside a released binary is a question for a lawyer, not for us.

## Size

`unifontexmono` is 13.7 MB and `better_vcr` 2.8 MB — together ~16.5 MB, most of the
repo. Unifont earns it: it is the only face with complete coverage of φ ρ π Δ • ◦ ¤
and the subscripts, so it is what stops every other font falling back mid-sentence.
If that is too much to carry in git, the alternative is subsetting it to the glyphs
actually used, which is a build step nobody has written yet.
