#!/usr/bin/env python3
"""Independent POSIX reference consumer of fqe CLI JSON v1; Python stdlib only.

No engine modules are imported. The chosen executable and local files are trusted;
this is bounded process orchestration and contract validation, not a sandbox.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import signal
import stat
import subprocess
import sys
import time

MAX_FILE = 262144
MAX_REQUEST = 16384
MAX_STDOUT = 4 * 1024 * 1024 + 1
MAX_STDERR = 16384
ID = re.compile(r"[A-Za-z0-9._-]{1,128}\Z")
INSTRUMENT = re.compile(r"[A-Za-z0-9:/._-]{1,128}\Z")
HASH = re.compile(r"sha256:[0-9a-f]{64}\Z")
INTEGER = re.compile(r"(?:0|[1-9][0-9]*|-[1-9][0-9]*)\Z")
U64 = 2**64 - 1
MESSAGES = {
    "request": "invalid or unsupported invocation request",
    "file_io": "cannot read the requested regular local file",
    "file_limit": "local input exceeds its byte limit",
    "platform": "this reference process supervisor requires POSIX",
    "launch": "cannot start the selected engine executable",
    "timeout": "engine process exceeded its deadline",
    "output_limit": "engine process output exceeds its byte limit",
    "process": "engine process did not produce a supported response",
    "protocol": "engine response does not satisfy the versioned contract",
    "input_changed": "engine source identities differ from the supplied files",
    "cleanup": "engine child could not be reaped within the cleanup deadline",
    "output_io": "cannot write complete caller response",
}


class CallerError(Exception):
    def __init__(self, code):
        self.code = code
        super().__init__(MESSAGES[code])


def require(condition, code="protocol"):
    if not condition:
        raise CallerError(code)


def fields(value, names):
    require(type(value) is dict and set(value) == set(names.split()))
    return value


def number(value, lo=0, hi=U64):
    require(type(value) is str and len(value) <= 40 and INTEGER.fullmatch(value))
    n = int(value)
    require(lo <= n <= hi)
    return n


def small(value, lo, hi):
    require(type(value) is int and lo <= value <= hi)
    return value


def identifier(value, pattern=ID):
    require(type(value) is str and pattern.fullmatch(value))
    return value


def digest(value):
    require(type(value) is str and HASH.fullmatch(value))
    return value


def raw_digest(value):
    return "sha256:" + hashlib.sha256(value).hexdigest()


def read_file(path, limit):
    try:
        require(stat.S_ISREG(path.stat().st_mode), "file_io")
        require(path.stat().st_size <= limit, "file_limit")
        with path.open("rb") as stream:
            require(stat.S_ISREG(os.fstat(stream.fileno()).st_mode), "file_io")
            data = stream.read(limit + 1)
        require(len(data) <= limit, "file_limit")
        return data
    except OSError:
        raise CallerError("file_io") from None


def decode(data, budget=1048576):
    """Bound nesting before json.loads; reject duplicate keys/nonexact numbers."""
    try:
        depth = 0
        quoted = escaped = False
        for byte in data:
            if quoted:
                if escaped:
                    escaped = False
                elif byte == 92:
                    escaped = True
                elif byte == 34:
                    quoted = False
            elif byte == 34:
                quoted = True
            elif byte in (91, 123):
                depth += 1
                require(depth <= 32)
            elif byte in (93, 125):
                depth -= 1
                require(depth >= 0)
        def pairs(items):
            result = {}
            for key, value in items:
                require(key not in result)
                result[key] = value
            return result
        def integer(value):
            require(len(value) <= 10 and value.isascii() and value.isdecimal())
            return small(int(value), 0, 2**32 - 1)
        def reject(_value):
            raise CallerError("protocol")
        result = json.loads(data.decode("utf-8"), object_pairs_hook=pairs,
                            parse_int=integer, parse_float=reject, parse_constant=reject)
        pending = [(result, 0)]
        count = 0
        while pending:
            value, depth = pending.pop()
            count += 1
            require(count <= budget and depth <= 32)
            if type(value) is str:
                require(len(value.encode("utf-8")) <= 4096)
            elif type(value) is dict:
                require(len(value) <= 1024)
                for key, item in value.items():
                    require(len(key.encode("utf-8")) <= 128)
                    pending.append((item, depth + 1))
            elif type(value) is list:
                require(len(value) <= 1024)
                pending.extend((item, depth + 1) for item in value)
        return result
    except (ValueError, UnicodeError, RecursionError):
        raise CallerError("protocol") from None


def load_request(path):
    try:
        request = fields(decode(read_file(path, MAX_REQUEST), 256),
                         "format schema_version request_id operation model parameters data run_id timeout_ms")
        require(request["format"] == "fqe.invoke-request")
        require(type(request["schema_version"]) is int and request["schema_version"] == 1)
        identifier(request["request_id"])
        require(request["operation"] in ("validate", "run"))
        small(request["timeout_ms"], 100, 60000)
        for name in ("model", "parameters", "data"):
            value = request[name]
            if value is not None:
                require(type(value) is str and 0 < len(value.encode("utf-8")) <= 4096 and "\0" not in value)
                request[name] = (path.parent / value).resolve()
        require(request["model"] is not None)
        if request["operation"] == "run":
            require(request["data"] is not None)
            identifier(request["run_id"])
        else:
            require(request["data"] is None and request["run_id"] is None)
        return request
    except CallerError as error:
        if error.code in ("file_io", "file_limit"):
            raise
        raise CallerError("request") from None
    except (OSError, ValueError, TypeError):
        raise CallerError("request") from None


def capture(engine, args, timeout_ms):
    require(os.name == "posix", "platform")
    process = None
    selector = selectors.DefaultSelector()
    buffers = {"stdout": bytearray(), "stderr": bytearray()}
    deadline = time.monotonic() + timeout_ms / 1000
    try:
        try:
            process = subprocess.Popen([str(engine), *args], stdin=subprocess.DEVNULL,
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                       shell=False, start_new_session=True)
        except OSError:
            raise CallerError("launch") from None
        for name in buffers:
            pipe = getattr(process, name)
            os.set_blocking(pipe.fileno(), False)
            selector.register(pipe, selectors.EVENT_READ, name)
        while selector.get_map():
            remaining = deadline - time.monotonic()
            require(remaining > 0, "timeout")
            for key, _mask in selector.select(min(remaining, 0.1)):
                try:
                    chunk = os.read(key.fd, 65536)
                except BlockingIOError:
                    continue
                if not chunk:
                    selector.unregister(key.fileobj)
                    key.fileobj.close()
                    continue
                target = buffers[key.data]
                limit = MAX_STDOUT if key.data == "stdout" else MAX_STDERR
                require(len(chunk) <= limit - len(target), "output_limit")
                target.extend(chunk)
        try:
            status = process.wait(timeout=max(0, deadline - time.monotonic()))
        except subprocess.TimeoutExpired:
            raise CallerError("timeout") from None
        return status, bytes(buffers["stdout"]), bytes(buffers["stderr"])
    finally:
        selector.close()
        if process is not None:
            # The owned session also covers descendants that retain inherited pipes.
            # Escaped process groups are outside this trusted-executable contract.
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            try:
                process.wait(timeout=1)
            except subprocess.TimeoutExpired:
                raise CallerError("cleanup") from None
            finally:
                for name in buffers:
                    pipe = getattr(process, name)
                    if pipe is not None:
                        pipe.close()


def descriptor(value):
    require(type(value) is dict)
    kind = value.get("kind")
    extra = "scale unit" if kind == "decimal" else "max_bytes" if kind == "text" else ""
    fields(value, "version shape kind " + extra)
    require(type(value["version"]) is int and value["version"] == 1 and value["shape"] == "scalar")
    require(kind in ("bool", "i64", "u64", "decimal", "text"))
    if kind == "text":
        small(value["max_bytes"], 1, 4096)
    if kind == "decimal":
        small(value["scale"], 0, 18)
        unit = value["unit"]
        require(type(unit) is dict)
        name = unit.get("kind")
        fields(unit, "kind " + ("instrument" if name in ("price", "quantity") else "currency" if name == "money" else ""))
        require(name in ("scalar", "ratio", "price", "quantity", "money"))
        if name in ("price", "quantity"):
            identifier(unit["instrument"], INSTRUMENT)
        elif name == "money":
            require(type(unit["currency"]) is str and re.fullmatch(r"[A-Z0-9]{1,16}", unit["currency"]))
    return value


def typed(value):
    fields(value, "type value")
    typ = descriptor(value["type"])
    kind, payload = typ["kind"], value["value"]
    if kind == "bool":
        require(type(payload) is bool)
    elif kind == "u64":
        number(payload)
    elif kind == "i64":
        number(payload, -(2**63), 2**63 - 1)
    elif kind == "decimal":
        number(payload, -(2**127), 2**127 - 1)
    else:
        require(type(payload) is str and len(payload.encode("utf-8")) <= typ["max_bytes"])
    return typ


def trigger(value):
    require(type(value) is dict)
    if value.get("kind") == "tick":
        fields(value, "kind instrument")
    else:
        fields(value, "kind instrument timeframe_ns price_scale volume_scale")
        require(value["kind"] == "closed_bar")
        number(value["timeframe_ns"], 1)
        small(value["price_scale"], 0, 12)
        small(value["volume_scale"], 0, 18)
    identifier(value["instrument"], INSTRUMENT)
    return value


def ports(value):
    require(type(value) is list and len(value) <= 32)
    result = {}
    for port in value:
        fields(port, "name type")
        name = identifier(port["name"], INSTRUMENT)
        require(name not in result)
        result[name] = descriptor(port["type"])
    return result


def framed(data):
    require(data.endswith(b"\n") and data.count(b"\n") == 1)
    return decode(data)


def validate_result(result, request, sources):
    """Verify wire/types/provenance and consistency, never compute model expressions."""
    is_run = request["operation"] == "run"
    fields(result, "format schema_version manifest " + ("observations summary" if is_run else "status inputs outputs node_count"))
    require(result["format"] == ("fqe.run-result" if is_run else "fqe.validation-result"))
    require(type(result["schema_version"]) is int and result["schema_version"] == 1)
    manifest = fields(result["manifest"], "artifact_id artifact_source_id execution_id graph_id registry_id parameters_source_id trigger warmup_samples " + ("run dataset_id engine_version interval_convention" if is_run else ""))
    for name in ("artifact_id", "artifact_source_id", "execution_id", "graph_id", "registry_id"):
        digest(manifest[name])
    for name, target in (("model", "artifact_source_id"), ("parameters", "parameters_source_id")):
        expected = raw_digest(sources[name]) if name in sources else None
        require(manifest[target] == expected, "input_changed")
    model = decode(sources["model"], 16384)
    require(type(model) is dict and model.get("format") == "fqe.event-plan" and type(model.get("schema_version")) is int and model["schema_version"] == 1)
    require(trigger(manifest["trigger"]) == trigger(model.get("trigger")))
    warmup = number(manifest["warmup_samples"])
    require(warmup == number(model.get("warmup_samples")))
    roots = model.get("outputs")
    require(type(roots) is list and 1 <= len(roots) <= 32)
    names = [identifier(root.get("name"), INSTRUMENT) for root in roots if type(root) is dict]
    require(len(names) == len(roots) == len(set(names)))
    if not is_run:
        require(result["status"] == "valid")
        number(result["node_count"], 0, 256)
        require(ports(result["inputs"]) == ports(model["graph"]["graph"]["inputs"]))
        require(set(ports(result["outputs"])) == set(names))
        return
    require(manifest["run"] == request["run_id"])
    require(manifest["dataset_id"] == raw_digest(sources["data"]), "input_changed")
    require(type(manifest["engine_version"]) is str and 1 <= len(manifest["engine_version"]) <= 32)
    domain = manifest["trigger"]
    convention = "UTC ns [open, close); timestamp = close" if domain["kind"] == "closed_bar" else "UTC ns timestamp; sequence resolves ties; no bar interval"
    require(manifest["interval_convention"] == convention)
    data = fields(decode(sources["data"], 16384), "format schema_version events")
    require(data["format"] == "fqe.market-data" and type(data["schema_version"]) is int and data["schema_version"] == 1)
    events, observations = data["events"], result["observations"]
    require(type(events) is list and type(observations) is list and len(events) == len(observations) <= 1024)
    rules = model.get("signals")
    require(type(rules) is list and len(rules) <= 32)
    signals = {}
    for rule in rules:
        fields(rule, "output kind")
        name = rule["output"]
        require(type(name) is str and name in names and name not in signals and rule["kind"] in ("long", "flat"))
        signals[name] = rule["kind"]
    matched_count = signal_count = 0
    previous_sequence = previous_time = None
    output_types = {}
    for event, observation in zip(events, observations):
        require(type(event) is dict)
        row = fields(observation, "run execution_id source_sequence timestamp_ns matched warmup outputs signals")
        require(row["run"] == request["run_id"] and row["execution_id"] == manifest["execution_id"])
        seq, timestamp = number(row["source_sequence"], 1), number(row["timestamp_ns"])
        require(row["source_sequence"] == event.get("sequence") and row["timestamp_ns"] == event.get("timestamp_ns"))
        require(previous_sequence is None or seq > previous_sequence)
        require(previous_time is None or timestamp >= previous_time)
        previous_sequence, previous_time = seq, timestamp
        require(event.get("kind") == domain["kind"])
        matched = event.get("instrument") == domain["instrument"]
        require(domain["kind"] == "tick" or matched)
        require(type(row["matched"]) is bool and row["matched"] == matched and type(row["warmup"]) is bool)
        require(type(row["outputs"]) is dict and type(row["signals"]) is list)
        if not matched:
            require(not row["outputs"] and not row["signals"] and not row["warmup"])
            continue
        matched_count += 1
        require(matched_count > warmup or row["warmup"])
        require(set(row["outputs"]) == set(names))
        for name, value in row["outputs"].items():
            require(type(value) is dict)
            if value.get("status") == "ready":
                fields(value, "status value")
                typ = typed(value["value"])
                require(name not in output_types or typ == output_types[name])
                output_types[name] = typ
            else:
                fields(value, "status")
                require(value["status"] in ("missing", "warmup"))
        expected = []
        for name, kind in sorted(signals.items()):
            value = row["outputs"][name]
            if value["status"] == "ready":
                require(value["value"]["type"]["kind"] == "bool")
                if value["value"]["value"] and not row["warmup"]:
                    expected.append(dict(source_sequence=row["source_sequence"], timestamp_ns=row["timestamp_ns"], instrument=domain["instrument"], kind=kind))
        require(row["signals"] == expected)
        signal_count += len(expected)
    summary = fields(result["summary"], "status processed_events matched_events signals last_sequence last_timestamp_ns")
    require(summary["status"] == "completed")
    require(number(summary["processed_events"]) == len(events))
    require(number(summary["matched_events"]) == matched_count and number(summary["signals"]) == signal_count)
    require(summary["last_sequence"] == (str(previous_sequence) if previous_sequence is not None else None))
    require(summary["last_timestamp_ns"] == (str(previous_time) if previous_time is not None else None))


def engine_diagnostic(value, status):
    fields(value, "format schema_version stage code path message")
    require(value["format"] == "fqe.diagnostic" and type(value["schema_version"]) is int and value["schema_version"] == 1)
    stages = {2: ("arguments",), 3: ("model_io", "model", "parameters_io", "parameters", "catalog", "compile", "data_io", "data"), 4: ("execution",), 5: ("result", "output")}
    require(type(value["stage"]) is str and value["stage"] in stages[status])
    for name, maximum in (("code", 128), ("path", 512), ("message", 4096)):
        require(type(value[name]) is str and 0 < len(value[name]) <= maximum)
    return value


def response(request_id, status, engine_exit=None, result=None, diagnostic=None, error=None):
    return dict(format="fqe.invoke-response", schema_version=1, request_id=request_id,
                status=status, engine_exit=engine_exit, result=result, diagnostic=diagnostic, error=error)


def invoke(engine, request_path):
    request_id = None
    try:
        request = load_request(Path(request_path).resolve())
        request_id = request["request_id"]
        sources = {name: read_file(request[name], MAX_FILE) for name in ("model", "parameters", "data") if request[name] is not None}
        args = [request["operation"], "--model", str(request["model"])]
        if request["parameters"] is not None:
            args += ["--params", str(request["parameters"])]
        if request["operation"] == "run":
            args += ["--data", str(request["data"]), "--run-id", request["run_id"]]
        status, stdout, stderr = capture(Path(engine).resolve(), args, request["timeout_ms"])
        if status != 0:
            require(status in (2, 3, 4, 5) and not stdout, "process")
            diagnostic = engine_diagnostic(framed(stderr), status)
            return response(request_id, "engine_error", status, diagnostic=diagnostic), 11
        require(not stderr)
        result = framed(stdout)
        try:
            validate_result(result, request, sources)
        except (KeyError, TypeError, ValueError, OverflowError):
            raise CallerError("protocol") from None
        return response(request_id, "ok", 0, result=result), 0
    except CallerError as error:
        code = 10 if error.code in ("request", "file_io", "file_limit") else 12
        return response(request_id, "caller_error", error=dict(code=error.code, message=MESSAGES[error.code])), code
    except OSError:
        return response(request_id, "caller_error", error=dict(code="process", message=MESSAGES["process"])), 12


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", required=True, type=Path, help="explicit trusted local executable")
    parser.add_argument("--request", required=True, type=Path, help="fqe.invoke-request/1 JSON file")
    options = parser.parse_args()
    packet, status = invoke(options.engine, options.request)
    stream = sys.stdout.buffer if status == 0 else sys.stderr.buffer
    try:
        payload = json.dumps(packet, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
        require(len(payload) <= MAX_STDOUT + MAX_STDERR, "output_limit")
        remaining = memoryview(payload + b"\n")
        while remaining:
            written = os.write(stream.fileno(), remaining)
            require(written > 0, "output_io")
            remaining = remaining[written:]
    except (OSError, CallerError):
        return 12
    return status


if __name__ == "__main__":
    raise SystemExit(main())
