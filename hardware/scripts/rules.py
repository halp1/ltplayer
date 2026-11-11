"""Apply board design rules and net classes (JLCPCB 4-layer capabilities).

KiCad stores these in the .kicad_pro next to the board, so this must run on the
real board path. Saving a board loaded from another directory resets them.
"""
import os
import sys
import pcbnew

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from geom import CLASSES, CLEARANCE

MM = pcbnew.FromMM
b = pcbnew.LoadBoard(sys.argv[1])
b.SetCopperLayerCount(4)
ds = b.GetDesignSettings()
ds.m_TrackMinWidth = MM(0.1)
ds.m_MinClearance = MM(0.1)
ds.m_ViasMinSize = MM(0.3)
ds.m_ViasMinAnnularWidth = MM(0.075)
ds.m_MinThroughDrill = MM(0.15)
ds.m_HoleToHoleMin = MM(0.25)
ds.m_HoleClearance = MM(0.2)
ds.m_CopperEdgeClearance = MM(0.3)
ds.m_SilkClearance = MM(0.0)
# every ground pad has its own via to In1, so one spoke into the outer fill is enough
ds.m_MinResolvedSpokes = 1

ns = ds.m_NetSettings
d = ns.GetDefaultNetclass()
d.SetClearance(MM(CLEARANCE))
d.SetTrackWidth(MM(0.15))
d.SetViaDiameter(MM(0.45))
d.SetViaDrill(MM(0.25))

for name, width, nets in CLASSES:
    c = pcbnew.NETCLASS(name)
    c.SetTrackWidth(MM(width))
    c.SetClearance(MM(CLEARANCE))
    c.SetViaDiameter(MM(0.45))
    c.SetViaDrill(MM(0.25))
    ns.SetNetclass(name, c)
    for n in nets:
        ns.SetNetclassPatternAssignment(n, name)

pcbnew.SaveBoard(sys.argv[1], b)
print("rules applied, copper layers", b.GetCopperLayerCount())
