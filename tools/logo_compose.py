"""The Logalith's portrait, composed into the release logo.

    python tools/logo_compose.py <capture.ppm> <outdir>

The input is a BYPO_SHOT capture (the GUI's own screenshot tool: run the
instrument with BYPO_SHOT=45:path.ppm and a full-madness state — rip 1.0,
haunt 1.0, index 1.0, master 0.5 so the shell renders WHOLE at scale 1.0;
the panel clips anything larger and a clipped shell has a flat edge no
recrop can heal). This script cuts cell Y, finds the shell by ink density
(lone stars don't stretch the box), centres it, integer-scales it (whole
pixels — the 1-bit law), and sets it in a black disc with a thin white
ring. Outputs: logalith_1024.png (transparent master), icon.ico
(256..16, the small rungs resampled — icons are packaging, where the
no-AA law does not reach), icon_256.rgba (raw bytes the window icon
embeds, so the binary needs no image decoder).
"""

import sys
from PIL import Image, ImageDraw

CELL_Y = (534, 69, 917, 485)  # interior of the shell's cell at 1180x780

def main(src, outdir):
    im = Image.open(src).convert('L')
    cell = im.crop(CELL_Y).point(lambda p: 255 if p > 127 else 0)
    px, (w, h) = cell.load(), cell.size
    cols = [sum(1 for y in range(h) if px[x, y]) for x in range(w)]
    rows = [sum(1 for x in range(w) if px[x, y]) for y in range(h)]
    xs = [x for x, c in enumerate(cols) if c > 4]
    ys = [y for y, c in enumerate(rows) if c > 4]
    cx, cy = (min(xs) + max(xs)) // 2, (min(ys) + max(ys)) // 2
    half = (max(max(xs) - min(xs), max(ys) - min(ys)) + 16) // 2
    shell = cell.crop((cx - half, cy - half, cx + half, cy + half))

    S = 1024
    inner = S - 2 * 26
    k = max(1, round(inner / shell.width))
    shell = shell.resize((shell.width * k, shell.height * k), Image.NEAREST)

    logo = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    draw = ImageDraw.Draw(logo)
    draw.ellipse((12, 12, S - 12, S - 12), fill=(0, 0, 0, 255))
    mask = Image.new('L', (S, S), 0)
    ImageDraw.Draw(mask).ellipse((26, 26, S - 26, S - 26), fill=255)
    layer = Image.new('L', (S, S), 0)
    layer.paste(shell, ((S - shell.width) // 2, (S - shell.height) // 2))
    layer = Image.composite(layer, Image.new('L', (S, S), 0), mask)
    logo.paste(Image.new('RGBA', (S, S), (255, 255, 255, 255)), (0, 0), layer)
    draw.ellipse((12, 12, S - 12, S - 12), outline=(255, 255, 255, 255), width=12)

    logo.save(outdir + '/logalith_1024.png')
    logo.save(outdir + '/icon.ico',
              sizes=[(256, 256), (128, 128), (64, 64), (48, 48), (32, 32), (16, 16)])
    with open(outdir + '/icon_256.rgba', 'wb') as f:
        f.write(logo.resize((256, 256), Image.LANCZOS).tobytes())
    print('wrote logalith_1024.png, icon.ico, icon_256.rgba')

if __name__ == '__main__':
    main(sys.argv[1], sys.argv[2])
