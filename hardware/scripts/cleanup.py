"""Delete router leftovers DRC reports as dangling: vias and track stubs.

Usage: cleanup.py board.kicad_pcb drc.json
Plane and trunk net stubs are never touched. Repeat until DRC reports none.
"""
import json
import os
import sys
import pcbnew

MM = pcbnew.FromMM
BOARD, REPORT = sys.argv[1], sys.argv[2]
b = pcbnew.LoadBoard(BOARD)
d = json.load(open(REPORT))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from geom import PLANES, TRUNK

KEEP = set(PLANES) | set(TRUNK)  # their stubs are meant to end in a via
spots = []
for v in d["violations"]:
    if v["type"] in ("via_dangling", "track_dangling"):
        for it in v["items"]:
            spots.append((v["type"], it["uuid"]))
want = {u for _, u in spots}
gone = 0
for t in list(b.GetTracks()):
    if t.m_Uuid.AsString() in want and t.GetNetname() not in KEEP:
        b.Remove(t)
        gone += 1
pcbnew.SaveBoard(BOARD, b)
print("removed", gone, "dangling items")
