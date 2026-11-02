"""Push Mesh Out arithmetic; run with the format tests (Blender's Python ships numpy)."""
import unittest

import numpy as np

import mesh_fix


class PushOutTest(unittest.TestCase):
    def test_vertices_move_along_the_mean_of_their_corner_normals(self):
        # vertex 0: two corners pointing up and sideways; vertex 1: one corner up; vertex 2: no corner
        loop_vertex = [0, 0, 1]
        corners = [(0.0, 0.0, 1.0), (1.0, 0.0, 0.0), (0.0, 0.0, 2.0)]
        normals = mesh_fix.vertex_normals(3, loop_vertex, corners)
        np.testing.assert_allclose(normals[0], (2 ** -0.5, 0.0, 2 ** -0.5))
        np.testing.assert_allclose(normals[1], (0.0, 0.0, 1.0))
        np.testing.assert_allclose(normals[2], (0.0, 0.0, 0.0))
        moved = mesh_fix.pushed_out([(0, 0, 0), (1, 1, 1), (5, 5, 5)], normals, mesh_fix.PUSH_OUT)
        np.testing.assert_allclose(moved[1], (1.0, 1.0, 1.0 + mesh_fix.PUSH_OUT))
        np.testing.assert_allclose(moved[2], (5.0, 5.0, 5.0))
        np.testing.assert_allclose(np.linalg.norm(moved[0]), mesh_fix.PUSH_OUT)

    def test_the_default_is_the_staging_offset(self):
        self.assertEqual(mesh_fix.PUSH_OUT, 0.3)


if __name__ == '__main__':
    unittest.main()
