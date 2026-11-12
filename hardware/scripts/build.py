"""Rebuild the unrouted ltplayer board from the schematic netlist.

Usage: build.py board.kicad_pcb netlist.net

Everything on the board is regenerated: footprints (with nets, fields and
symbol links so schematic parity holds), the outline, mounting holes and the
anchor placement. Passives are left at the origin for place_passives.py.

Coordinates: board is 67.0 x 94.6 mm centred on (CX, CY), inside the 67.4 x 95
enclosure pocket. Front (F.Cu) faces the display and the user.
"""
import os
import re
import sys
import pcbnew

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from geom import (CX, CY, W, H, RAD, J401_X, DACS, DAC_CELL, DAC_ROTATION, dac_point, kicad,
                  ISLAND_1V8, PANEL, ACTIVE, J302_X, CABLE_Y, SLOT, DPAD, DPAD_STEP, BTN_UPPER_X,
                  BTN_UPPER_Y, USB_X, SD_X, HOLE_X, HOLE_Y, SIDE_X)

MM = pcbnew.FromMM
BOARD, NETLIST = sys.argv[1], sys.argv[2]
HERE = os.path.dirname(os.path.abspath(BOARD))
SYS_FP = "/Applications/KiCad/KiCad.app/Contents/SharedSupport/footprints"

L, T, R, B = CX - W / 2, CY - H / 2, CX + W / 2, CY + H / 2

J402_OVERHANG = 1.6
# Level shifters straddle the bottom edge of the +1V8 island on In2: their
# 1.8 V pins face north onto the island, their 3.3 V pins south onto +3V3.
LS_Y = ISLAND_1V8[3] - 1.0


def enc(x, y, rot=0, side="F"):
    kx, ky = kicad(x, y)
    return (kx, ky, rot, side)


# ref: (x, y, rotation, side), KiCad board mm
ANCHORS = {
    "H101": (CX - HOLE_X, CY - HOLE_Y, 0, "F"),
    "H102": (CX + HOLE_X, CY - HOLE_Y, 0, "F"),
    "H103": (CX - HOLE_X, CY + HOLE_Y, 0, "F"),
    "H104": (CX + HOLE_X, CY + HOLE_Y, 0, "F"),
    # top edge: 3.5 mm jack (nose passes the edge), 4.4 mm jack (body overhangs)
    "J401": (J401_X, T + 4.5, 180, "F"),
    "J402": (146.0, T - J402_OVERHANG, 0, "F"),
    # bottom edge: USB-C centred on the front, microSD on the back
    "J101": (CX + USB_X, B - 3.67, 0, "F"),
    "J301": (CX + SD_X, B - 8.7, 0, "B"),
    # five-way pad, then back and menu above its upper corners
    "SW207": enc(DPAD[0], DPAD[1] + DPAD_STEP),
    "SW208": enc(DPAD[0], DPAD[1] - DPAD_STEP),
    "SW201": enc(DPAD[0] - DPAD_STEP, DPAD[1]),
    "SW202": enc(DPAD[0] + DPAD_STEP, DPAD[1]),
    "SW203": enc(*DPAD),
    "SW204": enc(-BTN_UPPER_X, BTN_UPPER_Y),
    "SW209": enc(BTN_UPPER_X, BTN_UPPER_Y),
    # right edge side-push: power, vol+, vol-
    "SW101": (CX + SIDE_X, 66.0, 270, "F"),
    "SW205": (CX + SIDE_X, 80.0, 270, "F"),
    "SW206": (CX + SIDE_X, 90.0, 270, "F"),
    # display cable connector on the back, its opening facing the cable slot
    "J302": (*kicad(J302_X, CABLE_Y), 90, "B"),
    # back: battery pads and the optional side-entry JST-PH next to them
    "J103": (*kicad(-27.0, -30.0), 90, "B"),
    "J102": (*kicad(-27.5, -15.5), 270, "B"),
    # MCU under the lower half of the panel
    "U201": (131.0, 97.0, 0, "F"),
    # audio
    "U406": (125.0, 56.0, 0, "F"),
    "U407": (130.5, 56.0, 0, "F"),
    "U408": (136.0, 56.0, 0, "F"),
    "R453": (127.4, 61.6, 0, "F"),
    "U403": (121.5, LS_Y, 270, "F"),
    "U404": (125.5, LS_Y, 270, "F"),
    "Y401": (130.5, LS_Y - 1.2, 0, "F"),
    "U405": (135.5, LS_Y, 270, "F"),
    "U104": (106.0, 64.0, 0, "F"),
    "U105": (106.0, 69.5, 0, "F"),
    # power, in the lower left column beside the pad
    "U101": (123.5, 134.0, 90, "F"),
    "U102": (110.5, 117.0, 0, "F"),
    "U103": (117.0, 126.0, 0, "F"),
    # buck for the 1.8 V LDOs, next to them on the left
    "U106": (111.0, 92.5, 0, "F"),
    "L101": (111.0, 88.5, 0, "F"),
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

# slot for the display cable, rounded ends
sx0, sy0 = kicad(SLOT[0], SLOT[3])
sx1, sy1 = kicad(SLOT[2], SLOT[1])
sr = (sx1 - sx0) / 2
seg(sx0, sy0 + sr, sx0, sy1 - sr)
seg(sx1, sy0 + sr, sx1, sy1 - sr)
arc(sx0, sy0 + sr, sx0 + sr, sy0, sx1, sy0 + sr)
arc(sx0, sy1 - sr, sx0 + sr, sy1, sx1, sy1 - sr)

# display panel and picture area, for reference only
dl = pcbnew.User_1
for x0, y0, x1, y1 in (PANEL, ACTIVE):
    (ax, ay), (bx, by) = kicad(x0, y1), kicad(x1, y0)
    seg(ax, ay, bx, ay, dl, 0.15)
    seg(bx, ay, bx, by, dl, 0.15)
    seg(bx, by, ax, by, dl, 0.15)
    seg(ax, by, ax, ay, dl, 0.15)

pcbnew.SaveBoard(BOARD, b)
print(f"built {len(fps)} footprints, {len(netinfo)} nets, {len(ANCHORS)} anchored")
