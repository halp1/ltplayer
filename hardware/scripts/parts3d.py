"""Write the geometry assembly.py needs from the board, as JSON.

Usage: parts3d.py board.kicad_pcb out.json

For each footprint without a 3D model in the KiCad library, the F.Fab body
rectangle in enclosure coordinates (origin at the board centre, +Y up, front
is +Z). Also the board thickness.
"""
import json
import os
import sys
import pcbnew

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from geom import CX, CY

T = pcbnew.ToMM
b = pcbnew.LoadBoard(sys.argv[1])
NEEDS = ("J402", "SW101", "SW205", "SW206", "U406", "U407", "U408", "U409", "Y401", "U501", "J102")


def enc(p):
    return [round(T(p.x) - CX, 3), round(CY - T(p.y), 3)]


out = {"thickness": T(b.GetDesignSettings().GetBoardThickness()), "parts": {}}
for ref in NEEDS:
    fp = b.FindFootprintByReference(ref)
    layer = pcbnew.B_Fab if fp.IsFlipped() else pcbnew.F_Fab
    rects = [g for g in fp.GraphicalItems() if isinstance(g, pcbnew.PCB_SHAPE) and g.GetLayer() == layer and g.GetShape() == pcbnew.SHAPE_T_RECT]
    if rects:
        pts = [enc(c) for c in rects[0].GetCorners()]
    else:
        # library footprints draw the body as a polygon of lines, use their extent
        pts = []
        for g in fp.GraphicalItems():
            if isinstance(g, pcbnew.PCB_SHAPE) and g.GetLayer() == layer:
                bb = g.GetBoundingBox()
                pts += [enc(pcbnew.VECTOR2I(bb.GetLeft(), bb.GetTop())), enc(pcbnew.VECTOR2I(bb.GetRight(), bb.GetBottom()))]
        if not pts:
            bb = fp.GetBoundingBox(False)
            pts = [enc(pcbnew.VECTOR2I(bb.GetLeft(), bb.GetTop())), enc(pcbnew.VECTOR2I(bb.GetRight(), bb.GetBottom()))]
    xs, ys = [p[0] for p in pts], [p[1] for p in pts]
    out["parts"][ref] = {
        "value": fp.GetValue(),
        "x": [min(xs), max(xs)], "y": [min(ys), max(ys)],
        "side": "back" if fp.IsFlipped() else "front",
        "rotation": fp.GetOrientationDegrees(),
        "origin": enc(fp.GetPosition()),
    }
json.dump(out, open(sys.argv[2], "w"), indent=1)
print("parts3d:", ", ".join(out["parts"]))
