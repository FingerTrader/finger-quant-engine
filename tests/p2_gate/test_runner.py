"""Gate controls only; strategies/numeric goldens are executed independently by Rust."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('p2_gate_runner', ROOT / 'scripts/check_p2_gate.py')
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class Controls(unittest.TestCase):
    def test_incomplete_dependencies_and_unstarted_gate_cannot_pass(self):
        status = '| P1-11 | COMPLETED | evidence |\n' + ''.join(f'| P2-{i:02} | COMPLETED | evidence |\n' for i in range(1, 6)) + '| P2-06 | IN_PROGRESS | actual acceptance |\n'
        gate.prerequisites(status)
        for i in range(1, 6):
            with self.assertRaisesRegex(ValueError, 'incomplete prerequisite'):
                gate.prerequisites(status.replace(f'P2-{i:02} | COMPLETED', f'P2-{i:02} | VERIFIED'))
        with self.assertRaisesRegex(ValueError, 'actively implemented'):
            gate.prerequisites(status.replace('P2-06 | IN_PROGRESS', 'P2-06 | READY'))

    def test_pin_change_missing_path_and_escape_are_rejected(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            (root / 'fixture').write_text('independent bytes')
            digest = 'sha256:' + gate.hashlib.sha256(b'independent bytes').hexdigest()
            gate.checked_file(root, 'fixture', digest)
            for path, pin in [('fixture', 'sha256:' + '0' * 64), ('absent', digest), ('../fixture', digest)]:
                with self.assertRaises(ValueError):
                    gate.checked_file(root, path, pin)

    def test_missing_boundary_or_wrong_suite_version_is_rejected(self):
        suite = json.loads(gate.SUITE.read_bytes())
        gate.validate_suite(ROOT, suite)
        for changed in [dict(suite, cases=suite['cases'][:-1]), dict(suite, schema_version=True), dict(suite, expected_rows=54), dict(suite, source_ids={}), dict(suite, novel_names=[]), dict(suite, minimum_rust_tests=243)]:
            with self.assertRaises(ValueError):
                gate.validate_suite(ROOT, changed)


if __name__ == '__main__':
    unittest.main()
