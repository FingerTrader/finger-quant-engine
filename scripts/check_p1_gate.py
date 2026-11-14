#!/usr/bin/env python3
"""Repeat the P1 gate offline; stdlib only, never regenerate expected fixtures."""
import argparse
import csv
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "crates/fqe-runtime/tests/fixtures/p1"
CALLER = ROOT / "examples/external/caller.py"


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def local(name):
    path = (ROOT / name).resolve()
    require(path.is_relative_to(ROOT), "fixture path escapes repository")
    return path


def sha(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def table(name):
    lines = local(name).read_text().splitlines()
    require(lines[0] == "# fqe.p1-financial-golden/1", "unknown financial table version")
    return list(csv.DictReader(lines[1:], delimiter="\t"))


def scalar(value):
    if value["status"] == "warmup":
        return "W"
    require(value["status"] == "ready", "unexpected financial readiness")
    payload = value["value"]["value"]
    return str(payload).lower() if type(payload) is bool else payload


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--skip-checks", action="store_true", help="process fixtures only; reports partial, never a full gate")
    options = parser.parse_args()
    report = dict(format="fqe.p1-gate-report", schema_version=1,
                  status="running", verified_at=datetime.now(timezone.utc).isoformat(),
                  full_checks=not options.skip_checks, checks=[], cases=[], source_ids={})
    env = dict(os.environ, RUSTUP_AUTO_INSTALL="0", PYTHONDONTWRITEBYTECODE="1")

    def command(label, args, timeout=180):
        result = subprocess.run(args, cwd=ROOT, env=env, capture_output=True, timeout=timeout)
        entry = dict(name=label, exit=result.returncode)
        if label == "rust-tests":
            text = result.stdout.decode(errors="replace")
            entry["passed"] = sum(int(n) for n in re.findall(r"test result: ok\. (\d+) passed", text))
            require(entry["passed"] >= 181, "required P1 Rust tests are missing")
        if label == "external-tests":
            text = result.stderr.decode(errors="replace")
            count = re.search(r"Ran (\d+) tests", text)
            require(count is not None and int(count[1]) >= 23, "required external tests are missing")
            entry["passed"] = int(count[1])
        report["checks"].append(entry)
        require(result.returncode == 0, label + ": " + (result.stdout + result.stderr).decode(errors="replace")[-3000:])
        print("PASS " + label, file=sys.stderr, flush=True)

    try:
        suite = json.loads((FIXTURES / "suite.json").read_bytes())
        require(suite["format"] == "fqe.p1-golden-suite" and type(suite["schema_version"]) is int and suite["schema_version"] == 1, "unsupported suite version")
        for name, expected in suite["source_ids"].items():
            actual = sha(local(name).read_bytes())
            require(actual == expected, "fixture/source pin changed: " + name)
            report["source_ids"][name] = actual
        artifact = (FIXTURES / "provider-source.sha256").read_text().strip()
        require(artifact == sha(local(suite["provider_source"]).read_bytes()), "test provider artifact pin differs from source")
        require(set(suite["inventory"]) == {"G-001", "G-002", "G-003", "G-004", "G-005", "G-006", "G-007", "X-001"}, "incomplete gate inventory")
        require(suite["inventory"]["G-006"]["status"] == "deferred-to-P2-06", "workflow gate must remain separate")
        for item in suite["inventory"].values():
            require(all(local(name).is_file() for name in item["evidence"]), "inventory evidence is missing")
        for crate in (ROOT / "crates").iterdir():
            for path in (crate / "src").rglob("*.rs"):
                source = path.read_text()
                require(all(name not in source for name in suite["novel_names"]), "test provider/rule ID hardcoded in production: " + str(path.relative_to(ROOT)))
        report["checks"].append(dict(name="source-pins-and-generic-dispatch", exit=0))
        if not options.skip_checks:
            for label, args in [
                ("rust-fmt", ["cargo", "fmt", "--check"]),
                ("rust-check", ["cargo", "check", "--workspace", "--locked", "--offline"]),
                ("rust-tests", ["cargo", "test", "--workspace", "--locked", "--offline"]),
                ("rust-clippy", ["cargo", "clippy", "--workspace", "--all-targets", "--locked", "--offline", "--", "-D", "warnings"]),
                ("cli-build", ["cargo", "build", "--locked", "--offline", "-p", "fqe-cli"]),
                ("external-tests", [sys.executable, "-m", "unittest", "discover", "-s", "tests/external", "-v"]),
            ]:
                command(label, args)
        engine = ROOT / "target/debug/fqe"
        require(engine.is_file(), "build fqe first; no implicit installation")
        golden = table(suite["financial_table"])
        child_env = dict(PATH="/nonexistent", PYTHONDONTWRITEBYTECODE="1")
        with tempfile.TemporaryDirectory(prefix="fqe-p1-gate-") as directory:
            for case in suite["cases"]:
                label = case["name"]
                model, data = local(case["model"]), local(case["data"])
                params = local(case["parameters"]) if case["parameters"] else None
                request = dict(format="fqe.invoke-request", schema_version=1, request_id=label,
                               operation="run", model=str(model), data=str(data), parameters=str(params) if params else None,
                               run_id=label, timeout_ms=5000)
                path = Path(directory) / (label + ".json")
                path.write_text(json.dumps(request))
                direct_args = [str(engine), "run", "--model", str(model), "--data", str(data), "--run-id", label]
                if params:
                    direct_args += ["--params", str(params)]
                direct = subprocess.run(direct_args, cwd=directory, env=child_env, capture_output=True, timeout=8)
                caller_args = [sys.executable, str(CALLER), "--engine", str(engine), "--request", str(path)]
                first = subprocess.run(caller_args, cwd=directory, env=child_env, capture_output=True, timeout=8)
                again = subprocess.run(caller_args, cwd=directory, env=child_env, capture_output=True, timeout=8)
                require(first.returncode == again.returncode and first.stdout == again.stdout and first.stderr == again.stderr, "nondeterministic caller: " + label)
                if "engine_error" in case:
                    require(direct.returncode == case["engine_error"] and not direct.stdout and first.returncode == 11 and not first.stdout, "partial or wrong engine failure: " + label)
                    packet = json.loads(first.stderr)
                    diagnostic = json.loads(direct.stderr)
                    require(packet["status"] == "engine_error" and packet["result"] is None and packet["diagnostic"] == diagnostic and diagnostic["code"] == case["code"], "diagnostic parity: " + label)
                    report["cases"].append(dict(name=label, status="expected-engine-error", code=diagnostic["code"]))
                    continue
                require(direct.returncode == 0 and first.returncode == 0 and not direct.stderr and not first.stderr, "process failure: " + label)
                packet = json.loads(first.stdout)
                result = json.loads(direct.stdout)
                require(packet["status"] == "ok" and packet["result"] == result, "CLI/external result mismatch: " + label)
                manifest = result["manifest"]
                require(manifest["artifact_source_id"] == sha(model.read_bytes()) and manifest["dataset_id"] == sha(data.read_bytes()), "actual source hash mismatch")
                require(manifest["parameters_source_id"] == (sha(params.read_bytes()) if params else None), "parameter hash mismatch")
                require(result["summary"]["status"] == "completed", "missing completed result")
                rows = result["observations"]
                signals = [signal for row in rows for signal in row["signals"]]
                require([s["source_sequence"] for s in signals] == case["signals"], "independent signal golden: " + label)
                for signal in signals:
                    require(signal["timestamp_ns"] == str(int(signal["source_sequence"]) * 1000) or label == "full-width", "signal timestamp")
                    require(signal["instrument"] == "DEMO" and signal["kind"] == "long", "signal provenance")
                expected = [r for r in golden if r["case"] == case.get("table_case")]
                if expected:
                    require(len(rows) == len(expected), "financial table length")
                    for row, want in zip(rows, expected):
                        require(row["source_sequence"] == want["sequence"] and str(row["warmup"]).lower() == want["warmup"], "event readiness/provenance")
                        require(scalar(row["outputs"]["decision"]) == want["decision"] and str(len(row["signals"])) == want["signals"], "financial decision golden")
                        if want["rsi"] != "-":
                            rsi = row["outputs"]["rsi"]
                            require(scalar(rsi) == want["rsi"], "independent rational RSI golden")
                            if rsi["status"] == "ready":
                                require(rsi["value"]["type"] == dict(version=1, shape="scalar", kind="decimal", scale=12, unit=dict(kind="ratio")), "RSI numeric policy")
                if label == "full-width":
                    require(rows[0]["source_sequence"] == "9007199254740993" and rows[1]["outputs"]["price"]["value"]["value"] == "18446744073709551615", "integer precision lost")
                if label == "empty":
                    require(not rows and result["summary"]["processed_events"] == "0" and result["summary"]["last_sequence"] is None, "explicit empty run")
                report["cases"].append(dict(name=label, status="passed", execution_id=manifest["execution_id"], signals=case["signals"]))
        identities = {c["name"]: c.get("execution_id") for c in report["cases"]}
        require(identities["g003-default"] == identities["g003-60"], "explicit default must preserve execution identity")
        require(identities["g003-default"] != identities["g003-90"] and identities["g003-default"] != identities["g003-model90"], "changed binding/artifact must change execution identity")
        if not options.skip_checks:
            command("docs", [sys.executable, "scripts/check_docs.py"])
            command("diff", ["git", "diff", "--check"])
        report["status"] = "partial" if options.skip_checks else "passed"
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.TimeoutExpired) as error:
        report["status"] = "failed"
        report["error"] = str(error)
    print(json.dumps(report, ensure_ascii=False, sort_keys=True, separators=(",", ":")))
    return 1 if report["status"] == "failed" else 0


if __name__ == "__main__":
    raise SystemExit(main())
