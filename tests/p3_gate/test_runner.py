"""Financial gate controls; numeric and pipeline outcomes execute separately in real Rust."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('p3_gate',ROOT/'scripts/check_p3_gate.py');gate=importlib.util.module_from_spec(spec);spec.loader.exec_module(gate)
class Controls(unittest.TestCase):
    def test_incomplete_financial_dependencies_and_unstarted_gate_reject(self):
        s=''.join(f'| {t} | COMPLETED | evidence |\n' for t in ['P1-11','P2-06']+[f'P3-{n:02}' for n in range(1,7)])+'| P3-07 | IN_PROGRESS | actual gate |\n';gate.prerequisites(s)
        for n in range(1,7):
            with self.assertRaisesRegex(ValueError,'incomplete prerequisite'):gate.prerequisites(s.replace(f'P3-{n:02} | COMPLETED',f'P3-{n:02} | VERIFIED'))
        with self.assertRaisesRegex(ValueError,'actively implemented'):gate.prerequisites(s.replace('P3-07 | IN_PROGRESS','P3-07 | READY'))
    def test_changed_missing_escaping_pin_reject(self):
        with tempfile.TemporaryDirectory() as name:
            root=Path(name);(root/'fixture').write_bytes(b'actual independent bytes');pin='sha256:'+gate.hashlib.sha256(b'actual independent bytes').hexdigest();gate.checked_file(root,'fixture',pin)
            for path,p in [('fixture','sha256:'+'0'*64),('absent',pin),('../fixture',pin)]:
                with self.assertRaises(ValueError):gate.checked_file(root,path,p)
    def test_incomplete_models_or_oracles_or_suite_cannot_pass(self):
        suite=json.loads(gate.SUITE.read_bytes());gate.validate_suite(ROOT,suite)
        for bad in [dict(suite,cases=suite['cases'][:-1]),dict(suite,schema_version=True),dict(suite,minimum_rust_tests=323),dict(suite,independent_cases={}),dict(suite,source_ids={})]:
            with self.assertRaises(ValueError):gate.validate_suite(ROOT,bad)
if __name__=='__main__':unittest.main()
