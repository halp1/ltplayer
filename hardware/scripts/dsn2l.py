"""Turn the KiCad Specctra export into a job Freerouting can finish.

Usage: dsn2l.py board.dsn sig.dsn

Freerouting does not converge with the plane nets in the job and it ignores
power-layer hints, so the board is presented as two signal layers and the
plane nets (GND, +3V3, +1V8) and the VSYS trunk lose their pins. Their pads
are tied to the inner layers by the stub vias from plane_vias.py, which stay
in the job as fixed obstacles.
"""
import re
import sys

import os

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from geom import PLANES, TRUNK, TRUNK_ROUTED

PLANES = tuple(PLANES) + tuple(TRUNK)
def drop_layer_items(text, layer):
    """Remove every (shape ...) and (keepout ...) expression drawn on a layer.

    Polygon shapes span several lines, so each one is removed as a whole
    balanced expression together with its leading whitespace.
    """
    out, pos = [], 0
    for m in re.finditer(r"\n[ \t]*\((?:shape|keepout)\b", text):
        if m.start() < pos:
            continue
        i, depth = m.end() - len(re.search(r"\((?:shape|keepout)$", m.group(0)).group(0)), 0
        j = i
        while True:
            c = text[j]
            if c == "(":
                depth += 1
            elif c == ")":
                depth -= 1
                if depth == 0:
                    break
            j += 1
        if re.search(r"\(\w+ %s[\s)]" % re.escape(layer), text[i:j + 1]):
            out.append(text[pos:m.start()])
            pos = j + 1
    out.append(text[pos:])
    return "".join(out)


src, dst = sys.argv[1], sys.argv[2]
s = open(src).read()

def keep_pins(m):
    refs = TRUNK_ROUTED.get(net, ())
    kept = [p for p in m.group(2).split() if p.split("-")[0] in refs]
    return m.group(1) + (" " + " ".join(kept) if kept else "") + ")"


for net in PLANES:
    s, n = re.subn(r'(\(net "?%s"?\s*\n\s*\(pins)([^)]*)\)' % re.escape(net), keep_pins, s)
    assert n == 1, (net, n)
s = re.sub(r"\n\s*\(plane [^\n]*(?:\n[^\n]*?)*?\)\)", "", s) if "(plane " in s else s
for layer in ("In1.Cu", "In2.Cu"):
    s, n = re.subn(r"\n    \(layer %s\n      \(type \w+\)\n      \(property\n        \(index \d\)\n      \)\n    \)" % re.escape(layer), "", s)
    assert n == 1, layer
    s = drop_layer_items(s, layer)
s = s.replace("(layer B.Cu\n      (type signal)\n      (property\n        (index 3)", "(layer B.Cu\n      (type signal)\n      (property\n        (index 1)")
s = s.replace("Via[0-3]", "Via[0-1]")

# Each pre-placed stub and its via become a private net. Left on the plane net
# the router would try to join them all to each other on the outer layers.
PLANE_RE = "|".join(re.escape(n) for n in PLANES)
names = {}


def private(key):
    return names.setdefault(key, "PLANE$%d" % len(names))


def via_net(m):
    return '%s(net "%s")(type fix))' % (m.group(1), private((m.group(2), m.group(3))))


def wire_net(m):
    return '%s(net "%s")(type fix))' % (m.group(1), private((m.group(2), m.group(3))))


s = re.sub(r'(\(via "[^"]+"\s+(-?[\d.]+) (-?[\d.]+) )\(net "?(?:%s)"?\)\(type fix\)\)' % PLANE_RE, via_net, s)
s = re.sub(r'(\(wire \(path \S+ \d+\s+-?[\d.]+ -?[\d.]+\s+(-?[\d.]+) (-?[\d.]+)\))\(net "?(?:%s)"?\)\(type fix\)\)' % PLANE_RE, wire_net, s)
decl = "".join('    (net "%s"\n      (pins)\n    )\n' % n for n in names.values())
s = s.replace("  (network\n", "  (network\n" + decl, 1)
print("private stub nets", len(names))
assert "In1.Cu" not in s and "In2.Cu" not in s
open(dst, "w").write(s)
print("wrote", dst)
