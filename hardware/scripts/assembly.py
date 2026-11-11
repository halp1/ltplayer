"""Build a 3D assembly of the board and display for the enclosure CAD.

Usage (cadquery is not in KiCad's Python, so run it through uv):
    uv run --python 3.12 --with cadquery python scripts/assembly.py

Inputs, all in 3d/ and route/:
    3d/ltplayer-board.step     KiCad export, origin at the board centre
    3d/adafruit-4311.step      Adafruit's model of the display breakout (MIT),
                               downloaded on first run
    route/parts3d.json         from parts3d.py: bodies of parts with no model

Output: 3d/ltplayer-assembly.step. Same origin as the DXF: board centre, +X
right, +Y up looking at the display, +Z toward the user. The board's back
face is at Z = 0.

Parts with no model in the KiCad library get plain blocks named
"placeholder", sized from their footprint body and datasheet height. The
display sits on M2.5 standoffs. Their length is the shortest stock size that
keeps every display part clear of every board part.
"""
import json
import os
import urllib.request

import cadquery as cq

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
D3 = os.path.join(HERE, "3d")
info = json.load(open(os.path.join(HERE, "route", "parts3d.json")))
T = info["thickness"]

# Heights above the board face, mm. From datasheets where one exists.
HEIGHT = {
    "SKSC": 1.25,   # Alps SKSCLBE010: 3.5 x 3.55 x 1.25
    "DSBGA": 0.5,   # TI YBH, 0.5 max seated height
    "J402": 6.6,    # PJ-4450: no published height. Estimate, see README
}
STANDOFFS = [2.0, 2.5, 3.0, 3.5, 4.0, 5.0, 6.0, 8.0, 10.0]
GAP = 0.3  # minimum air between display parts and board parts

DISPLAY_URL = ("https://raw.githubusercontent.com/adafruit/Adafruit_CAD_Parts/main/"
               "4311%202in%20TFT%20IPS%20Display/4311%202in%20TFT%20IPS%20Display.step")
if not os.path.exists(os.path.join(D3, "adafruit-4311.step")):
    urllib.request.urlretrieve(DISPLAY_URL, os.path.join(D3, "adafruit-4311.step"))

board = cq.importers.importStep(os.path.join(D3, "ltplayer-board.step"))
board_solids = board.solids().vals()
top = max(s.BoundingBox().zmax for s in board_solids if s.BoundingBox().xlen > 60)
# KiCad leaves copper and mask out of the body, so the top face sits a little
# under the nominal thickness. Parts are mounted on the nominal face.
top = max(top, T)


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
        # body plus a round nose on the plug axis at mid height
        body = block(p["x"], p["y"], top, h)
        cx = (p["x"][0] + p["x"][1]) / 2
        nose = (cq.Workplane("XZ").center(cx, top + h / 2).circle(3.0)
                .extrude(-1.2).translate((0, p["y"][1], 0)))
        shape = body.union(nose)
    else:
        shape = block(p["x"], p["y"], top, h)
    placeholders[ref] = (p["value"], shape)

# Display: Adafruit's model has the glass on -Z and its parts on +Z, with the
# M2.5 holes 2.54 mm in from the top corners. Turn it over about Y so the
# glass faces the user, then put the hole pair on H105/H106.
disp = cq.importers.importStep(os.path.join(D3, "adafruit-4311.step"))
disp = disp.rotate((0, 0, 0), (0, 1, 0), 180)
hx = (info["holes"]["H105"][0] + info["holes"]["H106"][0]) / 2
hy = info["holes"]["H105"][1]
# after the turn, the model spans x -35.56..0 and the holes sit at x -17.78
disp = disp.translate((hx + 17.78, hy - (59.055 - 2.54), 0))


def clear(lift):
    d = disp.translate((0, 0, lift))
    dsol = d.solids().vals()
    others = board_solids + [s for _, w in placeholders.values() for s in w.solids().vals()]
    for a in dsol:
        ab = a.BoundingBox()
        for b in others:
            bb = b.BoundingBox()
            if bb.xlen > 60:
                continue  # the board itself, checked by the standoff height
            if (ab.xmin < bb.xmax and bb.xmin < ab.xmax and ab.ymin < bb.ymax and bb.ymin < ab.ymax
                    and ab.zmin < bb.zmax + GAP and bb.zmin < ab.zmax + GAP):
                if a.intersect(b).Volume() > 1e-6 or a.distance(b) < GAP:
                    return False
    return True


# lift = standoff length: the display PCB's back face (z = -1.57 after the
# turn) lands on top of the standoff.
standoff = None
for s in STANDOFFS:
    lift = top + s + 1.57
    if disp.val().BoundingBox().zmin + lift < top + GAP:
        continue
    if clear(lift):
        standoff = s
        break
if standoff is None:
    raise SystemExit("no stock standoff clears the parts under the display")
lift = top + standoff + 1.57
disp = disp.translate((0, 0, lift))

posts = []
for ref in ("H105", "H106"):
    x, y = info["holes"][ref]
    posts.append((ref, cq.Workplane("XY").polygon(6, 5.0 / 0.866).extrude(standoff).translate((x, y, top))))

asm = cq.Assembly(name="ltplayer")
asm.add(board, name="board", color=cq.Color(0.2, 0.35, 0.2))
asm.add(disp, name="display Adafruit 4311", color=cq.Color(0.15, 0.15, 0.2))
for ref, w in posts:
    asm.add(w, name=f"standoff {ref} M2.5 x {standoff:g}", color=cq.Color(0.8, 0.7, 0.4))
for ref, (value, w) in placeholders.items():
    asm.add(w, name=f"{ref} {value} placeholder", color=cq.Color(0.9, 0.4, 0.1))
out = os.path.join(D3, "ltplayer-assembly.step")
asm.export(out)

whole = asm.toCompound().BoundingBox()
glass = disp.val().BoundingBox().zmax
print(f"standoff M2.5 x {standoff:g} mm, display glass top at Z {glass:.2f}")
print(f"assembly Z {whole.zmin:.2f} .. {whole.zmax:.2f}, X {whole.xmin:.2f} .. {whole.xmax:.2f}, Y {whole.ymin:.2f} .. {whole.ymax:.2f}")
print("wrote", out)
