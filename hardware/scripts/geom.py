"""Geometry and net tables shared by the layout scripts (KiCad board mm)."""

# Board: 67.0 x 94.6 with 6.15 mm corners, inside the 67.4 x 95 enclosure
# pocket. Widened from 61.6 so the display picture sits on the centre line.
CX, CY = 131.0, 97.5
W, H, RAD = 67.0, 94.6, 6.15
EDGE = (CX - W / 2, CY - H / 2, CX + W / 2, CY + H / 2)  # left, top, right, bottom
J401_X = 116.0
NOTCH = (J401_X - 2.2, EDGE[1], J401_X + 2.2, EDGE[1] + 1.4)  # 3.5 mm jack nose
# Corner holes sit on the corner arc centres.
HOLE_X, HOLE_Y = W / 2 - RAD, H / 2 - RAD


def kicad(x, y):
    """Enclosure mm (origin at the board centre, +Y up) to KiCad board mm."""
    return (round(CX + x, 4), round(CY - y, 4))


# Display: Newhaven NHD-2.4-240320AF-CSXP, 2.4in IPS 240x320, ST7789VI, 4-wire
# SPI. Portrait in the drawing with the cable on the bottom short edge; mounted
# landscape with that edge on the left. Picture area borders: 3.02 mm on the
# portrait top (right here), 7.93 mm on the cable side (left), 3.04 mm on
# both long sides. Picture centred on x = 0.
PANEL_W, PANEL_H, PANEL_T = 59.91, 42.80, 2.55  # outline, landscape
ACTIVE_W, ACTIVE_H = 48.96, 36.72
PANEL_TOP = 38.5
ACTIVE = (-ACTIVE_W / 2, PANEL_TOP - 3.04 - ACTIVE_H, ACTIVE_W / 2, PANEL_TOP - 3.04)
PANEL = (ACTIVE[0] - 7.93, PANEL_TOP - PANEL_H, ACTIVE[2] + 3.02, PANEL_TOP)  # x0, y0, x1, y1
# The cable leaves the left edge of the panel. Unfolded, from that edge: the
# first 13.2 mm are the 39 mm wide shielded part, then a 20.5 mm wide, 0.3 mm
# thick tail ends 30 mm out (contacts on the panel-rear face). It is made to
# fold flat behind the panel: folded, the tail ends 27.80 mm from the edge, so
# the fold takes 2.2 mm. Only the narrow tail fits the slot, so the slot sits
# just past the end of the wide part. The tail drops through it and runs along
# the back into J302, whose opening faces the slot.
#
# The display sits as low as the front parts allow: PANEL_GAP above the tallest
# part under it, an SOT-23 at 1.55 mm. No part in the fold strip (under the
# folded 39 mm part, which is 0.8 mm thick with its parts) may be taller than
# 0603 resistors and capacitors. assembly.py checks both.
PANEL_GAP = 0.3
PANEL_BACK_EST = 3.45
#
# Contacts: unfolded they are on the panel-rear face. After the fold, the drop
# and the turn along the back they face the board, so J302 is a bottom-contact
# FH12. Pin order: the tail arrives mirrored, panel pin n lands on J302 pin
# 41 - n. The schematic labels are wired that way.
CABLE_Y = (PANEL[1] + PANEL[3]) / 2
CABLE_W = 20.5
FOLD_LOSS = 2.2
WIDE_END = PANEL[0] + 13.2 - FOLD_LOSS      # x where the 39 mm wide part ends, folded
SLOT_W = 1.2
SLOT = (WIDE_END + 0.5, CABLE_Y - CABLE_W / 2 - 0.5, WIDE_END + 0.5 + SLOT_W, CABLE_Y + CABLE_W / 2 + 0.5)
# Narrow tail 16.8 mm (+-0.5): flat behind the panel to the slot centre, down
# through the board to the connector entry 0.9 mm off the back, then along the
# back to the tail end, 3.3 mm inside J302 (1.1 mm short of the footprint
# origin, which is 4.4 mm behind the actuator front). J302 sits where that
# leaves CABLE_SLACK to bow in the gap under the panel.
TAIL_L = 16.8
CABLE_SLACK = 2.0
_SX = (SLOT[0] + SLOT[2]) / 2
_DROP = (PANEL_BACK_EST - 0.6) + 0.9
J302_X = round(_SX + 1.1 + (TAIL_L - CABLE_SLACK - (_SX - WIDE_END) - _DROP), 2)
# Battery: 506060-size LiPo pouch, 5.0 x 60 x 60 mm bare, 2000 to 2500 mAh,
# about 62 long with the protection board, 61 wide with tape. On the back
# between the bottom screw bosses and the two jacks, which sit on the back
# along the top edge. Its top face sits BATT_GAP below the deepest part under
# it, the 2 mm display connector. Wires leave the protection board end
# (bottom) and run to J103 or J102 in the bottom margin.
BATT_W, BATT_L, BATT_T = 61.0, 62.0, 5.0
BATT = (-BATT_W / 2, -37.0, BATT_W / 2, -37.0 + BATT_L)
BATT_GAP = 0.3
FOLD_STRIP = (PANEL[0] - 0.8, PANEL[1], WIDE_END + 0.3, PANEL[3])

# Front controls, enclosure mm. A five-way pad with back and menu above its
# upper corners and below the screen.
DPAD = (0.0, -24.0)
DPAD_STEP = 8.5
BTN_UPPER_Y = -10.0
BTN_UPPER_X = 19.0
USB_X = 0.0
SD_X = 16.5   # J301 courtyard 0.5 mm clear of the H104 boss
SIDE_X = W / 2 - 2.35   # side-push button origin, actuator at the right edge

# Nets tied to an inner plane through a via at every pad.
PLANES = ("GND", "+3V3", "+1V8")
# Nets routed as traces on In2.Cu between stub vias, not by the autorouter.
# VSYS reaches every regulator and both DACs. HP_GND is the headphone return:
# a dedicated trace from each DAC to both jack sleeves and the 0 ohm tie R453.
TRUNK = ("HP_GND", "VSYS")  # HP_GND first: it is short and local, VSYS goes round it
TRUNK_ROUTED = {}

# In2.Cu carries +3V3 with a +1V8 island under the DAC block. Its bottom edge
# runs through the level-shifter row: 1.8 V pins face north, 3.3 V pins south.
ISLAND_1V8 = (101.5, 58.5, 144.0, 80.0)
# Cut out of the island's bottom edge under the display connector J302 (on
# the back), so its +3V3 pins, which sit level with the island, reach the
# +3V3 pour: x0, y0, x1 of the cut, it runs down to the island edge.
ISLAND_NOTCH = (121.5, 73.0, 125.8, 80.0)


def island_outline():
    x0, y0, x1, y1 = ISLAND_1V8
    nx0, ny0, nx1, _ = ISLAND_NOTCH
    return [(x0, y0), (x1, y0), (x1, y1), (nx1, y1), (nx1, ny0), (nx0, ny0), (nx0, y1), (x0, y1)]


def in_island(x, y, margin=0.0):
    """True if (x, y) lies at least margin inside the +1V8 island (negative
    margin grows it)."""
    x0, y0, x1, y1 = ISLAND_1V8
    if not (x0 + margin <= x <= x1 - margin and y0 + margin <= y <= y1 - margin):
        return False
    nx0, ny0, nx1, ny1 = ISLAND_NOTCH
    return not (nx0 - margin <= x <= nx1 + margin and ny0 - margin <= y <= ny1 + margin)

# Net classes: (name, track width, [nets]). One clearance for every class so
# the autorouter and DRC agree. 0.2 mm is what fits a 0.4 mm pitch QFN pin.
CLEARANCE = 0.127
_CP = [f"DAC{i}_{s}" for i in (1, 2) for s in ("FILTP", "FILTN", "NVA", "FLYP_VA", "FLYN_VA", "VCPP", "VCPN", "FLYP", "FLYN", "FLYC")]
CLASSES = [
    ("Power", 0.25, ["VBUS", "VBAT", "VSYS", "+3V3", "+1V8", "+1V8A", "VDD11", "SMPS_LX", "GND", "HP_GND"]),
    ("Audio", 0.2, ["DAC1_OUTA", "DAC1_OUTB", "DAC2_OUTA", "DAC2_OUTB", "J35_TIP", "J35_RING", "J44_LP", "J44_LN", "J44_RP", "J44_RN"]),
    ("ChargePump", 0.2, _CP),
    ("USB", 0.2, ["USB_DP", "USB_DM"]),
]

# CS43131 positions and the support parts around each one. Offsets are written
# for the package in its library orientation: (dx, dy, rotation). Index n is
# C4nn for U401 and C4(nn+20) for U402. Caps sit in pin order around the
# package so the fan-out never crosses.
#
# Both DACs are mounted rotated 180 degrees, so the analog pins (11 to 20)
# face the jacks at the top edge and the I2S and clock pins (31 to 40) face
# the level shifters and the MCU. dac_point() applies that rotation.
DACS = {"U401": (119.4, 69.2, 0), "U402": (133.9, 69.2, 20)}
DAC_ROTATION = 180


def dac_point(centre, offset):
    return (round(centre[0] - offset[0], 4), round(centre[1] - offset[1], 4))


DAC_CELL = {
    10: (-5.0, -2.9, 180),    # VL 100n            pin 4
    1: (-5.0, -1.25, 180),    # FILT+              pin 5
    2: (-5.0, 0.55, 0),       # FILT-              pin 6
    11: (-5.0, 2.2, 180),     # VA                 pin 7
    3: (-5.0, 3.85, 0),       # -VA                pin 9
    4: (-2.6, 4.6, 270),      # FLYP_VA / FLYN_VA  pins 10, 11
    6: (1.6, 4.6, 90),        # -VCP               pin 18
    7: (4.6, 1.85, 270),      # FLYP / FLYC        pins 24, 23
    5: (3.9, 4.15, 0),        # +VCP               pin 21
    8: (6.4, 4.0, 270),       # FLYC / FLYN        pins 23, 20
    12: (6.85, 1.0, 90),      # VCP 2.2u           pin 25
    13: (4.425, -1.3, 90),    # VP 100n            pin 26
    14: (6.4, -2.2, 90),      # VP 4.7u            pin 26
    9: (2.575, -3.9, 0),      # VD 100n            pin 31
}

# Ground pins that run straight in to the exposed pad: pin -> end point.
DAC_EP_PINS = {"8": (-1.7, 1.0), "19": (1.4, 1.7), "30": (1.7, -1.8), "35": (0.2, -1.7)}
# Fixed fan-out: pin -> (track points from the pin, via position). Supply
# tracks pass through the pads of that pin's decoupling capacitors.
DAC_ROUTES = {
    "4": ([(-3.0, -0.6), (-3.35, -0.95)], (-3.35, -0.95)),
    # VL (+1V8) straight out to a via just short of the FLYP cap
    "25": ([(3.2, 0.2), (3.7, 0.32)], (3.7, 0.32)),
    # VP (VSYS) straight out through its 100n cap's VSYS pad to a via past it
    "26": ([(4.425, -0.2), (5.15, -0.2)], (5.15, -0.2)),
    # DAC_INT and DAC_RESET: fixed fan-out vias, the router takes them on
    # from there on any layer. Without them the supply tracks above box the
    # two pins in.
    "27": ([(3.0, -0.6), (3.55, -0.75)], (3.55, -0.75)),
    "28": ([(3.0, -1.0), (3.45, -1.5)], (3.45, -1.5)),
    "31": ([(1.8, -3.9), (1.8, -5.0)], (1.8, -5.0)),
    # short cap connections the router sometimes boxes in: pad to cap pad,
    # no via (None)
    "5": ([(-3.85, -0.2), (-3.85, -0.9)], None),
    # charge-pump flying cap: FLYP (24) and FLYC (23) straight to C407
    "24": ([(3.3, 0.6), (3.775, 1.075), (4.6, 1.075)], None),
    "23": ([(3.1, 1.0), (3.1, 2.625), (4.6, 2.625)], None),
    # HP_GND between -VCP and OUTB: straight out to a via, joined on In2
    "17": ([(0.6, 3.55)], (0.6, 3.55)),
}
