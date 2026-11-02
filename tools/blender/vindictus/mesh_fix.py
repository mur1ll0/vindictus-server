"""Mesh fixes that need no Blender: the arithmetic of the Push Mesh Out operator (blender_io.push_out).

Some NA armour pieces sit partly inside the body they cover: in the bind pose the skin (the Miri's and the 1.69
Fiona's inner) shows through the pytonshock suit and the Dragon Rise sleeves, and the 1.69 client draws it so.
Moving every vertex of the piece PUSH_OUT game units along its game normal covered the skin (Blender renders of the
client's own kits; docs/future/miri-creation-grid.md 10). scripts/client/stage_na_class.inflated_armor does the
same to the NA armour it stages (ARMOR_INFLATE)."""
from __future__ import annotations

import numpy as np

PUSH_OUT = 0.3  # game units; stage_na_class.ARMOR_INFLATE


def vertex_normals(vertex_count: int, loop_vertex, corner_normals) -> np.ndarray:
    """Unit normal of each vertex: the mean of its corner (game) normals; zero for a vertex no corner uses."""
    normals = np.zeros((vertex_count, 3))
    np.add.at(normals, np.asarray(loop_vertex), np.asarray(corner_normals, dtype=float).reshape(-1, 3))
    length = np.linalg.norm(normals, axis=1, keepdims=True)
    return np.divide(normals, length, out=np.zeros_like(normals), where=length > 0)


def pushed_out(positions, normals, offset: float) -> np.ndarray:
    """Positions moved `offset` (in the positions' units) along the normals."""
    return np.asarray(positions, dtype=float).reshape(-1, 3) + np.asarray(normals, dtype=float) * offset
