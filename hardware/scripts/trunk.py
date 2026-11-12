"""Route the trunk nets (VSYS, HP_GND) on In2.Cu between their stub vias.

Usage: trunk.py board.kicad_pcb

Runs after the autorouter, so every via on the board is known. Each net is
routed in turn and its copper blocks the nets after it. Through-hole pads of
the net (jack sleeves) count as targets like the vias. In2.Cu holds
only the +3V3 and +1V8 pours, and the only obstacles there are drilled items
(vias and through-hole pads of other nets), the board edge and the In2
keepouts of footprints. A grid maze router grows a tree from one target and
adds the nearest unconnected target each round (multi-source Dijkstra), so
the branches share copper. Cells near the board edge and the display cable
slot cost more, so the trunk never pinches a strip of plane off there.
"""
import heapq
import math
import os
import sys
import pcbnew

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from geom import EDGE, TRUNK, SLOT, ISLAND_NOTCH, kicad, in_island, island_outline

MM = pcbnew.FromMM
T = pcbnew.ToMM
BOARD = sys.argv[1]
b = pcbnew.LoadBoard(BOARD)
LAYER = pcbnew.In2_Cu
WIDTH = 0.4
CLR = 0.2          # a little over the class clearance, the pour also needs room
EDGE_CLR = 0.32       # board rule is 0.3 copper to edge
G = 0.1            # grid pitch, mm
L, T0, R, B = EDGE
NX, NY = int((R - L) / G) + 1, int((B - T0) / G) + 1

outline = pcbnew.SHAPE_POLY_SET()
b.GetBoardPolygonOutlines(outline, True)


def cell(x, y):
    return int(round((x - L) / G)), int(round((y - T0) / G))


def xy(i, j):
    return L + i * G, T0 + j * G


blocked = bytearray(NX * NY)
half = WIDTH / 2


def block_segment(grid, t, r):
    x1, y1, x2, y2 = T(t.GetStart().x), T(t.GetStart().y), T(t.GetEnd().x), T(t.GetEnd().y)
    i0, j0 = cell(min(x1, x2) - r, min(y1, y2) - r)
    i1, j1 = cell(max(x1, x2) + r, max(y1, y2) + r)
    dx, dy = x2 - x1, y2 - y1
    ll = dx * dx + dy * dy or 1e-12
    for i in range(max(0, i0), min(NX, i1 + 1)):
        for j in range(max(0, j0), min(NY, j1 + 1)):
            cx, cy = xy(i, j)
            u = max(0.0, min(1.0, ((cx - x1) * dx + (cy - y1) * dy) / ll))
            if (cx - x1 - u * dx) ** 2 + (cy - y1 - u * dy) ** 2 <= r * r:
                grid[j * NX + i] = 1


def block_circle(grid, x, y, r):
    i0, j0 = cell(x - r, y - r)
    i1, j1 = cell(x + r, y + r)
    for i in range(max(0, i0), min(NX, i1 + 1)):
        for j in range(max(0, j0), min(NY, j1 + 1)):
            cx, cy = xy(i, j)
            if (cx - x) ** 2 + (cy - y) ** 2 <= r * r:
                grid[j * NX + i] = 1


# board edge: walk the grid once with the polygon test on a coarse inflate
inner = pcbnew.SHAPE_POLY_SET(outline)
inner.Inflate(-MM(EDGE_CLR + half), pcbnew.CORNER_STRATEGY_ROUND_ALL_CORNERS, MM(0.01))
for j in range(NY):
    for i in range(NX):
        x, y = xy(i, j)
        if not inner.Contains(pcbnew.VECTOR2I(MM(x), MM(y))):
            blocked[j * NX + i] = 1

base = bytes(blocked)


# The trunk should run through the middle of the pour, never along an edge
# or the display cable slot, where it would cut a strip of plane off.
SLOT_K = (*kicad(SLOT[0], SLOT[3]), *kicad(SLOT[2], SLOT[1]))


# Inside the +1V8 island the trunk costs much more, so it goes round the
# island and reaches targets inside it with short dead-end spurs. A path that
# crosses the island from one side to the other would cut a pocket off it.
# Running close along the island edge, on either side, pinches off a strip of
# one pour or the other, so that costs more too. Each crossing of the island
# edge costs a lot, so the tree crosses once per spur instead of weaving.
ISLAND = island_outline()
# The notch is a narrow +3V3 bay walled by the island on three sides. A trunk
# through it cuts the bay off from the rest of the pour.
NX0, NY0, NX1, NY1 = ISLAND_NOTCH


def in_notch(x, y, m=0.3):
    return NX0 - m <= x <= NX1 + m and NY0 - m <= y <= NY1 + m
ISLAND_SEGS = list(zip(ISLAND, ISLAND[1:] + ISLAND[:1]))


def seg_dist(x, y, a, c):
    dx, dy = c[0] - a[0], c[1] - a[1]
    t = max(0.0, min(1.0, ((x - a[0]) * dx + (y - a[1]) * dy) / (dx * dx + dy * dy)))
    return ((x - a[0] - t * dx) ** 2 + (y - a[1] - t * dy) ** 2) ** 0.5


def edge_cost(i, j):
    x, y = xy(i, j)
    d = min(x - L, R - x, y - T0, B - y,
            max(SLOT_K[0] - x, x - SLOT_K[2], SLOT_K[1] - y, y - SLOT_K[3]))
    di = min(seg_dist(x, y, a, c) for a, c in ISLAND_SEGS)
    return (1.0 + 6.0 / (1.0 + max(d, 0.0)) + 6.0 / (1.0 + di)
            + (4.0 if in_island(x, y, -0.3) else 0.0)
            + (40.0 if in_notch(x, y) else 0.0))


COST = [edge_cost(i, j) for j in range(NY) for i in range(NX)]
INSIDE = bytes(1 if in_island(*xy(i, j)) else 0 for j in range(NY) for i in range(NX))
CROSS = 300.0
STEPS = [(1, 0, 1.0), (-1, 0, 1.0), (0, 1, 1.0), (0, -1, 1.0),
         (1, 1, 1.414), (1, -1, 1.414), (-1, 1, 1.414), (-1, -1, 1.414)]


def route_net(name, blocked):
    code = b.FindNet(name).GetNetCode()
    targets = []
    for t in b.GetTracks():
        if t.Type() == pcbnew.PCB_VIA_T:
            p = t.GetPosition()
            r = T(t.GetWidth(pcbnew.F_Cu)) / 2
            if t.GetNetCode() == code:
                targets.append((T(p.x), T(p.y)))
            else:
                block_circle(blocked, T(p.x), T(p.y), r + CLR + half)
        elif t.GetLayer() == LAYER and t.GetNetCode() != code:
            block_segment(blocked, t, T(t.GetWidth()) / 2 + CLR + half)
    for fp in b.GetFootprints():
        for p in fp.Pads():
            if p.GetDrillSize().x == 0:
                continue
            q = p.GetPosition()
            r = max(T(p.GetSize().x), T(p.GetSize().y), T(p.GetDrillSize().x)) / 2
            if p.GetNetCode() == code and p.GetAttribute() != pcbnew.PAD_ATTRIB_NPTH:
                targets.append((T(q.x), T(q.y)))
            else:
                block_circle(blocked, T(q.x), T(q.y), r + CLR + half)
        for z in fp.Zones():
            if z.GetIsRuleArea() and z.GetDoNotAllowTracks() and z.IsOnLayer(LAYER):
                bb = z.GetBoundingBox()
                i0, j0 = cell(T(bb.GetLeft()), T(bb.GetTop()))
                i1, j1 = cell(T(bb.GetRight()), T(bb.GetBottom()))
                for i in range(max(0, i0 - 4), min(NX, i1 + 5)):
                    for j in range(max(0, j0 - 4), min(NY, j1 + 5)):
                        x, y = xy(i, j)
                        if z.Outline().Collide(pcbnew.VECTOR2I(MM(x), MM(y)), MM(half + CLR)):
                            blocked[j * NX + i] = 1
    # open only the target's own cell: the last step into it is short and
    # stays inside the target copper, every other cell keeps its clearance
    for x, y in targets:
        i, j = cell(x, y)
        blocked[j * NX + i] = 0
    tree = {cell(*targets[0])[1] * NX + cell(*targets[0])[0]}
    todo = list({cell(x, y)[1] * NX + cell(x, y)[0] for x, y in targets[1:]} - tree)
    paths = []
    while todo:
        dist = {k: 0.0 for k in tree}
        prev = {}
        heap = [(0.0, k) for k in tree]
        heapq.heapify(heap)
        goal = set(todo)
        hit = None
        while heap:
            d, k = heapq.heappop(heap)
            if d > dist.get(k, 1e18):
                continue
            if k in goal:
                hit = k
                break
            i, j = k % NX, k // NX
            for di, dj, w in STEPS:
                ni, nj = i + di, j + dj
                if not (0 <= ni < NX and 0 <= nj < NY):
                    continue
                nk = nj * NX + ni
                if blocked[nk]:
                    continue
                # a turn costs a little so the result comes out in long straights
                turn = 0.0
                if k in prev:
                    pk = prev[k]
                    if (i - pk % NX, j - pk // NX) != (di, dj):
                        turn = 0.6
                cross = CROSS if INSIDE[nk] != INSIDE[k] else 0.0
                nd = d + w * COST[nk] + turn + cross
                if nd < dist.get(nk, 1e18):
                    dist[nk] = nd
                    prev[nk] = k
                    heapq.heappush(heap, (nd, nk))
        if hit is None:
            raise SystemExit("trunk %s: no path to %s" % (name, [xy(k % NX, k // NX) for k in todo]))
        path = [hit]
        while path[-1] in prev:
            path.append(prev[path[-1]])
        tree.update(path)
        todo.remove(hit)
        paths.append(path[::-1])

    net = b.FindNet(name)
    segs = 0

    def seg(x1, y1, x2, y2):
        t = pcbnew.PCB_TRACK(b)
        t.SetStart(pcbnew.VECTOR2I(MM(x1), MM(y1)))
        t.SetEnd(pcbnew.VECTOR2I(MM(x2), MM(y2)))
        t.SetWidth(MM(WIDTH))
        t.SetLayer(LAYER)
        t.SetNet(net)
        b.Add(t)

    # A path end that is a target lands exactly on its centre, so the track
    # end stays inside the via copper instead of poking out by the grid error.
    exact = {cell(x, y)[1] * NX + cell(x, y)[0]: (x, y) for x, y in targets}
    for path in paths:
        pts = [exact.get(k) or xy(k % NX, k // NX) for k in path]
        corners = [pts[0]]
        for a, c, d in zip(pts, pts[1:], pts[2:]):
            if (round(c[0] - a[0], 3), round(c[1] - a[1], 3)) != (round(d[0] - c[0], 3), round(d[1] - c[1], 3)):
                corners.append(c)
        corners.append(pts[-1])
        for (x1, y1), (x2, y2) in zip(corners, corners[1:]):
            seg(x1, y1, x2, y2)
            segs += 1
    length = sum(math.dist(xy(a % NX, a // NX), xy(c % NX, c // NX)) for p in paths for a, c in zip(p, p[1:]))
    print(f"trunk {name}: {len(targets)} targets, {segs} segments, {length:.1f} mm on In2.Cu")


for name in TRUNK:
    route_net(name, bytearray(base))

pcbnew.SaveBoard(BOARD, b)
