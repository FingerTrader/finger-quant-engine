"""Independent consumer tests: real fqe plus trusted, purpose-built fault processes.
Run after `cargo build --locked --offline -p fqe-cli`, stdlib only.
"""
import copy
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
CALLER = ROOT / "examples/external/caller.py"
REQUESTS = CALLER.parent / "requests"
SPEC = importlib.util.spec_from_file_location("external_caller", CALLER)
caller = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(caller)


class ExternalCaller(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.engine = Path(os.environ.get("FQE_ENGINE", ROOT / "target/debug/fqe")).resolve()
        if not cls.engine.is_file():
            raise RuntimeError("Build fqe first: cargo build --locked --offline -p fqe-cli")
        cls.baseline = caller.load_request(REQUESTS / "g002.json")
        cls.valid_bytes = cls.direct(cls.baseline).stdout
        cls.valid = json.loads(cls.valid_bytes)

    @staticmethod
    def direct(request):
        args = [str(ExternalCaller.engine), request["operation"], "--model", str(request["model"])]
        if request["parameters"] is not None:
            args += ["--params", str(request["parameters"])]
        if request["operation"] == "run":
            args += ["--data", str(request["data"]), "--run-id", request["run_id"]]
        result = subprocess.run(args, capture_output=True, timeout=5, env={"PATH": "/nonexistent"})
        return result

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="fqe-p110-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.serial = 0

    def request(self, **changes):
        value = dict(self.baseline)
        value.update(changes)
        value = {key: str(item) if isinstance(item, Path) else item for key, item in value.items()}
        self.serial += 1
        path = self.directory / f"request-{self.serial}.json"
        path.write_text(json.dumps(value))
        return path

    def engine_script(self, body):
        self.serial += 1
        path = self.directory / f"stub engine {self.serial}.py"
        pid = self.directory / f"pid-{self.serial}"
        path.write_text(f"#!{sys.executable}\nimport os,sys,time,subprocess\nfrom pathlib import Path\nPath({str(pid)!r}).write_text(str(os.getpid()))\n{body}\n")
        path.chmod(0o700)
        return path, pid

    def stub(self, stdout=b"", stderr=b"", status=0):
        return self.engine_script(f"os.write(1,{stdout!r})\nos.write(2,{stderr!r})\nsys.exit({status})")

    def gone(self, pid_file):
        self.assertTrue(pid_file.is_file())
        pid = int(pid_file.read_text())
        deadline = time.monotonic() + 2
        while time.monotonic() < deadline:
            try:
                os.kill(pid, 0)
            except ProcessLookupError:
                return
            time.sleep(0.01)
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        self.fail("child process remains visible after cleanup")

    def execute_caller(self, request, engine=None, cwd=None):
        return subprocess.run([sys.executable, str(CALLER), "--engine", str(engine or self.engine), "--request", str(request)], capture_output=True, timeout=8, cwd=cwd, env={"PATH": "/nonexistent"})

    def assert_error(self, packet, status, code=None):
        self.assertEqual(status, 12)
        self.assertEqual(packet["status"], "caller_error")
        self.assertIsNone(packet["result"])
        if code:
            self.assertEqual(packet["error"]["code"], code)

    def test_real_reference_models_match_direct_cli_and_independent_goldens(self):
        for name, expected in [("g002", ["5"]), ("g002-high", []), ("g007", ["5"])]:
            with self.subTest(name=name):
                request = REQUESTS / (name + ".json")
                completed = self.execute_caller(request)
                self.assertEqual(completed.returncode, 0, completed.stderr)
                self.assertEqual(completed.stderr, b"")
                packet = json.loads(completed.stdout)
                self.assertEqual(packet["format"], "fqe.invoke-response")
                self.assertEqual(packet["schema_version"], 1)
                result = packet["result"]
                signals = [s for o in result["observations"] for s in o["signals"]]
                self.assertEqual([s["source_sequence"] for s in signals], expected)
                self.assertTrue(all(s["timestamp_ns"] == "5000" and s["kind"] == "long" and s["instrument"] == "DEMO" for s in signals))
                if name == "g002":
                    self.assertEqual(result["observations"][4]["outputs"]["rsi"]["value"]["value"], "83333333333333")
                direct = self.direct(caller.load_request(request))
                self.assertEqual(direct.returncode, 0)
                self.assertEqual(result, json.loads(direct.stdout))
                self.assertEqual(completed.stdout, self.execute_caller(request).stdout)

    def test_changed_model_artifact_changes_decisions_without_engine_rebuild(self):
        changed = self.request(model=ROOT / "crates/fqe-cli/tests/fixtures/g002-high-model.json", parameters=None)
        result = self.execute_caller(changed)
        self.assertEqual(result.returncode, 0, result.stderr)
        packet = json.loads(result.stdout)
        self.assertEqual(packet["result"]["summary"]["signals"], "0")
        self.assertNotEqual(packet["result"]["manifest"]["execution_id"], self.valid["manifest"]["execution_id"])

    def test_validate_uses_same_versioned_process_boundary(self):
        request = REQUESTS / "validate.json"
        result = self.execute_caller(request)
        self.assertEqual(result.returncode, 0, result.stderr)
        value = json.loads(result.stdout)["result"]
        self.assertEqual(value["status"], "valid")
        self.assertNotIn("observations", value)
        self.assertEqual(value, json.loads(self.direct(caller.load_request(request)).stdout))

    def test_engine_type_limits_and_names_are_preserved_by_consumer(self):
        request = caller.load_request(REQUESTS / "tick.json")
        model = json.loads(request["model"].read_bytes())
        model["outputs"][0]["name"] = "rule:decision/v1"
        model["signals"][0]["output"] = "rule:decision/v1"
        payloads = {
            "extension:money/v1": {
                "type": {"version": 1, "shape": "scalar", "kind": "decimal", "scale": 18,
                         "unit": {"kind": "money", "currency": "TEST123456789012"}},
                "value": str(-(2**127)),
            },
            "extension:signed/v1": {
                "type": {"version": 1, "shape": "scalar", "kind": "i64"},
                "value": str(-(2**63)),
            },
            "extension:text/v1": {
                "type": {"version": 1, "shape": "scalar", "kind": "text", "max_bytes": 6},
                "value": "量化",
            },
        }
        model["outputs"].extend({"name": name, "expression": {"literal": value}}
                                for name, value in payloads.items())
        source = self.directory / "types.json"
        source.write_text(json.dumps(model))
        request["model"] = source
        path = self.request(**request)
        completed = self.execute_caller(path)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        result = json.loads(completed.stdout)["result"]
        self.assertEqual(result, json.loads(self.direct(caller.load_request(path)).stdout))
        for name, value in payloads.items():
            self.assertEqual(result["observations"][1]["outputs"][name],
                             {"status": "ready", "value": value})
        request.update(operation="validate", data=None, run_id=None)
        validation = self.execute_caller(self.request(**request))
        self.assertEqual(validation.returncode, 0, validation.stderr)

    def test_declared_warmup_is_checked_without_evaluating_indicators(self):
        request = caller.load_request(REQUESTS / "tick.json")
        model = json.loads(request["model"].read_bytes())
        model["warmup_samples"] = "1"
        source = self.directory / "warmup.json"
        source.write_text(json.dumps(model))
        request["model"] = source
        path = self.request(**request)
        completed = self.execute_caller(path)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        result = json.loads(completed.stdout)["result"]
        self.assertTrue(result["observations"][1]["warmup"])
        self.assertEqual(result["summary"]["signals"], "0")
        result["observations"][1]["warmup"] = False
        engine, pid = self.stub(stdout=json.dumps(result).encode() + b"\n")
        self.assert_error(*caller.invoke(engine, path), "protocol")
        self.gone(pid)

    def test_full_width_integers_and_explicit_empty_inputs(self):
        tick = self.execute_caller(REQUESTS / "tick.json")
        self.assertEqual(tick.returncode, 0, tick.stderr)
        value = json.loads(tick.stdout)["result"]
        self.assertEqual(value["summary"]["last_sequence"], str(2**64 - 1))
        self.assertEqual(value["observations"][1]["outputs"]["price"]["value"]["value"], str(2**64 - 1))
        self.assertEqual(value["observations"][0]["source_sequence"], "9007199254740993")
        self.assertEqual(value["summary"]["matched_events"], "1")
        empty = self.execute_caller(REQUESTS / "empty.json")
        self.assertEqual(empty.returncode, 0, empty.stderr)
        self.assertEqual(json.loads(empty.stdout)["result"]["observations"], [])

    def test_real_engine_failure_is_structured_and_not_a_partial_success(self):
        completed = self.execute_caller(REQUESTS / "engine-error.json")
        self.assertEqual(completed.returncode, 11)
        self.assertEqual(completed.stdout, b"")
        packet = json.loads(completed.stderr)
        self.assertEqual(packet["status"], "engine_error")
        self.assertEqual(packet["engine_exit"], 4)
        self.assertEqual(packet["diagnostic"]["code"], "expression_numeric")
        self.assertIsNone(packet["result"])

    def test_request_versions_fields_duplicates_and_numbers_are_strict(self):
        for changes in [dict(schema_version=2), dict(schema_version=True), dict(format="other"), dict(operation="shell"), dict(run_id="bad id"), dict(timeout_ms=99), dict(timeout_ms=60001), dict(timeout_ms=True), dict(model=None), dict(data=None), dict(extra="secret")]:
            with self.subTest(changes=changes):
                packet, status = caller.invoke(self.engine, self.request(**changes))
                self.assertEqual(status, 10)
                self.assertEqual(packet["error"]["code"], "request")
        path = self.directory / "duplicate.json"
        path.write_bytes(b'{"format":"one","format":"two"}')
        packet, status = caller.invoke(self.engine, path)
        self.assertEqual(status, 10)
        self.assertIsNone(packet["request_id"])

    def test_request_and_source_files_have_byte_and_regular_file_limits(self):
        path = self.directory / "large-request"
        path.write_bytes(b" " * (caller.MAX_REQUEST + 1))
        packet, status = caller.invoke(self.engine, path)
        self.assertEqual(status, 10)
        self.assertEqual(packet["error"]["code"], "file_limit")
        source = self.directory / "large-data"
        source.write_bytes(b" " * (caller.MAX_FILE + 1))
        packet, status = caller.invoke(self.engine, self.request(data=source))
        self.assertEqual(status, 10)
        self.assertEqual(packet["error"]["code"], "file_limit")
        for source in [self.directory, self.directory / "absent"]:
            packet, status = caller.invoke(self.engine, self.request(model=source))
            self.assertEqual(status, 10)
            self.assertEqual(packet["error"]["code"], "file_io")

    def test_malformed_actual_input_is_engine_error_not_caller_success(self):
        source = self.directory / "bad-model"
        source.write_text('{"private":"DO_NOT_LEAK_SECRET"}')
        packet, status = caller.invoke(self.engine, self.request(model=source))
        self.assertEqual(status, 11)
        self.assertEqual(packet["engine_exit"], 3)
        self.assertNotIn("DO_NOT_LEAK_SECRET", json.dumps(packet))

    def test_relative_unicode_and_shell_metacharacter_paths_are_literal(self):
        folder = self.directory / "本地 请求"
        folder.mkdir()
        model = folder / "model $(not-a-command).json"
        data = folder / "行情 ; plain.json"
        model.write_bytes((ROOT / "crates/fqe-cli/tests/fixtures/tick-model.json").read_bytes())
        data.write_bytes((ROOT / "crates/fqe-cli/tests/fixtures/tick-data.json").read_bytes())
        request = json.loads((REQUESTS / "tick.json").read_text())
        request.update(model=model.name, data=data.name)
        path = folder / "请求.json"
        path.write_text(json.dumps(request))
        completed = self.execute_caller(path, cwd=self.directory)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(json.loads(completed.stdout)["result"]["summary"]["signals"], "1")

    def test_missing_executable_returns_bounded_caller_error(self):
        packet, status = caller.invoke(self.directory / "missing engine", self.request())
        self.assert_error(packet, status, "launch")

    def test_supported_nonzero_diagnostic_is_preserved(self):
        diagnostic = dict(format="fqe.diagnostic", schema_version=1, stage="compile", code="registry_entry", path="$.graph", message="model could not be compiled")
        engine, pid = self.stub(stderr=json.dumps(diagnostic).encode() + b"\n", status=3)
        packet, status = caller.invoke(engine, self.request())
        self.assertEqual(status, 11)
        self.assertEqual(packet["diagnostic"], diagnostic)
        self.gone(pid)

    def test_nonzero_unknown_exit_or_conflicting_streams_cannot_be_success(self):
        for stdout, stderr, status in [(self.valid_bytes, b"", 3), (b"", b"", 7), (b"", b"DO_NOT_LEAK_SECRET", 3), (self.valid_bytes, b"warning\n", 0)]:
            with self.subTest(status=status):
                engine, pid = self.stub(stdout, stderr, status)
                packet, result_status = caller.invoke(engine, self.request())
                self.assert_error(packet, result_status)
                self.assertNotIn("DO_NOT_LEAK_SECRET", json.dumps(packet))
                self.gone(pid)

    def test_framing_json_unknown_versions_and_duplicate_keys_are_rejected(self):
        payloads = [b"", self.valid_bytes[:-1], self.valid_bytes + b"\n", self.valid_bytes[:-4], b'{"format":"x","format":"x"}\n', b'{"x":NaN}\n', b'{"x":1.0}\n', b'\xff\n', b'[' * 33 + b'0' + b']' * 33 + b'\n']
        for payload in payloads:
            with self.subTest(payload=payload[:30]):
                engine, pid = self.stub(stdout=payload)
                packet, status = caller.invoke(engine, self.request())
                self.assert_error(packet, status, "protocol")
                self.gone(pid)

    def test_result_structure_types_readiness_counts_and_provenance_are_checked(self):
        mutations = [
            lambda v: v.update(schema_version=2),
            lambda v: v.update(schema_version=True),
            lambda v: v.update(unexpected=True),
            lambda v: v["manifest"].update(run="wrong"),
            lambda v: v["manifest"].update(artifact_id="sha256:" + "A" * 64),
            lambda v: v["manifest"]["trigger"].update(price_scale=True),
            lambda v: v["observations"][0].update(source_sequence=1),
            lambda v: v["observations"][0].update(source_sequence="01"),
            lambda v: v["observations"][0].update(matched=1),
            lambda v: v["observations"][4].update(warmup=True),
            lambda v: v["observations"][0]["outputs"]["rsi"].update(status="error"),
            lambda v: v["observations"][4]["outputs"]["rsi"]["value"].update(value="083333333333333"),
            lambda v: v["observations"][4]["outputs"]["rsi"]["value"]["type"].update(scale=19),
            lambda v: v["observations"][4].update(signals=[]),
            lambda v: v["summary"].update(processed_events="5"),
            lambda v: v["summary"].update(signals="0"),
            lambda v: v["summary"].update(last_sequence="5"),
        ]
        for mutate in mutations:
            changed = copy.deepcopy(self.valid)
            mutate(changed)
            engine, pid = self.stub(stdout=json.dumps(changed, separators=(",", ":")).encode() + b"\n")
            packet, status = caller.invoke(engine, self.request())
            self.assert_error(packet, status, "protocol")
            self.gone(pid)
        changed = copy.deepcopy(self.valid)
        changed["manifest"]["dataset_id"] = "sha256:" + "0" * 64
        engine, pid = self.stub(stdout=json.dumps(changed).encode() + b"\n")
        self.assert_error(*caller.invoke(engine, self.request()), "input_changed")
        self.gone(pid)

    def test_file_change_before_engine_read_is_detected_by_actual_hash(self):
        params = self.directory / "parameters.json"
        params.write_bytes((ROOT / "crates/fqe-cli/tests/fixtures/parameters-60.json").read_bytes())
        high = (ROOT / "crates/fqe-cli/tests/fixtures/parameters-90.json").read_bytes()
        body = f"Path({str(params)!r}).write_bytes({high!r})\nos.execv({str(self.engine)!r},[{str(self.engine)!r},*sys.argv[1:]])"
        engine, pid = self.engine_script(body)
        packet, status = caller.invoke(engine, self.request(parameters=params))
        self.assert_error(packet, status, "input_changed")
        self.gone(pid)

    def test_stdout_and_stderr_floods_are_bounded_and_processes_reaped(self):
        for fd in (1, 2):
            with self.subTest(fd=fd):
                engine, pid = self.engine_script(f"while True: os.write({fd},b'x'*65536)")
                packet, status = caller.invoke(engine, self.request(timeout_ms=2000))
                self.assert_error(packet, status, "output_limit")
                self.gone(pid)

    def test_timeout_terminates_and_reaps_sleeping_engine(self):
        engine, pid = self.engine_script("time.sleep(30)")
        started = time.monotonic()
        packet, status = caller.invoke(engine, self.request(timeout_ms=1500))
        self.assert_error(packet, status, "timeout")
        self.assertLess(time.monotonic() - started, 3)
        self.gone(pid)

    def test_closed_pipes_do_not_remove_the_process_exit_deadline(self):
        engine, pid = self.engine_script("os.close(1)\nos.close(2)\ntime.sleep(30)")
        packet, status = caller.invoke(engine, self.request(timeout_ms=1500))
        self.assert_error(packet, status, "timeout")
        self.gone(pid)

    def test_inherited_pipe_descendant_is_killed_with_owned_process_group(self):
        child_pid = self.directory / "descendant-pid"
        child_code = f"from pathlib import Path;import os,time;Path({str(child_pid)!r}).write_text(str(os.getpid()));time.sleep(30)"
        body = f"subprocess.Popen([sys.executable,'-c',{child_code!r}])\nwhile not Path({str(child_pid)!r}).exists(): time.sleep(.01)"
        engine, pid = self.engine_script(body)
        packet, status = caller.invoke(engine, self.request(timeout_ms=1500))
        self.assert_error(packet, status, "timeout")
        self.gone(pid)
        self.gone(child_pid)

    def test_unsupported_platform_fails_before_launch(self):
        with patch.object(caller.os, "name", "nt"):
            with self.assertRaises(caller.CallerError) as error:
                caller.capture(self.engine, [], 100)
            self.assertEqual(error.exception.code, "platform")

    def test_caller_output_pipe_failure_exits_without_shutdown_traceback(self):
        for name, stream in (("g002", "stdout"), ("engine-error", "stderr")):
            with self.subTest(stream=stream):
                read_fd, write_fd = os.pipe()
                os.close(read_fd)
                options = {"stdout": subprocess.PIPE, "stderr": subprocess.PIPE}
                options[stream] = write_fd
                try:
                    completed = subprocess.run(
                        [sys.executable, str(CALLER), "--engine", str(self.engine),
                         "--request", str(REQUESTS / (name + ".json"))],
                        timeout=8, env={"PATH": "/nonexistent"}, **options)
                finally:
                    os.close(write_fd)
                self.assertEqual(completed.returncode, 12)
                self.assertEqual(getattr(completed, "stderr" if stream == "stdout" else "stdout"), b"")


if __name__ == "__main__":
    unittest.main()
