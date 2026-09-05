#!/usr/bin/env python3
"""Execute declared causal-intelligence proofs; existing evidence is never input.

Only an executed test framework can supply test counts. A fresh artifact can
supplement those tests, but its claimed status cannot make a task pass.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import uuid


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_VERSION = 1
MAX_OUTPUT_BYTES = 8 * 1024 * 1024
MAX_JSON_BYTES = 2 * 1024 * 1024


class InvalidProof(ValueError):
    """The requested proof is missing or cannot establish its declared claim."""


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def unique_object(pairs: list[tuple[str, object]]) -> dict:
    result = {}
    for key, value in pairs:
        if key in result:
            raise InvalidProof(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def read_json(path: Path) -> dict:
    if path.stat().st_size > MAX_JSON_BYTES:
        raise InvalidProof(f"JSON proof exceeds bounded size: {path.name}")
    value = json.loads(path.read_text(), object_pairs_hook=unique_object)
    if not isinstance(value, dict):
        raise InvalidProof(f"expected JSON object: {path.name}")
    return value


def internal_file(root: Path, value: str) -> Path:
    path = (root / value).resolve()
    if not path.is_relative_to(root.resolve()) or not path.is_file():
        raise InvalidProof(f"missing or external source file: {value}")
    return path


def fingerprint(root: Path, paths: list[str]) -> dict[str, str]:
    hashes = {}
    for value in paths:
        path = (root / value).resolve()
        if not path.is_relative_to(root.resolve()) or not path.exists():
            raise InvalidProof(f"missing or external source path: {value}")
        candidates = sorted(path.rglob("*")) if path.is_dir() else [path]
        files = [item for item in candidates if item.is_file() and "__pycache__" not in item.parts]
        if not files:
            raise InvalidProof(f"empty source path: {value}")
        for item in files:
            if not item.resolve().is_relative_to(root.resolve()):
                raise InvalidProof(f"source symlink escapes repository: {value}")
            hashes[item.relative_to(root).as_posix()] = digest(item)
    return hashes


def identity_digest(value: dict) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest()


def external_fixtures(paths: list[str]) -> dict[str, str]:
    hashes = {}
    for value in paths:
        path = Path(value)
        if not path.is_absolute() or not path.is_file():
            raise InvalidProof("external fixture must be an absolute regular file")
        if any(part.is_symlink() for part in [path, *path.parents]):
            raise InvalidProof("external fixture symlinks are forbidden")
        if str(path) in hashes:
            raise InvalidProof("external fixture paths must be unique")
        hashes[str(path)] = digest(path)
    return hashes


def test_counts(kind: str, output: str, expected: int) -> dict:
    if kind == "unittest":
        counts = re.findall(r"^Ran (\d+) tests? in [^\n]+$", output, re.MULTILINE)
        endings = re.findall(r"^OK(?: \(([^\n]*)\))?$", output, re.MULTILINE)
        if len(counts) != 1 or len(endings) != 1:
            raise InvalidProof("missing or ambiguous unittest execution summary")
        count = int(counts[0])
        if endings[0]:
            raise InvalidProof("required unittest cases skipped or not successful")
    elif kind == "cargo_test":
        summaries = re.findall(
            r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;",
            output,
            re.MULTILINE,
        )
        if not summaries or any(int(row[1]) or int(row[2]) or int(row[3]) for row in summaries):
            raise InvalidProof("missing cargo test summary or required cases failed/ignored")
        count = sum(int(row[0]) for row in summaries)
    else:
        raise InvalidProof(f"unknown counted test format: {kind}")
    if count <= 0 or count != expected:
        raise InvalidProof(f"expected {expected} executed cases, observed {count}")
    return {"executed": count, "passed": count, "skipped": 0}


def validate_manifest(manifest: dict, task: str) -> None:
    if manifest.get("schema_version") != SCHEMA_VERSION or manifest.get("task") != task:
        raise InvalidProof("manifest schema or task mismatch")
    cases = manifest.get("cases")
    if not isinstance(cases, list) or not cases:
        raise InvalidProof("manifest must declare nonempty required cases")
    ids = [case.get("id") for case in cases if isinstance(case, dict)]
    if len(ids) != len(cases) or any(not isinstance(value, str) or not re.fullmatch(r"[a-zA-Z0-9_.-]+", value) for value in ids):
        raise InvalidProof("invalid case ID")
    if len(set(ids)) != len(ids) or manifest.get("required_case_ids") != ids:
        raise InvalidProof("required case IDs must match the complete ordered manifest")
    if not isinstance(manifest.get("source_paths"), list) or not manifest["source_paths"]:
        raise InvalidProof("manifest must identify implementation and test sources")
    for key in ["source_paths", "config_paths", "binary_paths", "external_fixture_paths"]:
        values = manifest.get(key, [])
        if not isinstance(values, list) or any(not isinstance(value, str) or not value for value in values):
            raise InvalidProof(f"{key} must contain path strings")
    for case in cases:
        kind = case.get("kind")
        count = case.get("expected_case_count")
        if type(count) is not int or count <= 0:
            raise InvalidProof("expected case count must be a positive integer")
        if kind == "unavailable":
            if not case.get("reason"):
                raise InvalidProof("unavailable case needs a reason")
            continue
        artifacts = case.get("required_artifacts", [])
        if not isinstance(artifacts, list) or any(not isinstance(value, str) or not value for value in artifacts):
            raise InvalidProof("required_artifacts must contain relative JSON paths")
        argv = case.get("argv")
        if not isinstance(argv, list) or not argv or any(not isinstance(arg, str) or not arg for arg in argv):
            raise InvalidProof("case needs a nonempty argv, never a shell expression")
        if kind == "unittest":
            if len(argv) < 4 or argv[1:3] != ["-m", "unittest"] or not Path(argv[0]).name.startswith("python"):
                raise InvalidProof("unittest proof must invoke Python's unittest module")
        elif kind in {"cargo_test", "cargo_clippy"}:
            subcommand = "test" if kind == "cargo_test" else "clippy"
            if Path(argv[0]).name != "cargo" or argv[1:2] != [subcommand] or "--locked" not in argv or "-p" not in argv:
                raise InvalidProof("Rust proof requires focused cargo test/clippy -p ... --locked")
            if kind == "cargo_clippy" and (count != 1 or any(flag not in argv for flag in ["--all-targets", "--no-deps", "--", "-D", "warnings"])):
                raise InvalidProof("Clippy requires one strict all-targets/no-deps check")
        elif kind == "dependency_policy":
            if argv != ["cargo", "dependency-policy"] or count != 1:
                raise InvalidProof("dependency proof must execute cargo dependency-policy")
        else:
            raise InvalidProof(f"unsupported proof kind: {kind}")
        timeout = case.get("timeout_seconds", 300)
        if type(timeout) not in (int, float) or not 0 < timeout <= 7200:
            raise InvalidProof("case timeout must be positive and at most 7200 seconds")


def run_process(argv: list[str], root: Path, env: dict, timeout: float) -> dict:
    # Temporary files avoid unbounded RAM; no raw provider output enters evidence.
    with tempfile.TemporaryFile() as output:
        process = subprocess.Popen(argv, cwd=root, env=env, stdout=output, stderr=subprocess.STDOUT, start_new_session=True)
        deadline = time.monotonic() + timeout
        timed_out = False
        output_exceeded = False
        try:
            while process.poll() is None:
                timed_out = time.monotonic() >= deadline
                output_exceeded = os.fstat(output.fileno()).st_size > MAX_OUTPUT_BYTES
                if timed_out or output_exceeded:
                    break
                time.sleep(0.02)
        finally:
            # Reap the subprocess and any same-group descendants on interruption,
            # timeout, output overflow, or a command that left children behind.
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait()
        output.seek(0, os.SEEK_END)
        size = output.tell()
        output.seek(0)
        raw = output.read(MAX_OUTPUT_BYTES + 1)
        return {"exit_code": process.returncode, "timed_out": timed_out, "output_limit_exceeded": output_exceeded or size > MAX_OUTPUT_BYTES, "output_bytes": size, "captured_output_sha256": hashlib.sha256(raw).hexdigest(), "text": raw.decode(errors="replace") if size <= MAX_OUTPUT_BYTES else None}


def execute_case(root: Path, case: dict, run_dir: Path, run_id: str, source_digest: str, env: dict) -> dict:
    result = {"id": case["id"], "kind": case["kind"], "expected_case_count": case["expected_case_count"], "status": "failed"}
    if case["kind"] == "unavailable":
        return {**result, "status": "blocked", "reason": case["reason"]}
    try:
        requested_executable = shutil.which(case["argv"][0], path=env.get("PATH"))
        if requested_executable is None:
            raise InvalidProof("declared command executable is unavailable")
        invocation_path = str(Path(requested_executable).absolute())
        executable = str(Path(invocation_path).resolve())
        result.update(argv=case["argv"], invocation_path=invocation_path, executable=executable, executable_sha256=digest(Path(executable)))
        artifact_dir = run_dir / case["id"]
        artifact_dir.mkdir()
        case_env = {**env, "KEITH_QUALIFICATION_RUN_ID": run_id, "KEITH_QUALIFICATION_CASE_ID": case["id"], "KEITH_QUALIFICATION_SOURCE_DIGEST": source_digest, "KEITH_QUALIFICATION_ARTIFACT_DIR": str(artifact_dir)}
        started = time.time_ns()
        result["started_at_unix_ns"] = started
        # Rustup and other multicall executables select behavior from argv[0].
        # Hash the resolved target, but preserve the invoked symlink's basename.
        process = run_process([invocation_path, *case["argv"][1:]], root, case_env, case.get("timeout_seconds", 300))
        result["finished_at_unix_ns"] = time.time_ns()
        output = process.pop("text")
        result.update(process)
        if process["timed_out"] or process["exit_code"] != 0:
            raise InvalidProof("declared subprocess timed out or returned nonzero")
        if output is None:
            raise InvalidProof("subprocess output exceeds bounded proof limit")
        if str(Path(invocation_path).resolve()) != executable or digest(Path(executable)) != result["executable_sha256"]:
            raise InvalidProof("command executable changed during execution")
        if case["kind"] in {"unittest", "cargo_test"}:
            result["counts"] = test_counts(case["kind"], output, case["expected_case_count"])
        else:
            result["checks_executed"] = 1
        artifacts = []
        for name in case.get("required_artifacts", []):
            path = internal_file(artifact_dir, name)
            if path.stat().st_size == 0 or path.stat().st_mtime_ns < started:
                raise InvalidProof("empty or stale required artifact")
            record = read_json(path)
            for key, expected in {"run_id": run_id, "case_id": case["id"], "source_digest": source_digest}.items():
                if record.get(key) != expected:
                    raise InvalidProof(f"required artifact invocation identity mismatch: {key}")
            artifacts.append({"path": path.relative_to(run_dir).as_posix(), "sha256": digest(path), "bytes": path.stat().st_size})
        result.update(status="passed", artifacts=artifacts)
    except (InvalidProof, OSError, ValueError) as error:
        result["reason"] = str(error)
    return result


def verify(root: Path, task: str, spec_path: Path, manifest_path: Path, evidence_root: Path) -> tuple[int, Path]:
    run_id = f"{time.strftime('%Y%m%dT%H%M%SZ', time.gmtime())}-{uuid.uuid4().hex}"
    run_dir = evidence_root / task / run_id
    run_dir.mkdir(parents=True, exist_ok=False)
    report = {"schema_version": SCHEMA_VERSION, "task": task, "run_id": run_id, "status": "failed", "cases": []}
    code = 1
    try:
        if not re.fullmatch(r"\d+\.\d+", task):
            raise InvalidProof("invalid task ID")
        spec = internal_file(root, str(spec_path))
        if f"[task.{task}]" not in spec.read_text():
            raise InvalidProof("task does not exist in requested spec")
        manifest_file = internal_file(root, str(manifest_path))
        manifest = read_json(manifest_file)
        validate_manifest(manifest, task)
        source_paths = [*manifest["source_paths"], *manifest.get("config_paths", []), str(spec), str(manifest_file), str(Path(__file__).resolve())]
        before = fingerprint(root, source_paths)
        binaries = {str((root / path).resolve()): digest((root / path).resolve()) for path in manifest.get("binary_paths", [])}
        fixtures = external_fixtures(manifest.get("external_fixture_paths", []))
        revision = subprocess.run(["git", "rev-parse", "HEAD"], cwd=root, capture_output=True, text=True, check=False)
        if revision.returncode != 0:
            raise InvalidProof("cannot establish current source Git revision")
        source_identity = {"git_revision": revision.stdout.strip(), "files": before, "binaries": binaries, "external_fixtures": fixtures}
        source_digest = identity_digest(source_identity)
        report.update(source_identity=source_identity, source_digest=source_digest)
        with tempfile.TemporaryDirectory(prefix="keith-causal-target-") as target:
            env = {**os.environ, "CARGO_INCREMENTAL": "0", "CARGO_TARGET_DIR": target, "PYTHONDONTWRITEBYTECODE": "1"}
            for case in manifest["cases"]:
                report["cases"].append(execute_case(root, case, run_dir, run_id, source_digest, env))
        if fingerprint(root, source_paths) != before or any(digest(Path(path)) != value for path, value in binaries.items()):
            raise InvalidProof("source, configuration, or binary changed during qualification")
        if external_fixtures(manifest.get("external_fixture_paths", [])) != fixtures:
            raise InvalidProof("external fixture changed during qualification")
        current_revision = subprocess.run(["git", "rev-parse", "HEAD"], cwd=root, capture_output=True, text=True, check=False)
        if current_revision.returncode != 0 or current_revision.stdout.strip() != source_identity["git_revision"]:
            raise InvalidProof("source Git revision changed during qualification")
        if all(case["status"] == "passed" for case in report["cases"]):
            report["status"] = "passed"
            code = 0
        elif any(case["status"] == "blocked" for case in report["cases"]):
            report["status"] = "blocked"
            code = 3
    except (InvalidProof, OSError, ValueError) as error:
        report["reason"] = str(error)
        code = 2
    report["finished_at_unix_ns"] = time.time_ns()
    report_path = run_dir / "result.json"
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    return code, report_path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["verify"])
    parser.add_argument("--task", required=True)
    parser.add_argument("--spec", type=Path, required=True)
    args = parser.parse_args()
    if not re.fullmatch(r"\d+\.\d+", args.task):
        parser.error("task must have the form 1.1")
    code, report_path = verify(ROOT, args.task, args.spec, ROOT / "tests/causal-intelligence/manifests" / f"{args.task}.json", ROOT / "evidence/causal-intelligence")
    print(f"qualification {'passed' if code == 0 else 'NOT PASSED'}: {report_path}")
    return code


if __name__ == "__main__":
    sys.exit(main())
