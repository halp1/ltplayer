#!/bin/sh
# Rebuild the whole board from the schematic: place, route, finish, export.
# Usage: scripts/route.sh
# Needs Java and Freerouting 2.5 (FREEROUTING, default below). Close the board
# in KiCad first, this overwrites ltplayer.kicad_pcb.
set -e
cd "$(dirname "$0")/.."
FREEROUTING=${FREEROUTING:-$HOME/code/pkg/freerouting/freerouting-2.5.0.jar}
PY=~/code/pkg/mcp/kicad/venv/bin/python
KICAD_CLI=/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli
if ls ./*.lck >/dev/null 2>&1; then echo "close the project in KiCad first"; exit 1; fi
python3 scripts/localize_fp.py
scripts/prep.sh
rm -f route/sig.ses
java -jar "$FREEROUTING" -de route/sig.dsn -do route/sig.ses --gui.enabled=false \
  --router.autorouter.max_passes=120 -mt 8 > route/freerouting.log 2>&1
grep -E "Auto-routing stage completed" route/freerouting.log | sed -E 's/.*final score[^(]*\(([^)]*)\).*/router: \1/'
scripts/finish.sh route/placed.kicad_pcb route/planes.kicad_pcb route/sig.ses
$PY scripts/mech.py ltplayer.kicad_pcb docs 2>&1 | grep -v -E "stdpbase|Debug|memory leak" || true
for s in top bottom; do
  $KICAD_CLI pcb render --side $s --quality high --width 1400 --height 2000 --background opaque -o docs/board-$s.png ltplayer.kicad_pcb >/dev/null
done
$KICAD_CLI sch export pdf -o ltplayer-schematic.pdf ltplayer.kicad_sch >/dev/null
