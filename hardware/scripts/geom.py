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
# The 0.3 mm cable leaves the left edge 20.5 mm wide (contacts 19.5 mm across),
# centred on the panel's short axis. It folds down through a slot in the board
# just inside the panel edge and plugs into J302 on the back, whose opening
# faces the slot. The tail is 30 mm long from the panel edge.
CABLE_Y = (PANEL[1] + PANEL[3]) / 2
CABLE_W = 20.5
SLOT = (PANEL[0] + 0.6, CABLE_Y - CABLE_W / 2 - 0.5, PANEL[0] + 0.6 + 1.2, CABLE_Y + CABLE_W / 2 + 0.5)
J302_X = SLOT[2] + 5.0   # back side, connector origin, 5 mm of free tail past the slot
# The panel's back face sits this far above the tallest part under it.
PANEL_GAP = 0.3

# Front controls, enclosure mm. A five-way pad with back and menu above its
# upper corners and below the screen.
DPAD = (0.0, -24.0)
DPAD_STEP = 8.5
BTN_UPPER_Y = -10.0
BTN_UPPER_X = 19.0
USB_X = 0.0
SD_X = 13.5
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
ISLAND_1V8 = (101.5, 58.5, 141.0, 80.0)
# Bottom-left corner cut out of the island so the display connector's +3V3
# pin can reach the +3V3 pour: x0, y0 of the cut, it runs to the island edge.
ISLAND_NOTCH = (101.5, 74.5, 120.0, 80.0)


def island_outline():
    x0, y0, x1, y1 = ISLAND_1V8
    nx1, ny0 = ISLAND_NOTCH[2], ISLAND_NOTCH[1]
    return [(x0, y0), (x1, y0), (x1, y1), (nx1, y1), (nx1, ny0), (x0, ny0)]


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
DACS = {"U401": (120.75, 67.85, 0), "U402": (134.5, 67.85, 20)}
DAC_ROTATION = 180


def dac_point(centre, offset):
    return (round(centre[0] - offset[0], 4), round(centre[1] - offset[1], 4))


DAC_CELL = {
    10: (-4.68, -2.6, 0),     # VL 100n            pin 4
    1: (-5.0, -1.0, 180),     # FILT+              pin 5
    2: (-5.0, 0.6, 0),        # FILT-              pin 6
    11: (-5.48, 1.9, 180),    # VA                 pin 7
    3: (-5.48, 2.95, 0),      # -VA                pin 9
    4: (-2.7, 3.9, 0),        # FLYP_VA / FLYN_VA  pins 10, 11
    6: (1.55, 4.12, 90),      # -VCP               pin 18
    5: (4.48, 2.85, 0),       # +VCP               pin 21
    7: (5.0, 1.3, 270),       # FLYP / FLYC        pins 24, 23
    8: (6.3, 3.0, 270),       # FLYC / FLYN        pins 23, 20
    12: (6.3, -0.45, 90),     # VCP 2.2u           pin 25
    13: (4.9, -0.75, 0),      # VP 100n            pin 26
    14: (5.3, -2.2, 0),       # VP 4.7u            pin 26
    9: (2.28, -3.7, 0),       # VD 100n            pin 31
}

# Ground pins that run straight in to the exposed pad: pin -> end point.
DAC_EP_PINS = {"8": (-1.7, 1.0), "19": (1.4, 1.7), "30": (1.7, -1.8), "35": (0.2, -1.7)}
# Supply pins: pin -> (track points from the pin, via position). Each track
# passes through the pads of that pin's decoupling capacitors.
DAC_ROUTES = {
    "4": ([(-3.0, -0.6), (-3.35, -0.95)], (-3.35, -0.95)),
    "25": ([(6.2, 0.2), (6.3, 0.95)], (6.3, 0.95)),
    "26": ([(3.9, -0.2), (4.3, -0.6), (4.75, -1.4), (4.525, -2.2), (4.525, -3.25)], (4.525, -3.25)),
    "31": ([(1.8, -3.6), (1.8, -4.5)], (1.8, -4.5)),
    # HP_GND between -VCP and OUTB: straight out to a via, joined on In2
    "17": ([(0.6, 3.5)], (0.6, 3.5)),
}
