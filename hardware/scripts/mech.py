"""Export the mechanical interface for the enclosure CAD.

Usage: mech.py board.kicad_pcb out_dir

Writes ltplayer-mech.dxf and ltplayer-mech.csv. Both use the enclosure
convention: origin at the board centre, +X right and +Y up looking at the
front (display side), millimetres. The DXF has one layer per feature kind:
OUTLINE (board edge), HOLES (every drilled hole at true size), PARTS (body
outlines of parts the enclosure has to clear or expose) and DISPLAY (panel
outline and picture area).
"""
import csv
import math
import os
import sys
import pcbnew

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from geom import CX, CY, PANEL, ACTIVE

T = pcbnew.ToMM
b = pcbnew.LoadBoard(sys.argv[1])
OUT = sys.argv[2]
os.makedirs(OUT, exist_ok=True)


def to_enc(x, y):
    return round(x - CX, 3), round(CY - y, 3)


ents = []  # (layer, kind, data)
for d in b.GetDrawings():
    if d.GetLayer() not in (pcbnew.Edge_Cuts, pcbnew.User_1):
        continue
    layer = "OUTLINE" if d.GetLayer() == pcbnew.Edge_Cuts else "DISPLAY"
    if d.GetShape() == pcbnew.SHAPE_T_SEGMENT:
        ents.append((layer, "LINE", to_enc(T(d.GetStart().x), T(d.GetStart().y)) + to_enc(T(d.GetEnd().x), T(d.GetEnd().y))))
    elif d.GetShape() == pcbnew.SHAPE_T_ARC:
        c = d.GetCenter()
        cx, cy = to_enc(T(c.x), T(c.y))
        r = T(d.GetRadius())
        sx, sy = to_enc(T(d.GetStart().x), T(d.GetStart().y))
        ex, ey = to_enc(T(d.GetEnd().x), T(d.GetEnd().y))
        a0 = math.degrees(math.atan2(sy - cy, sx - cx))
        a1 = math.degrees(math.atan2(ey - cy, ex - cx))
        # Y is flipped, so KiCad's clockwise arc runs counter-clockwise here.
        # DXF arcs run counter-clockwise from start to end angle.
        sweep = (a1 - a0) % 360
        if sweep > 180:
            a0, a1 = a1, a0
        ents.append((layer, "ARC", (cx, cy, r, a0 % 360, a1 % 360)))

rows = []
for fp in sorted(b.GetFootprints(), key=lambda f: f.GetReference()):
    ref = fp.GetReference()
    side = "back" if fp.IsFlipped() else "front"
    for p in fp.Pads():
        if p.GetDrillSize().x == 0:
            continue
        x, y = to_enc(T(p.GetPosition().x), T(p.GetPosition().y))
        dx, dy = T(p.GetDrillSize().x), T(p.GetDrillSize().y)
        plated = p.GetAttribute() != pcbnew.PAD_ATTRIB_NPTH
        if plated and dx < 0.5:
            continue  # thermal and stitching vias inside a footprint
        if dx == dy:
            ents.append(("HOLES", "CIRCLE", (x, y, dx / 2)))
        else:
            # slot: two half circles and two lines along the long axis
            vert = (dy > dx) != (round(fp.GetOrientationDegrees()) % 180 == 90)
            w, l = min(dx, dy), max(dx, dy)
            r, h = w / 2, (l - w) / 2
            ax, ay = (0, h) if vert else (h, 0)
            ents.append(("HOLES", "CIRCLE", (x + ax, y + ay, r)))
            ents.append(("HOLES", "CIRCLE", (x - ax, y - ay, r)))
        if ref.startswith("H"):
            rows.append([ref, fp.GetValue(), "hole", side, x, y, round(dx, 2), "plated" if plated else "npth"])

PARTS = ["J401", "J402", "J101", "J301", "SW101", "SW205", "SW206", "SW201", "SW202", "SW203", "SW204",
         "SW207", "SW208", "SW209", "J102", "J103", "J302", "D101", "U501"]
for ref in PARTS:
    fp = b.FindFootprintByReference(ref)
    bb = fp.GetCourtyard(pcbnew.B_CrtYd if fp.IsFlipped() else pcbnew.F_CrtYd).BBox()
    x0, y1 = to_enc(T(bb.GetLeft()), T(bb.GetTop()))
    x1, y0 = to_enc(T(bb.GetRight()), T(bb.GetBottom()))
    for a, c in (((x0, y0), (x1, y0)), ((x1, y0), (x1, y1)), ((x1, y1), (x0, y1)), ((x0, y1), (x0, y0))):
        ents.append(("PARTS", "LINE", a + c))
    x, y = to_enc(T(fp.GetPosition().x), T(fp.GetPosition().y))
    rows.append([ref, fp.GetValue(), "part", "back" if fp.IsFlipped() else "front", x, y, "", f"courtyard x {x0}..{x1} y {y0}..{y1}"])


for name, (x0, y0, x1, y1) in (("panel", PANEL), ("picture area", ACTIVE)):
    rows.append([name, "NHD-2.4-240320AF-CSXP", "display", "front", round((x0 + x1) / 2, 3), round((y0 + y1) / 2, 3), "",
                 f"x {x0:.2f}..{x1:.2f} y {y0:.2f}..{y1:.2f}"])


def dxf(path):
    out = ["0", "SECTION", "2", "HEADER", "9", "$INSUNITS", "70", "4", "0", "ENDSEC",
           "0", "SECTION", "2", "TABLES", "0", "TABLE", "2", "LAYER", "70", "4"]
    colors = {"OUTLINE": 7, "HOLES": 1, "PARTS": 3, "DISPLAY": 5}
    for name, col in colors.items():
        out += ["0", "LAYER", "2", name, "70", "0", "62", str(col), "6", "CONTINUOUS"]
    out += ["0", "ENDTAB", "0", "ENDSEC", "0", "SECTION", "2", "ENTITIES"]
    for layer, kind, d in ents:
        if kind == "LINE":
            out += ["0", "LINE", "8", layer, "10", f"{d[0]}", "20", f"{d[1]}", "11", f"{d[2]}", "21", f"{d[3]}"]
        elif kind == "CIRCLE":
            out += ["0", "CIRCLE", "8", layer, "10", f"{d[0]}", "20", f"{d[1]}", "40", f"{d[2]}"]
        elif kind == "ARC":
            out += ["0", "ARC", "8", layer, "10", f"{d[0]}", "20", f"{d[1]}", "40", f"{d[2]}", "50", f"{d[3]}", "51", f"{d[4]}"]
    out += ["0", "ENDSEC", "0", "EOF"]
    open(path, "w").write("\n".join(out) + "\n")


dxf(os.path.join(OUT, "ltplayer-mech.dxf"))
with open(os.path.join(OUT, "ltplayer-mech.csv"), "w", newline="") as f:
    w = csv.writer(f)
    w.writerow(["ref", "value", "kind", "side", "x_mm", "y_mm", "drill_mm", "notes"])
    w.writerows(rows)
print(f"mech: {len(ents)} dxf entities, {len(rows)} rows")
