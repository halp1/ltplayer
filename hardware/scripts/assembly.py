"""Build a 3D assembly of the board and display panel for the enclosure CAD.

Usage (cadquery is not in KiCad's Python, so run it through uv):
    uv run --python 3.12 --with cadquery python scripts/assembly.py

Inputs:
    3d/ltplayer-board.step     KiCad export, origin at the board centre
    route/parts3d.json         from parts3d.py: bodies of parts with no model

Output: 3d/ltplayer-assembly.step. Same origin as the DXF: board centre, +X
right, +Y up looking at the display, +Z toward the user. The board's back
face is at Z = 0.

The display is the Newhaven NHD-2.4-240320AF-CSXP drawn from its datasheet:
the backlight and glass block at full outline, the picture area as a separate
thin solid on top so it shows where the window goes, and the cable as a strip
that leaves the left edge, drops through the board slot and runs along the
back to J302. The front shell holds the panel, so there are no
standoffs. The panel sits PANEL_GAP above the tallest part under it.

Parts with no model in the KiCad library get plain blocks named
"placeholder", sized from their footprint body and datasheet height. The
panel height is set from them, so measuring the 4.4 mm jack and updating
HEIGHT moves the panel.
"""
import json
import os
import sys

import cadquery as cq

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(HERE, "scripts"))
from geom import PANEL, ACTIVE, PANEL_T, PANEL_GAP, CABLE_Y, CABLE_W, SLOT, J302_X  # noqa: E402

D3 = os.path.join(HERE, "3d")
info = json.load(open(os.path.join(HERE, "route", "parts3d.json")))
T = info["thickness"]

# Heights above the board face, mm. From datasheets where one exists.
HEIGHT = {
    "SKSC": 1.25,   # Alps SKSCLBE010: 3.5 x 3.55 x 1.25
    "DSBGA": 0.5,   # TI YBH, 0.5 max seated height
    "J402": 6.6,    # PJ-4450: no published height. Estimate, see README
}

board = cq.importers.importStep(os.path.join(D3, "ltplayer-board.step"))
board_solids = board.solids().vals()
top = max(max(s.BoundingBox().zmax for s in board_solids if s.BoundingBox().xlen > 60), T)


def block(x, y, z0, h):
    return (cq.Workplane("XY")
            .box(x[1] - x[0], y[1] - y[0], h, centered=False)
            .translate((x[0], y[0], z0)))


placeholders = {}
for ref, p in info["parts"].items():
    if ref.startswith("SW"):
        h = HEIGHT["SKSC"]
    elif ref.startswith("U4"):
        h = HEIGHT["DSBGA"]
    else:
        h = HEIGHT[ref]
    if ref == "J402":
        body = block(p["x"], p["y"], top, h)
        cx = (p["x"][0] + p["x"][1]) / 2
        nose = (cq.Workplane("XZ").center(cx, top + h / 2).circle(3.0)
                .extrude(-1.2).translate((0, p["y"][1], 0)))
        shape = body.union(nose)
    else:
        shape = block(p["x"], p["y"], top, h)
    placeholders[ref] = (p["value"], shape)

# Panel height: PANEL_GAP above the tallest part whose outline it covers.
x0, y0, x1, y1 = PANEL
under = [s for s in board_solids if s.BoundingBox().xlen < 60]
under += [s for _, w in placeholders.values() for s in w.solids().vals()]
tallest = max(
    ((s.BoundingBox().zmax, s.BoundingBox()) for s in under
     if s.BoundingBox().xmax > x0 and s.BoundingBox().xmin < x1
     and s.BoundingBox().ymax > y0 and s.BoundingBox().ymin < y1),
    key=lambda t: t[0])
PANEL_BACK_Z = round(tallest[0] + PANEL_GAP, 2)

# Panel: backlight and glass block, then the picture area as a 0.05 mm skin.
panel = block((x0, x1), (y0, y1), PANEL_BACK_Z, PANEL_T)
ax0, ay0, ax1, ay1 = ACTIVE
picture = block((ax0, ax1), (ay0, ay1), PANEL_BACK_Z + PANEL_T, 0.05)
# Cable: 0.3 mm strip that leaves the panel's left edge, folds back under the
# panel, drops through the slot to the board's back face, then runs along
# the back into J302.
ct = 0.3
y0c, y1c = CABLE_Y - CABLE_W / 2, CABLE_Y + CABLE_W / 2
fold = x0 - 0.8
sx = (SLOT[0] + SLOT[2]) / 2
cable = (block((fold, x0), (y0c, y1c), PANEL_BACK_Z - 0.8, 0.8 + PANEL_T / 2)       # bend at the edge
         .union(block((fold, sx + ct / 2), (y0c, y1c), PANEL_BACK_Z - 0.8, ct))     # back under the panel
         .union(block((sx - ct / 2, sx + ct / 2), (y0c, y1c), -ct, PANEL_BACK_Z - 0.8 + ct))  # through the slot
         .union(block((sx - ct / 2, J302_X), (y0c, y1c), -ct, ct)))                 # along the back

asm = cq.Assembly(name="ltplayer")
asm.add(board, name="board", color=cq.Color(0.2, 0.35, 0.2))
asm.add(panel, name="display YX24009A", color=cq.Color(0.15, 0.15, 0.18))
asm.add(picture, name="display picture area", color=cq.Color(0.3, 0.45, 0.7))
asm.add(cable, name="display cable", color=cq.Color(0.85, 0.6, 0.2))
for ref, (value, w) in placeholders.items():
    asm.add(w, name=f"{ref} {value} placeholder", color=cq.Color(0.9, 0.4, 0.1))
out = os.path.join(D3, "ltplayer-assembly.step")
asm.export(out)

whole = asm.toCompound().BoundingBox()
tb = tallest[1]
print(f"panel back Z {PANEL_BACK_Z:.2f}, glass Z {PANEL_BACK_Z + PANEL_T:.2f}, set by the part at "
      f"x {tb.xmin:.1f}..{tb.xmax:.1f} y {tb.ymin:.1f}..{tb.ymax:.1f} (top {tb.zmax:.2f})")
print(f"assembly Z {whole.zmin:.2f} .. {whole.zmax:.2f}, X {whole.xmin:.2f} .. {whole.xmax:.2f}, "
      f"Y {whole.ymin:.2f} .. {whole.ymax:.2f}")
print("wrote", out)
