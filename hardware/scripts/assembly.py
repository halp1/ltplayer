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
folded flat behind the panel, dropping through the board slot and running
along the back to J302. The battery is a plain block on the back. The front shell holds the panel, so there are no
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
from geom import (PANEL, ACTIVE, PANEL_T, PANEL_GAP, CABLE_Y, CABLE_W, SLOT, J302_X, WIDE_END, FOLD_STRIP,  # noqa: E402
                  BATT, BATT_T, BATT_GAP)

D3 = os.path.join(HERE, "3d")
info = json.load(open(os.path.join(HERE, "route", "parts3d.json")))
T = info["thickness"]

# Heights above the board face, mm. From datasheets where one exists.
HEIGHT = {
    "SKSC": 1.25,   # Alps SKSCLBE010: 3.5 x 3.55 x 1.25
    "WSON": 0.8,    # TI DSG, 0.8 max
    "OSC": 0.8,     # 3.2 x 2.5 oscillator, 0.8 typical
    "J402": 6.6,    # PJ-4450: no published height. Estimate, see README
    "U501": 2.4,    # ESP32-MINI-1U: 13.2 x 13.5 x 2.4
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
    elif ref == "U501":
        h = HEIGHT["U501"]
    elif ref.startswith("U4"):
        h = HEIGHT["WSON"]
    elif ref == "Y401":
        h = HEIGHT["OSC"]
    else:
        h = HEIGHT[ref]
    if ref == "J402":
        z0 = -h if p["side"] == "back" else top
        body = block(p["x"], p["y"], z0, h)
        cx = (p["x"][0] + p["x"][1]) / 2
        nose = (cq.Workplane("XZ").center(cx, z0 + h / 2).circle(3.0)
                .extrude(-1.2).translate((0, p["y"][1], 0)))
        shape = body.union(nose)
    elif p["side"] == "back":
        shape = block(p["x"], p["y"], -h, h)
    else:
        shape = block(p["x"], p["y"], top, h)
    placeholders[ref] = (p["value"], shape)

# Panel height: PANEL_GAP above the tallest part whose outline it covers, and
# the folded 39 mm cable part (0.8 mm) plus PANEL_GAP above the tallest part
# in the fold strip.
x0, y0, x1, y1 = PANEL
under = [s for s in board_solids if s.BoundingBox().xlen < 60]
under += [s for _, w in placeholders.values() for s in w.solids().vals()]
# Through-hole pins of the back-mounted jacks poke out of the front. KiCad's
# models show them at full length (about 1.6 mm past the face). They are cut
# to PIN_TAIL after soldering, so the panel clears them at that height.
PIN_TAIL = 1.0
JACK_BACK = [s.BoundingBox() for s in under if s.BoundingBox().zmin < -4.0 and s.BoundingBox().zmax > T]


def front_top(bb):
    for j in JACK_BACK:
        if abs(bb.xmin - j.xmin) < 0.01 and abs(bb.ymax - j.ymax) < 0.01:
            return T + PIN_TAIL
    return bb.zmax


def tallest_in(r):
    hits = [(front_top(s.BoundingBox()), s.BoundingBox()) for s in under
            if s.BoundingBox().xmax > r[0] and s.BoundingBox().xmin < r[2]
            and s.BoundingBox().ymax > r[1] and s.BoundingBox().ymin < r[3]]
    return max(hits, key=lambda t: t[0]) if hits else (top, None)


tallest = tallest_in(PANEL)
fold_top = tallest_in(FOLD_STRIP)
PANEL_BACK_Z = round(max(tallest[0] + PANEL_GAP, fold_top[0] + 0.8 + PANEL_GAP), 2)

# Panel: backlight and glass block, then the picture area as a 0.05 mm skin.
panel = block((x0, x1), (y0, y1), PANEL_BACK_Z, PANEL_T)
ax0, ay0, ax1, ay1 = ACTIVE
picture = block((ax0, ax1), (ay0, ay1), PANEL_BACK_Z + PANEL_T, 0.05)
# Cable, folded flat behind the panel as Newhaven intends: the 39 mm wide
# shielded part (0.8 mm with its parts) runs from the bend at the panel edge
# to WIDE_END, the 20.5 mm tail carries on to the slot, drops through it and
# runs along the back into J302. Contacts end up facing the board.
ct = 0.3
y0c, y1c = CABLE_Y - CABLE_W / 2, CABLE_Y + CABLE_W / 2
y0w, y1w = CABLE_Y - 19.5, CABLE_Y + 19.5
fold = x0 - 0.8
sx = (SLOT[0] + SLOT[2]) / 2
wz = PANEL_BACK_Z - 0.8
cable = (block((fold, x0), (y0w, y1w), wz, 0.8 + PANEL_T / 2)                  # bend at the edge
         .union(block((fold, WIDE_END), (y0w, y1w), wz, 0.8))                   # wide part behind the panel
         .union(block((WIDE_END, sx + ct / 2), (y0c, y1c), wz + 0.5, ct))       # tail to the slot
         .union(block((sx - ct / 2, sx + ct / 2), (y0c, y1c), -0.9, wz + 0.5 + ct + 0.9))  # through the slot
         .union(block((sx - ct / 2, J302_X - 1.0), (y0c, y1c), -0.9, ct)))      # into J302

# Battery on the back, BATT_GAP below the deepest back part over its outline.
bx0, by0, bx1, by1 = BATT
over = [s.BoundingBox() for s in under]
clash = [b for b in over if b.zmin < -2.6 and b.xmax > bx0 and b.xmin < bx1 and b.ymax > by0 and b.ymin < by1]
if clash:
    raise SystemExit("battery outline overlaps a deep back part at x %.1f..%.1f y %.1f..%.1f"
                     % (clash[0].xmin, clash[0].xmax, clash[0].ymin, clash[0].ymax))
deepest = min([b.zmin for b in over if b.xmax > bx0 and b.xmin < bx1 and b.ymax > by0 and b.ymin < by1] + [0.0])
BATT_TOP_Z = round(deepest - BATT_GAP, 2)
battery = block((bx0, bx1), (by0, by1), BATT_TOP_Z - BATT_T, BATT_T)

asm = cq.Assembly(name="ltplayer")
asm.add(board, name="board", color=cq.Color(0.2, 0.35, 0.2))
asm.add(panel, name="display YX24009A", color=cq.Color(0.15, 0.15, 0.18))
asm.add(picture, name="display picture area", color=cq.Color(0.3, 0.45, 0.7))
asm.add(cable, name="display cable", color=cq.Color(0.85, 0.6, 0.2))
asm.add(battery, name="battery 506060 LiPo", color=cq.Color(0.75, 0.75, 0.8))
for ref, (value, w) in placeholders.items():
    asm.add(w, name=f"{ref} {value} placeholder", color=cq.Color(0.9, 0.4, 0.1))
out = os.path.join(D3, "ltplayer-assembly.step")
asm.export(out)

whole = asm.toCompound().BoundingBox()
tb = tallest[1]
print(f"fold strip tallest {fold_top[0]:.2f}")
print(f"panel back Z {PANEL_BACK_Z:.2f}, glass Z {PANEL_BACK_Z + PANEL_T:.2f}, set by the part at "
      f"x {tb.xmin:.1f}..{tb.xmax:.1f} y {tb.ymin:.1f}..{tb.ymax:.1f} (top {tb.zmax:.2f})")
print(f"battery Z {BATT_TOP_Z - BATT_T:.2f} .. {BATT_TOP_Z:.2f}")
print(f"assembly Z {whole.zmin:.2f} .. {whole.zmax:.2f}, X {whole.xmin:.2f} .. {whole.xmax:.2f}, "
      f"Y {whole.ymin:.2f} .. {whole.ymax:.2f}")
print("wrote", out)
