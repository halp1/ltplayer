"""Place the remaining small parts next to the pin they serve.

Each unplaced part is pulled toward one pad of an already placed part that
shares a signal net with it (rails are ignored unless listed in OWNER). It is
dropped on the nearest free grid point whose courtyard clears everything on
that side, clears through-hole parts and the display standoffs on both sides,
stays off the rounded corners and keeps an edge margin.
"""
import math
import os
import sys
import pcbnew

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from geom import ISLAND_1V8

BOARD = sys.argv[1]
b = pcbnew.LoadBoard(BOARD)
MM = pcbnew.FromMM
CX, CY = 131.0, 97.5
X0, Y0, X1, Y1 = CX - 30.8, CY - 47.3, CX + 30.8, CY + 47.3
EDGE = 1.3
GAP = 0.5
RAILS = {"GND", "+3V3", "+1V8", "+1V8A", "VSYS", "VBUS", "VBAT", "HP_GND"}

# Parts whose useful nets are all rails: (owner ref, owner pad or None).
OWNER = {
    "C101": ("J101", "A4"), "C102": ("U102", "3"), "C103": ("Q101", "2"),
    "C104": ("U103", "1"), "C105": ("U103", "5"),
    "C106": ("U104", "1"), "C107": ("U104", "5"), "C108": ("U105", "1"), "C109": ("U105", "5"),
    "C203": ("U201", "69"), "C204": ("U201", "17"), "C205": ("U201", "38"), "C206": ("U201", "49"),
    "C207": ("U201", "59"), "C208": ("U201", "73"), "C209": ("U201", "84"), "C210": ("U201", "95"),
    "C211": ("U201", "108"), "C212": ("U201", "121"), "C213": ("U201", "130"), "C214": ("U201", "32"),
    "C301": ("J301", "4"), "C302": ("J301", "4"), "C303": ("J302", "18"),
    "C409": ("U401", "31"), "C410": ("U401", "4"), "C411": ("U401", "7"), "C412": ("U401", "25"),
    "C413": ("U401", "26"), "C414": ("U401", "26"),
    "C429": ("U402", "31"), "C430": ("U402", "4"), "C431": ("U402", "7"), "C432": ("U402", "25"),
    "C433": ("U402", "26"), "C434": ("U402", "26"),
    "C441": ("Y401", "4"), "C442": ("U404", "3"), "C443": ("U404", "7"),
    "R448": ("U401", "28"), "R106": ("SW101", "2"), "R105": ("Q101", "1"), "R104": ("U102", "4"),
}


def rail_ok(net, x, y, margin=0.9):
    """A +1V8 pad must sit over the 1.8 V island on In2, a +3V3 pad off it."""
    x0, y0, x1, y1 = ISLAND_1V8
    if net == "+1V8":
        return x0 + margin <= x <= x1 - margin and y0 + margin <= y <= y1 - margin
    if net == "+3V3":
        return not (x0 - margin < x < x1 + margin and y0 - margin < y < y1 + margin)
    return True


def courtyard(fp):
    box = fp.GetCourtyard(pcbnew.B_CrtYd if fp.IsFlipped() else pcbnew.F_CrtYd).BBox()
    if box.GetWidth() == 0:
        box = fp.GetBoundingBox(False)
    return (pcbnew.ToMM(box.GetLeft()), pcbnew.ToMM(box.GetTop()), pcbnew.ToMM(box.GetRight()), pcbnew.ToMM(box.GetBottom()))


def hit(a, c, gap):
    return not (a[2] + gap <= c[0] or c[2] + gap <= a[0] or a[3] + gap <= c[1] or c[3] + gap <= a[1])


fps = {f.GetReference(): f for f in b.GetFootprints()}


def is_placed(f):
    p = f.GetPosition()
    return X0 - 5 < pcbnew.ToMM(p.x) < X1 + 5 and Y0 - 5 < pcbnew.ToMM(p.y) < Y1 + 5


placed = {r for r, f in fps.items() if is_placed(f)}
boxes = {r: courtyard(fps[r]) for r in placed}
back = {r: fps[r].IsFlipped() for r in placed}
both_sides = {r for r, f in fps.items() if any(p.GetAttribute() != pcbnew.PAD_ATTRIB_SMD for p in f.Pads())}
todo = sorted(r for r in fps if r not in placed)


def target(ref):
    if ref in OWNER:
        owner, pad = OWNER[ref]
        if owner in placed:
            p = fps[owner].FindPadByNumber(pad) if pad else None
            return (p.GetPosition() if p else fps[owner].GetPosition()), owner
    rails = {p.GetNetname() for p in fps[ref].Pads()} & {"+1V8", "+3V3"}
    best = None
    for pad in fps[ref].Pads():
        n = pad.GetNetname()
        if not n or n in RAILS:
            continue
        for r in placed:
            for q in fps[r].Pads():
                if q.GetNetname() != n:
                    continue
                qp = q.GetPosition()
                # stay on the right side of the In2 split, then prefer ICs and
                # connectors over other passives
                wrong = any(not rail_ok(rail, pcbnew.ToMM(qp.x), pcbnew.ToMM(qp.y), 0.0) for rail in rails)
                key = (wrong, 0 if r[0] in "UJQYS" else 1)
                if best is None or key < best[0]:
                    best = (key, qp, r)
    if best:
        return best[1], best[2]
    return None, None


def try_place(ref):
    pos, anchor = target(ref)
    if pos is None:
        return False, "no anchor"
    fp = fps[ref]
    ax, ay = pcbnew.ToMM(pos.x), pcbnew.ToMM(pos.y)
    if back.get(anchor, False) != fp.IsFlipped():
        fp.Flip(fp.GetPosition(), pcbnew.FLIP_DIRECTION_LEFT_RIGHT)
    flipped = fp.IsFlipped()
    for radius in [x * 0.4 for x in range(2, 90)]:
        steps = max(12, int(radius * 8))
        for k in range(steps):
            ang = 2 * math.pi * k / steps
            x = round((ax + radius * math.cos(ang)) * 4) / 4
            y = round((ay + radius * math.sin(ang)) * 4) / 4
            for rot in (0, 90):
                fp.SetOrientationDegrees(rot)
                fp.SetPosition(pcbnew.VECTOR2I(MM(x), MM(y)))
                bb = courtyard(fp)
                if bb[0] < X0 + EDGE or bb[2] > X1 - EDGE or bb[1] < Y0 + EDGE or bb[3] > Y1 - EDGE:
                    continue
                if min(bb[0] - X0, X1 - bb[2]) < 7.5 and min(bb[1] - Y0, Y1 - bb[3]) < 7.5:
                    continue
                if not all(rail_ok(p.GetNetname(), pcbnew.ToMM(p.GetPosition().x), pcbnew.ToMM(p.GetPosition().y)) for p in fp.Pads()):
                    continue
                clash = False
                for r, o in boxes.items():
                    same = back[r] == flipped or r in both_sides
                    if same and hit(bb, o, GAP if r[0] in "UJSHQY" else 0.35):
                        clash = True
                        break
                if clash:
                    continue
                boxes[ref] = bb
                back[ref] = flipped
                placed.add(ref)
                return True, anchor
    return False, f"no room near {anchor}"


report = []
pending = list(todo)
for _ in range(3):
    still = []
    for ref in pending:
        ok, why = try_place(ref)
        if not ok:
            still.append((ref, why))
    if not still or len(still) == len(pending):
        report = still
        break
    pending = [r for r, _ in still]
    report = still

pcbnew.SaveBoard(BOARD, b)
print(f"placed {len(todo) - len(report)} of {len(todo)}")
for ref, why in report:
    print("  ", ref, why)
