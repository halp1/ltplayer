"""Fix tiny clearance misses between a plane stub via and a routed track.

Usage: nudge.py board.kicad_pcb drc.json
Freerouting lands tracks a few microns off its grid, so now and then a track
passes 0.12 mm from a stub via instead of 0.127. For each clearance violation
under 0.02 mm short between a via on a plane or trunk net and a track, the via
moves straight away from the track by the shortfall plus 0.01 mm, and its stub
track end follows. Bigger misses are left for DRC to report.
"""
import json
import math
import os
import sys
import pcbnew

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from geom import PLANES, TRUNK, CLEARANCE

MM = pcbnew.FromMM
T = pcbnew.ToMM
b = pcbnew.LoadBoard(sys.argv[1])
d = json.load(open(sys.argv[2]))
STUB = set(PLANES) | set(TRUNK)
by_uuid = {t.m_Uuid.AsString(): t for t in b.GetTracks()}
moved = 0
for v in d["violations"]:
    if v["type"] != "clearance":
        continue
    items = [by_uuid.get(it["uuid"]) for it in v["items"]]
    if None in items:
        continue
    via = next((t for t in items if t.Type() == pcbnew.PCB_VIA_T and t.GetNetname() in STUB), None)
    trk = next((t for t in items if t.Type() == pcbnew.PCB_TRACE_T and t is not via), None)
    if not via or not trk:
        continue
    try:
        actual = float(v["description"].split("actual ")[1].split(" mm")[0])
    except (IndexError, ValueError):
        continue
    short = CLEARANCE - actual
    if not 0 < short < 0.02:
        continue
    p = via.GetPosition()
    a, c = trk.GetStart(), trk.GetEnd()
    dx, dy = c.x - a.x, c.y - a.y
    ll = dx * dx + dy * dy or 1
    u = max(0.0, min(1.0, ((p.x - a.x) * dx + (p.y - a.y) * dy) / ll))
    qx, qy = a.x + u * dx, a.y + u * dy
    nx, ny = p.x - qx, p.y - qy
    n = math.hypot(nx, ny) or 1
    step = MM(short + 0.01)
    np_ = pcbnew.VECTOR2I(int(p.x + nx / n * step), int(p.y + ny / n * step))
    for t in b.GetTracks():
        if t.Type() == pcbnew.PCB_TRACE_T and t.GetNetCode() == via.GetNetCode():
            if t.GetStart() == p:
                t.SetStart(np_)
            if t.GetEnd() == p:
                t.SetEnd(np_)
    via.SetPosition(np_)
    moved += 1
pcbnew.SaveBoard(sys.argv[1], b)
print("nudged", moved)
