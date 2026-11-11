"""Copy library footprints into ltplayer.pretty without edge and silk outlines.

Usage: localize_fp.py

The jacks, USB-C, microSD and side buttons hang over the board edge. Their
stock footprints draw an Edge.Cuts notch or a silk body outline there, which
would fight the board outline drawn by build.py. Rerun after a KiCad library
update to pick up upstream fixes.
"""
import os
import re

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SYS = "/Applications/KiCad/KiCad.app/Contents/SharedSupport/footprints"
LOCAL = os.path.join(HERE, "ltplayer.pretty")
SOURCES = [
    ("Connector_Audio", "Jack_3.5mm_CUI_SJ1-3524N_Horizontal"),
    ("Connector_USB", "USB_C_Receptacle_GCT_USB4105-xx-A_16P_TopMnt_Horizontal"),
    ("Connector_Card", "microSD_HC_Hirose_DM3AT-SF-PEJM5"),
    ("ltplayer", "Jack_4.4mm_PJ-4450_Horizontal"),
    ("ltplayer", "SW_Alps_SKSC_SidePush"),
]
DROP = ('"Edge.Cuts"', '"F.SilkS"', '"B.SilkS"')
GRAPHICS = ("fp_line", "fp_arc", "fp_circle", "fp_rect", "fp_poly")


def top_level_items(text):
    """Yield (start, end) of each direct child expression of the footprint."""
    depth, start = 0, None
    for i, c in enumerate(text):
        if c == "(":
            depth += 1
            if depth == 2:
                start = i
        elif c == ")":
            if depth == 2:
                yield start, i + 1
            depth -= 1


for lib, name in SOURCES:
    src = os.path.join(LOCAL if lib == "ltplayer" else os.path.join(SYS, lib + ".pretty"), name + ".kicad_mod")
    text = open(src).read()
    out, pos, dropped = [], 0, 0
    for a, z in top_level_items(text):
        item = text[a:z]
        head = re.match(r"\((\w+)", item).group(1)
        if head in GRAPHICS and any(("(layer %s)" % l) in item for l in DROP):
            out.append(text[pos:a].rstrip(" \t"))
            pos = z
            dropped += 1
    out.append(text[pos:])
    text = re.sub(r"\n\s*\n+", "\n", "".join(out))
    if lib != "ltplayer":
        text = re.sub(r'\(descr "', '(descr "ltplayer copy, edge and silk outline removed. ', text, count=1)
    open(os.path.join(LOCAL, name + ".kicad_mod"), "w").write(text)
    print(f"{name}: dropped {dropped} graphics")
