-- BYPO portrait grid exporter (Aseprite script)
--
-- Exports the active sprite as a portrait grid for Space Z: one line per pixel
-- row, `#` for ink and `.` for empty. Drop the result in
-- `crates/fibonacci-gui/assets/portraits/<avatar>.txt` and the plinth picks it up
-- within two seconds, running.
--
-- Install: copy this file into Aseprite's scripts folder
--   File > Scripts > Open Scripts Folder
-- then File > Scripts > Rescan, and it appears in that menu.
--
-- Ink is the *light* pixels, because the plinth draws ink white on black. If you
-- worked dark-on-light, tick Invert.
--
-- Transparent pixels are always empty, whatever their colour — that is what lets
-- the plinth's scanlines show through behind the figure, which is the whole look.

local MAX_W, MAX_H = 96, 120

local spr = app.activeSprite
if not spr then
  app.alert("No sprite open.")
  return
end

if spr.width > MAX_W or spr.height > MAX_H then
  app.alert{
    title = "Too big",
    text = {
      ("This sprite is %dx%d."):format(spr.width, spr.height),
      ("The limit is %dx%d — the cloud is redrawn every frame, so"):format(MAX_W, MAX_H),
      "grid size is a per-frame cost.",
      "",
      "Sprite > Sprite Size, and use Nearest Neighbor.",
      "64x80 is the sweet spot for a face."
    }
  }
  return
end

local dlg = Dialog("BYPO portrait grid")
dlg:check{ id = "invert", text = "Invert (ink = dark pixels)", selected = false }
dlg:slider{ id = "threshold", label = "Threshold", min = 1, max = 254, value = 128 }
dlg:label{ label = "", text = ("%dx%d, limit %dx%d"):format(spr.width, spr.height, MAX_W, MAX_H) }
dlg:button{ id = "ok", text = "Export", focus = true }
dlg:button{ id = "cancel", text = "Cancel" }
dlg:show()
if not dlg.data.ok then return end

local invert = dlg.data.invert
local threshold = dlg.data.threshold

-- Flatten the whole sprite at the current frame, so layers and blending are
-- included rather than just the active cel.
local frame = app.activeFrame or spr.frames[1]
local flat = Image(spr.width, spr.height, ColorMode.RGB)
flat:drawSprite(spr, frame)

local pc = app.pixelColor
local rows = {}
local inked = 0

for y = 0, flat.height - 1 do
  local row = {}
  for x = 0, flat.width - 1 do
    local px = flat:getPixel(x, y)
    local a = pc.rgbaA(px)
    local ink
    if a < 128 then
      ink = false -- transparent is always empty: the scanlines show through
    else
      local lum = 0.299 * pc.rgbaR(px) + 0.587 * pc.rgbaG(px) + 0.114 * pc.rgbaB(px)
      ink = lum >= threshold
      if invert then ink = not ink end
    end
    row[#row + 1] = ink and "#" or "."
    if ink then inked = inked + 1 end
  end
  rows[#rows + 1] = table.concat(row)
end

if inked == 0 then
  app.alert("Nothing inked — try Invert, or lower the threshold.")
  return
end

-- Default the output beside the sprite, with a .txt extension.
local base = spr.filename
if base == nil or base == "" then base = "portrait" end
base = base:gsub("%.[^.\\/]+$", "")
local out = base .. ".txt"

local fdlg = Dialog("Save grid")
fdlg:file{ id = "path", label = "Write to", save = true, filename = out, filetypes = { "txt" } }
fdlg:button{ id = "ok", text = "Save", focus = true }
fdlg:button{ id = "cancel", text = "Cancel" }
fdlg:show()
if not fdlg.data.ok then return end
out = fdlg.data.path
if out == nil or out == "" then return end

local f, err = io.open(out, "w")
if not f then
  app.alert("Could not write: " .. tostring(err))
  return
end
f:write(("// %dx%d, %d points -- exported from Aseprite for BYPO Space Z\n")
  :format(flat.width, flat.height, inked))
for _, row in ipairs(rows) do
  f:write(row, "\n")
end
f:close()

app.alert{
  title = "Exported",
  text = {
    ("%dx%d, %d inked points."):format(flat.width, flat.height, inked),
    out,
    "",
    "Points are drawn white on the scanline field, so empty",
    "cells are where the stripes show through."
  }
}
