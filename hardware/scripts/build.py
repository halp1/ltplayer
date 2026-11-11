"""Rebuild the unrouted ltplayer board from the schematic netlist.

Usage: build.py board.kicad_pcb netlist.net

Everything on the board is regenerated: footprints (with nets, fields and
symbol links so schematic parity holds), the outline, mounting holes and the
anchor placement. Passives are left at the origin for place_passives.py.

Coordinates: board is 61.6 x 94.6 mm centred on (CX, CY), inside the 62 x 95
enclosure pocket. Front (F.Cu) faces the display and the user.
"""
import os
import re
import sys
import pcbnew

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from geom import CX, CY, W, H, RAD, J401_X, DACS, DAC_CELL, DAC_ROTATION, dac_point

MM = pcbnew.FromMM
BOARD, NETLIST = sys.argv[1], sys.argv[2]
HERE = os.path.dirname(os.path.abspath(BOARD))
SYS_FP = "/Applications/KiCad/KiCad.app/Contents/SharedSupport/footprints"

L, T, R, B = CX - W / 2, CY - H / 2, CX + W / 2, CY + H / 2
HOLE_X, HOLE_Y = 24.65, 41.15

# Adafruit 4311 breakout: 35.56 x 59.055, two M2.5 holes 2.54 in from the top corners.
DISP_W, DISP_H = 35.56, 59.055
DISP_X0, DISP_Y0 = CX - DISP_W / 2, 72.0

J402_OVERHANG = 1.6
BTN_Y = 134.2

# ref: (x, y, rotation, side)
ANCHORS = {
    "H101": (CX - HOLE_X, CY - HOLE_Y, 0, "F"),
    "H102": (CX + HOLE_X, CY - HOLE_Y, 0, "F"),
    "H103": (CX - HOLE_X, CY + HOLE_Y, 0, "F"),
    "H104": (CX + HOLE_X, CY + HOLE_Y, 0, "F"),
    "H105": (DISP_X0 + 2.54, DISP_Y0 + 2.54, 0, "F"),
    "H106": (DISP_X0 + DISP_W - 2.54, DISP_Y0 + 2.54, 0, "F"),
    # top edge: 3.5 mm jack (nose passes the edge), 4.4 mm jack (body overhangs)
    "J401": (J401_X, T + 4.5, 180, "F"),
    "J402": (146.0, T - J402_OVERHANG, 0, "F"),
    # bottom edge
    "J101": (114.5, B - 3.67, 0, "F"),
    "J301": (142.0, B - 8.7, 0, "B"),
    # front buttons below the display: back, left, play/pause, right
    "SW204": (CX - 12.0, BTN_Y, 0, "F"),
    "SW201": (CX - 4.0, BTN_Y, 0, "F"),
    "SW203": (CX + 4.0, BTN_Y, 0, "F"),
    "SW202": (CX + 12.0, BTN_Y, 0, "F"),
    # right edge side-push: power, vol+, vol-
    "SW101": (R - 2.35, 66.0, 270, "F"),
    "SW205": (R - 2.35, 80.0, 270, "F"),
    "SW206": (R - 2.35, 90.0, 270, "F"),
    # left side
    "J302": (106.0, 100.0, 90, "F"),
    "J102": (104.5, 121.0, 0, "B"),
    "J201": (107.0, 84.5, 0, "B"),
    # MCU
    "U201": (131.0, 104.0, 0, "F"),
    # audio
    "U406": (125.0, 56.0, 0, "F"),
    "U407": (130.5, 56.0, 0, "F"),
    "U408": (136.0, 56.0, 0, "F"),
    "Y401": (130.5, 77.0, 0, "F"),
    "R453": (127.4, 61.6, 0, "F"),
    # level shifters: 1.8 V side faces the DACs, 3.3 V side faces the MCU
    "U403": (119.0, 83.5, 270, "F"),
    "U404": (125.5, 83.5, 270, "F"),
    "U405": (132.0, 83.5, 270, "F"),
    "U104": (106.0, 64.0, 0, "F"),
    "U105": (106.0, 72.0, 0, "F"),
    # power
    "U101": (122.5, 140.5, 90, "F"),
    "U102": (112.0, 127.0, 0, "F"),
    "D102": (108.5, 133.0, 90, "F"),
    "Q101": (104.5, 131.0, 0, "F"),
    "U103": (119.0, 122.0, 0, "F"),
    "L201": (141.0, 119.5, 0, "F"),
}

for dac, (dx, dy, base) in DACS.items():
    ANCHORS[dac] = (dx, dy, DAC_ROTATION, "F")
    for n, (ox, oy, rot) in DAC_CELL.items():
        x, y = dac_point((dx, dy), (ox, oy))
        ANCHORS["C4%02d" % (base + n)] = (x, y, (rot + DAC_ROTATION) % 360, "F")


def tokens(text):
    for m in re.finditer(r'\(|\)|"(?:\\.|[^"\\])*"|[^\s()]+', text):
        t = m.group(0)
        yield t


def parse(text):
    stack, cur = [], []
    for t in tokens(text):
        if t == "(":
            stack.append(cur)
            cur = []
        elif t == ")":
            done, cur = cur, stack.pop()
            cur.append(done)
        elif t.startswith('"'):
            cur.append(t[1:-1].replace('\\"', '"').replace("\\\\", "\\"))
        else:
            cur.append(t)
    return cur[0]


def child(node, key):
    for n in node:
        if isinstance(n, list) and n and n[0] == key:
            return n
    return None


def children(node, key):
    return [n for n in node if isinstance(n, list) and n and n[0] == key]


def val(node, key, default=""):
    c = child(node, key)
    return c[1] if c and len(c) > 1 else default


def load_fp(fpid):
    lib, name = fpid.split(":", 1)
    path = os.path.join(HERE, "ltplayer.pretty") if lib == "ltplayer" else os.path.join(SYS_FP, lib + ".pretty")
    fp = pcbnew.FootprintLoad(path, name)
    if fp is None:
        raise SystemExit(f"footprint not found: {fpid}")
    fp.SetFPID(pcbnew.LIB_ID(lib, name))
    return fp


tree = parse(open(NETLIST).read())
comps = children(child(tree, "components"), "comp")
nets = children(child(tree, "nets"), "net")

# Start from an empty board. Design rules and net classes live in the
# .kicad_pro and are applied by rules.py afterwards.
b = pcbnew.CreateEmptyBoard()
b.SetCopperLayerCount(4)

# nets
netinfo = {}
for n in nets:
    name = val(n, "name")
    ni = b.FindNet(name)
    if ni is None:
        ni = pcbnew.NETINFO_ITEM(b, name)
        b.Add(ni)
    netinfo[name] = ni
pin_net = {}
for n in nets:
    for node in children(n, "node"):
        pin_net[(val(node, "ref"), val(node, "pin"))] = netinfo[val(n, "name")]

# footprints
fps = {}
for c in comps:
    ref = val(c, "ref")
    fp = load_fp(val(c, "footprint"))
    fp.SetReference(ref)
    fp.SetValue(val(c, "value"))
    sp = child(c, "sheetpath")
    fp.SetPath(pcbnew.KIID_PATH(val(sp, "tstamps") + val(c, "tstamps")))
    props = {val(p, "name"): val(p, "value") for p in children(c, "property")}
    fp.SetSheetname(props.get("Sheetname", ""))
    fp.SetSheetfile(props.get("Sheetfile", ""))
    fp.SetExcludedFromBOM("exclude_from_bom" in props)
    fp.SetExcludedFromPosFiles("exclude_from_board" in props)
    fp.SetDNP("dnp" in props)
    fields = child(c, "fields")
    for f in children(fields, "field") if fields else []:
        name = val(f, "name")
        if name in ("Datasheet", "Description") and len(f) > 2:
            fp.SetField(name, f[2])
    desc = val(c, "description")
    if desc:
        fp.SetField("Description", desc)
    ds = val(c, "datasheet")
    if ds:
        fp.SetField("Datasheet", ds)
    for pad in fp.Pads():
        ni = pin_net.get((ref, pad.GetNumber()))
        if ni is not None:
            pad.SetNet(ni)
        if "QFN" in val(c, "footprint") and pad.GetNumber() == "41":
            # exposed pad and its thermal vias join the ground fill solidly
            pad.SetLocalZoneConnection(pcbnew.ZONE_CONNECTION_FULL)
    fp.Reference().SetVisible(False)
    fp.Value().SetVisible(False)
    fp.SetPosition(pcbnew.VECTOR2I(0, 0))
    b.Add(fp)
    fps[ref] = fp

missing = [r for r in ANCHORS if r not in fps]
if missing:
    raise SystemExit(f"anchors not in netlist: {missing}")

for ref, (x, y, rot, side) in ANCHORS.items():
    fp = fps[ref]
    fp.SetPosition(pcbnew.VECTOR2I(MM(x), MM(y)))
    fp.SetOrientationDegrees(rot)
    if side == "B":
        fp.Flip(fp.GetPosition(), pcbnew.FLIP_DIRECTION_LEFT_RIGHT)

def seg(x1, y1, x2, y2, layer=pcbnew.Edge_Cuts, width=0.1):
    s = pcbnew.PCB_SHAPE(b, pcbnew.SHAPE_T_SEGMENT)
    s.SetStart(pcbnew.VECTOR2I(MM(x1), MM(y1)))
    s.SetEnd(pcbnew.VECTOR2I(MM(x2), MM(y2)))
    s.SetLayer(layer)
    s.SetWidth(MM(width))
    b.Add(s)


def arc(x1, y1, xm, ym, x2, y2, layer=pcbnew.Edge_Cuts, width=0.1):
    a = pcbnew.PCB_SHAPE(b, pcbnew.SHAPE_T_ARC)
    a.SetArcGeometry(pcbnew.VECTOR2I(MM(x1), MM(y1)), pcbnew.VECTOR2I(MM(xm), MM(ym)), pcbnew.VECTOR2I(MM(x2), MM(y2)))
    a.SetLayer(layer)
    a.SetWidth(MM(width))
    b.Add(a)


k = RAD * (1 - 2 ** -0.5)
nl, nr = J401_X - 2.2, J401_X + 2.2
# top edge with the jack notch (4.4 wide, 1.4 deep, 0.4 inner radius)
seg(L + RAD, T, nl, T)
seg(nl, T, nl, T + 1.0)
arc(nl, T + 1.0, nl + 0.4 * (1 - 2 ** -0.5), T + 1.0 + 0.4 * 2 ** -0.5, nl + 0.4, T + 1.4)
seg(nl + 0.4, T + 1.4, nr - 0.4, T + 1.4)
arc(nr - 0.4, T + 1.4, nr - 0.4 * (1 - 2 ** -0.5), T + 1.0 + 0.4 * 2 ** -0.5, nr, T + 1.0)
seg(nr, T + 1.0, nr, T)
seg(nr, T, R - RAD, T)
arc(R - RAD, T, R - k, T + k, R, T + RAD)
seg(R, T + RAD, R, B - RAD)
arc(R, B - RAD, R - k, B - k, R - RAD, B)
seg(R - RAD, B, L + RAD, B)
arc(L + RAD, B, L + k, B - k, L, B - RAD)
seg(L, B - RAD, L, T + RAD)
arc(L, T + RAD, L + k, T + k, L + RAD, T)

# display breakout outline, for reference only
dl = pcbnew.User_1
seg(DISP_X0, DISP_Y0, DISP_X0 + DISP_W, DISP_Y0, dl, 0.15)
seg(DISP_X0 + DISP_W, DISP_Y0, DISP_X0 + DISP_W, DISP_Y0 + DISP_H, dl, 0.15)
seg(DISP_X0 + DISP_W, DISP_Y0 + DISP_H, DISP_X0, DISP_Y0 + DISP_H, dl, 0.15)
seg(DISP_X0, DISP_Y0 + DISP_H, DISP_X0, DISP_Y0, dl, 0.15)

pcbnew.SaveBoard(BOARD, b)
print(f"built {len(fps)} footprints, {len(netinfo)} nets, {len(ANCHORS)} anchored")
