#!/bin/sh
# Build the placed board and the Freerouting job from the schematic.
# Usage: prep.sh   (then run Freerouting on route/sig.dsn, then finish.sh)
set -e
cd "$(dirname "$0")/.."
PY=~/code/pkg/mcp/kicad/venv/bin/python
KICAD_CLI=/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli
quiet() { grep -v -E "stdpbase|Debug|memory leak" || true; }
mkdir -p route
$KICAD_CLI sch export netlist -o route/ltplayer.net ltplayer.kicad_sch >/dev/null 2>&1
$PY scripts/build.py ltplayer.kicad_pcb route/ltplayer.net 2>&1 | quiet
$PY scripts/place_passives.py ltplayer.kicad_pcb 2>&1 | quiet
$PY scripts/rules.py ltplayer.kicad_pcb 2>&1 | quiet
cp ltplayer.kicad_pcb route/placed.kicad_pcb
$PY scripts/plane_vias.py ltplayer.kicad_pcb 2>&1 | quiet
cp ltplayer.kicad_pcb route/planes.kicad_pcb
$PY - <<'PY' 2>&1 | quiet
import pcbnew
b = pcbnew.LoadBoard("route/planes.kicad_pcb")
for t in b.GetTracks():
    t.SetLocked(True)
print("dsn export", pcbnew.ExportSpecctraDSN(b, "route/board.dsn"))
PY
python3 scripts/dsn2l.py route/board.dsn route/sig.dsn
