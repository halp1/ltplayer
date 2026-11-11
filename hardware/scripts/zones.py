"""Copper zones: In1 solid GND, In2 split power, F/B GND fill.

In2 carries +3V3 over most of the board, with a +1V8 island under the DAC and
level-shifter block at the top. Outer layers get low-priority GND fill so
leftover copper stitches back to the plane.
"""
import os
import sys
import pcbnew

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from geom import ISLAND_1V8

MM = pcbnew.FromMM
b = pcbnew.LoadBoard(sys.argv[1])
for z in list(b.Zones()):
    b.Remove(z)

L, T, R, B = 99.0, 49.0, 163.0, 146.0  # oversize, KiCad clips to the board edge


def zone(layer, net, pts, prio=0, clear=0.2):
    z = pcbnew.ZONE(b)
    z.SetLayer(layer)
    z.SetNet(b.FindNet(net))
    z.SetAssignedPriority(prio)
    z.SetLocalClearance(MM(clear))
    z.SetMinThickness(MM(0.2))
    z.SetPadConnection(pcbnew.ZONE_CONNECTION_THERMAL)
    z.SetThermalReliefGap(MM(0.25))
    z.SetThermalReliefSpokeWidth(MM(0.3))
    o = z.Outline()
    o.NewOutline()
    for x, y in pts:
        o.Append(MM(x), MM(y))
    b.Add(z)


box = [(L, T), (R, T), (R, B), (L, B)]
zone(pcbnew.In1_Cu, "GND", box)
zone(pcbnew.In2_Cu, "+3V3", box, prio=0)
ix0, iy0, ix1, iy1 = ISLAND_1V8
zone(pcbnew.In2_Cu, "+1V8", [(ix0, iy0), (ix1, iy0), (ix1, iy1), (ix0, iy1)], prio=1)
zone(pcbnew.F_Cu, "GND", box, prio=0, clear=0.25)
zone(pcbnew.B_Cu, "GND", box, prio=0, clear=0.25)

pcbnew.SaveBoard(sys.argv[1], b)
print("zones", len(list(b.Zones())))
