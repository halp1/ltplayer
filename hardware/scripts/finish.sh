#!/bin/sh
# Import a routed session, restore the plane stub vias, fill zones, run DRC.
# Usage: finish.sh route/placed.kicad_pcb route/planes.kicad_pcb route/sig.ses
#   placed: placed, unrouted board
#   planes: the same board plus the stub vias from plane_vias.py
#   ses:    Freerouting result for the two-layer job made by dsn2l.py
set -e
cd "$(dirname "$0")/.."
PY=~/code/pkg/mcp/kicad/venv/bin/python
quiet() { grep -v -E "stdpbase|Debug|memory leak" || true; }
mkdir -p route
cp "$1" ltplayer.kicad_pcb
$PY - "$2" "$3" <<'PY' 2>&1 | quiet
import re, sys, pcbnew
planes_path, ses_path = sys.argv[1], sys.argv[2]

# The router saw a two-layer board and private nets for the plane stubs.
# Drop those nets from the session and restore the real via names.
ses = open(ses_path).read()
out, pos = [], 0
for m in re.finditer(r'\n\s*\(net "?PLANE\$\d+"?', ses):
    i = ses.index("(", m.start())
    depth, j = 0, i
    while True:
        depth += {"(": 1, ")": -1}.get(ses[j], 0)
        if depth == 0:
            break
        j += 1
    out.append(ses[pos:m.start()])
    pos = j + 1
out.append(ses[pos:])
ses = "".join(out).replace("Via[0-1]", "Via[0-3]")
tmp = ses_path + ".import"
open(tmp, "w").write(ses)

b = pcbnew.LoadBoard("ltplayer.kicad_pcb")
ok = pcbnew.ImportSpecctraSES(b, tmp)
for t in b.GetTracks():
    if t.Type() == pcbnew.PCB_VIA_T:
        t.SetViaType(pcbnew.VIATYPE_THROUGH)
        t.SetLayerPair(pcbnew.F_Cu, pcbnew.B_Cu)
    elif t.GetWidth() < pcbnew.FromMM(0.1):
        t.SetWidth(pcbnew.FromMM(0.1))
routed = len(b.GetTracks())

src = pcbnew.LoadBoard(planes_path)
for t in src.GetTracks():
    if t.Type() == pcbnew.PCB_VIA_T:
        n = pcbnew.PCB_VIA(b)
        n.SetPosition(t.GetPosition())
        n.SetWidth(pcbnew.F_Cu, t.GetWidth(pcbnew.F_Cu))
        n.SetDrill(t.GetDrill())
        n.SetViaType(pcbnew.VIATYPE_THROUGH)
        n.SetLayerPair(pcbnew.F_Cu, pcbnew.B_Cu)
    else:
        n = pcbnew.PCB_TRACK(b)
        n.SetStart(t.GetStart())
        n.SetEnd(t.GetEnd())
        n.SetWidth(t.GetWidth())
        n.SetLayer(t.GetLayer())
    n.SetNet(b.FindNet(t.GetNetname()))
    b.Add(n)
print("import", ok, "routed items", routed, "plane items", len(b.GetTracks()) - routed)
pcbnew.SaveBoard("ltplayer.kicad_pcb", b)
PY
$PY scripts/rules.py ltplayer.kicad_pcb 2>&1 | quiet
$PY scripts/trunk.py ltplayer.kicad_pcb > route/trunk.log 2>&1 || { quiet < route/trunk.log; echo "trunk routing failed"; exit 1; }
quiet < route/trunk.log
$PY scripts/zones.py ltplayer.kicad_pcb 2>&1 | quiet
scripts/fill_drc.sh ltplayer.kicad_pcb route/drc.json
# router leftovers: drop dangling vias and stubs until none remain
for i in 1 2 3; do
  n=$(python3 -c "import json;d=json.load(open('route/drc.json'));print(sum(v['type'] in ('via_dangling','track_dangling') for v in d['violations']))")
  [ "$n" = 0 ] && break
  $PY scripts/cleanup.py ltplayer.kicad_pcb route/drc.json 2>&1 | quiet
  scripts/fill_drc.sh ltplayer.kicad_pcb route/drc.json
done
