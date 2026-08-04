# Portrait grids for Space Z

The plinth draws a **point cloud**, not an image: a dithered 1-bit grid whose
inked cells become points, and the points are what get animated. Every point
breathes on its own phase, a scan band crosses them, and the whole cloud scatters
outward as the entity's agitation rises. A raster could only be moved as a block;
this is why the format is a grid of characters rather than a PNG.

## The format

`crates/fibonacci-gui/assets/portraits/<avatar>.txt`, named after the `avatar`
field in `relic_log.json` with `.png` swapped for `.txt` — so
`engineer_1bit.png` → `portraits/engineer_1bit.txt`.

One line per pixel row.

- **Space, `.` and `0` are empty. Every other character is ink.** Deliberately
  liberal: image-to-ASCII converters emit `#`, `@`, `*`, `X`, `1`, `%` and worse,
  and none of that should need matching.
- **Comments start with `//`** — not `#`, which is the commonest ink character
  there is.
- Ragged lines, blank padding above and below, CRLF and a UTF-8 BOM are all
  handled. Padding is trimmed so a converter's margin doesn't shrink the art.
- **Maximum 96×120; 64×80 is the sweet spot for a face.** This is a performance
  limit, not a taste one — the cloud is redrawn every frame, and 64×80 lands
  around 2,000 points, the same order as the shell's stroke count.
- **Leave the background empty rather than filling it black.** Empty cells are
  where the plinth's scanlines show through, which is what makes a figure look
  like it is standing on stripes. A filled background paints a slab over them.

Hot-reloaded every ~2 s, so you can convert and watch it land without restarting.

## Workflow (Aseprite)

Aseprite is the right tool because it lets you **fix the dither by hand**, which
no automatic converter does — and on a 64×80 face, three or four corrected pixels
around the eyes is the difference between a person and a smudge.

1. `File > Open` your licensed source image.
2. `Sprite > Sprite Size` → 64×80 (or thereabouts), interpolation **Nearest
   Neighbor**. Don't preserve aspect if a crop reads better.
3. Get it to two colours. Either `Sprite > Color Mode > Indexed` with a 2-colour
   palette and dithering, or dither it elsewhere first (see below) and bring the
   result in.
4. **Erase the background to transparent** — the eraser, or select and delete.
   This is the step that matters most for the look.
5. Touch up by hand. Eyes, jawline, anything that reads as noise.
6. `File > Scripts > bypo-portrait-grid`, then save into
   `assets/portraits/<avatar>.txt`.

Ink is the *light* pixels, since the plinth draws ink white on black. If you
worked dark-on-light, tick **Invert**. The threshold slider is there for stock
images that dither too dark or too pale.

To install the script: `File > Scripts > Open Scripts Folder`, copy
`bypo-portrait-grid.lua` in, then `File > Scripts > Rescan`.

## Alternatives

- **[Dithermark](https://app.dithermark.com/)** — free, browser, far more dither
  algorithms than Aseprite (Floyd–Steinberg, Atkinson, ordered, blue noise) with
  live preview. Best used to *choose* the dither, then bring the PNG into Aseprite
  for the background cut and the hand fixes.
- **GIMP** — `Image > Scale`, then `Image > Mode > Indexed` with 1-bit and
  Floyd–Steinberg. Free, but its pixel editing is worse than Aseprite's.
- **ImageMagick**, if you'd rather batch:
  `magick in.png -resize 64x80! -colorspace gray -ordered-dither o4x4 -monochrome txt:-`
  then reshape the coordinate dump. Fastest for eleven at once, no hand fixes.

Any of these is fine — the parser doesn't care what produced the file.
