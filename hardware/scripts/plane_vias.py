"""Connect pads on plane nets to the inner layers with stub vias.

GND, +3V3 and +1V8 pads reach their plane through the via. VSYS pads get the
same stub and are joined later by a trunk on In2.Cu (finish_routes.py).

The DAC supply pins come first, from the table in geom.py: ground pins run in
to the exposed pad, supply pins follow a fixed track through their decoupling
capacitors to a via. Every other pad gets a via on a ring around it. A
candidate is kept only if the via and the stub clear all other-net copper,
vias, holes and the board edge, and a +1V8 or +3V3 via lands on its own side
of the In2 split. DSBGA balls get a filled via-in-pad.
"""
import math
import os
import sys
import pcbnew

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from geom import DACS, DAC_EP_PINS, DAC_ROUTES, PLANES, TRUNK, TRUNK_ROUTED, dac_point, in_island

MM = pcbnew.FromMM
b = pcbnew.LoadBoard(sys.argv[1])
STUB_NETS = set(PLANES) | set(TRUNK)
VIA_D, VIA_DRILL = 0.45, 0.25
CLR = 0.15
STUB = 0.2
F, B = pcbnew.F_Cu, pcbnew.B_Cu
outline = pcbnew.SHAPE_POLY_SET()
b.GetBoardPolygonOutlines(outline, True)
inner = pcbnew.SHAPE_POLY_SET(outline)
inner.Inflate(-MM(0.5), pcbnew.CORNER_STRATEGY_ROUND_ALL_CORNERS, MM(0.02))

fps = list(b.GetFootprints())
keepouts = [z for f in fps for z in f.Zones() if z.GetIsRuleArea() and z.GetDoNotAllowVias()]
pads = [p for f in fps for p in f.Pads()]
tracks = list(b.GetTracks())
holes = [(p.GetPosition().x, p.GetPosition().y, max(p.GetDrillSize().x, p.GetDrillSize().y) / 2) for p in pads if p.GetDrillSize().x > 0]
NET_1V8 = b.FindNet("+1V8").GetNetCode()
NET_3V3 = b.FindNet("+3V3").GetNetCode()


def on_own_plane(netcode, pos, margin=0.4):
    x, y = pcbnew.ToMM(pos.x), pcbnew.ToMM(pos.y)
    if netcode == NET_1V8:
        return in_island(x, y, margin)
    if netcode == NET_3V3:
        return not in_island(x, y, -margin)
    return True


def free(netcode, layer, via_pos, stub_from):
    """Via must clear other-net copper on both outer layers, the stub only on its own."""
    via = pcbnew.SHAPE_CIRCLE(via_pos, MM(VIA_D / 2))
    seg = pcbnew.SHAPE_SEGMENT(stub_from, via_pos, MM(STUB))
    for p in pads:
        if p.GetNetCode() == netcode and netcode != 0:
            continue
        for l in (F, B):
            if not (p.IsOnLayer(l) or p.GetDrillSize().x > 0):
                continue
            sh = p.GetEffectiveShape(l)
            if sh.Collide(via, MM(CLR)):
                return False
            if l == layer and sh.Collide(seg, MM(CLR)):
                return False
    for t in tracks:
        if t.GetNetCode() == netcode:
            continue
        if t.Type() == pcbnew.PCB_VIA_T:
            sh = t.GetEffectiveShape(F)
            if sh.Collide(via, MM(CLR)) or sh.Collide(seg, MM(CLR)):
                return False
        else:
            sh = t.GetEffectiveShape(t.GetLayer())
            if sh.Collide(via, MM(CLR)):
                return False
            if t.GetLayer() == layer and sh.Collide(seg, MM(CLR)):
                return False
    if not on_own_plane(netcode, via_pos):
        return False
    if not outline.Contains(via_pos) or outline.Collide(via, MM(0.35)) and not inner.Contains(via_pos):
        return False
    for z in keepouts:
        if z.Outline().Contains(via_pos) or z.Outline().Collide(via, MM(0.1)):
            return False
    for hx, hy, hr in holes:
        if math.hypot(via_pos.x - hx, via_pos.y - hy) < hr + MM(VIA_DRILL / 2 + 0.3):
            return False
    return True


def add_track(net, a, c, layer=F, width=STUB):
    t = pcbnew.PCB_TRACK(b)
    t.SetStart(a)
    t.SetEnd(c)
    t.SetWidth(MM(width))
    t.SetLayer(layer)
    t.SetNet(net)
    b.Add(t)
    tracks.append(t)


def add_via(net, pos, d=VIA_D, drill=VIA_DRILL):
    v = pcbnew.PCB_VIA(b)
    v.SetPosition(pos)
    v.SetWidth(MM(d))
    v.SetDrill(MM(drill))
    v.SetNet(net)
    b.Add(v)
    tracks.append(v)
    holes.append((pos.x, pos.y, MM(drill / 2)))


def reaches_via(p):
    for t in tracks:
        if t.GetNetCode() != p.GetNetCode():
            continue
        if t.Type() == pcbnew.PCB_VIA_T and p.HitTest(t.GetPosition()):
            return True
        if t.Type() == pcbnew.PCB_TRACE_T and (p.HitTest(t.GetStart()) or p.HitTest(t.GetEnd())):
            return True
    return False


fixed = 0
for ref, (cx, cy, _) in DACS.items():
    fp = b.FindFootprintByReference(ref)
    at = lambda o: pcbnew.VECTOR2I(*(MM(v) for v in dac_point((cx, cy), o)))
    for p in fp.Pads():
        num = p.GetNumber()
        if num in DAC_EP_PINS and p.GetNetname() == "GND":
            add_track(p.GetNet(), p.GetPosition(), at(DAC_EP_PINS[num]), width=0.15)
            fixed += 1
        elif num in DAC_ROUTES:
            pts, via = DAC_ROUTES[num]
            prev = p.GetPosition()
            for pt in pts:
                add_track(p.GetNet(), prev, at(pt))
                prev = at(pt)
            add_via(p.GetNet(), at(via))
            fixed += 1

added, inpad, left = 0, 0, []
for p in pads:
    if p.GetNetname() not in STUB_NETS or p.GetAttribute() not in (pcbnew.PAD_ATTRIB_SMD, pcbnew.PAD_ATTRIB_CONN):
        continue
    fp = p.GetParentFootprint()
    fpid = fp.GetFPIDAsString()
    if "QFN" in fpid and p.GetNumber() == "41":
        continue
    if fp.GetReference() in TRUNK_ROUTED.get(p.GetNetname(), ()):
        continue
    if reaches_via(p):
        continue
    layer = B if fp.IsFlipped() else F
    pos = p.GetPosition()
    if "DSBGA" in fpid:
        add_via(p.GetNet(), pos, 0.3, 0.15)
        inpad += 1
        continue
    px, py = pcbnew.ToMM(pos.x), pcbnew.ToMM(pos.y)
    half = max(pcbnew.ToMM(p.GetSize().x), pcbnew.ToMM(p.GetSize().y)) / 2
    done = False
    for ring in (half + 0.45, half + 0.6, half + 0.8, half + 1.0, half + 1.3, half + 1.7, half + 2.2, half + 2.8):
        for k in range(48):
            a = 2 * math.pi * k / 48
            vp = pcbnew.VECTOR2I(MM(round((px + math.cos(a) * ring) * 40) / 40), MM(round((py + math.sin(a) * ring) * 40) / 40))
            if not free(p.GetNetCode(), layer, vp, pos):
                continue
            add_track(p.GetNet(), pos, vp, layer)
            add_via(p.GetNet(), vp)
            added += 1
            done = True
            break
        if done:
            break
    if not done:
        left.append(f"{fp.GetReference()}.{p.GetNumber()} {p.GetNetname()}")

pcbnew.SaveBoard(sys.argv[1], b)
print(f"dac pins {fixed}, stub vias {added}, via-in-pad {inpad}, open {len(left)}")
for x in left:
    print("  ", x)
