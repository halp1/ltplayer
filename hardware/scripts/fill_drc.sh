#!/bin/sh
# Refill zones, then run DRC with parity. Usage: fill_drc.sh board.kicad_pcb out.json
set -e
~/code/pkg/mcp/kicad/venv/bin/python - "$1" <<'PY' 2>&1 | grep -v -E "stdpbase|Debug" || true
import sys, pcbnew
b = pcbnew.LoadBoard(sys.argv[1])
pcbnew.ZONE_FILLER(b).Fill(b.Zones())
pcbnew.SaveBoard(sys.argv[1], b)
PY
/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli pcb drc --schematic-parity --format json -o "$2" "$1" >/dev/null 2>&1
python3 - "$2" <<'PY'
import json, sys, collections
d = json.load(open(sys.argv[1]))
c = collections.Counter(v["type"] for v in d["violations"])
print("violations", sum(c.values()), dict(c.most_common(14)))
print("unconnected", len(d["unconnected_items"]), "parity", len(d.get("schematic_parity", [])))
PY
