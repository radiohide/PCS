"""Isolated storage regression/performance check. Never opens a production database.

Usage: python scripts/check-storage-sharing.py --new target/release/pcscope-cli.exe
       [--old dist/dev/pcscope-cli.exe] [--out target/storage-sharing-bench]
"""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import time


def material(run):
    at = datetime.datetime(2026, 10, 23, tzinfo=datetime.timezone.utc) + datetime.timedelta(minutes=10 * run)
    return json.dumps({"protocol_version": 1, "request_id": str(run), "type": "snapshot",
        "source": "pcscope.startup.services", "scope": "machine:fixture/install:fixture/schema:1",
        "run_id": str(run), "observed_at": at.isoformat(), "complete": True,
        "items": [{"id": f"fixture-{i}", "data": {"name": f"fixture-{i}",
            "display_name": f"Fixture service {i}", "state": "Running", "description": "unchanged " * 150}}
            for i in range(100)]}, separators=(",", ":")) + "\n"


def ingest(exe, database, inputs):
    tick = time.perf_counter()
    result = subprocess.run([str(exe), "ingest", str(database)], input=inputs.encode(), capture_output=True, timeout=180)
    assert result.returncode == 0, result.stderr.decode(errors="replace")
    replies = [json.loads(line) for line in result.stdout.splitlines()]
    assert len(replies) == inputs.count("\n")
    return round((time.perf_counter() - tick) * 1000, 2)


def stats(database, shared):
    with sqlite3.connect(database) as c:
        assert c.execute("pragma integrity_check").fetchone()[0] == "ok"
        assert c.execute("pragma foreign_key_check").fetchall() == []
        snapshots = c.execute("select count(*),sum(length(cast(payload as blob))) from snapshots").fetchone()
        contents = c.execute("select count(*),sum(length(cast(items as blob))) from snapshot_contents").fetchone() if shared else (0, 0)
        return {"snapshot_rows": snapshots[0], "content_versions": contents[0],
            "payload_bytes": snapshots[1] + (contents[1] or 0), "database_bytes": database.stat().st_size,
            "evidence_rows": c.execute("select count(*) from evidence").fetchone()[0],
            "queue_rows": c.execute("select count(*) from projection_queue").fetchone()[0]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--new", type=Path, required=True)
    parser.add_argument("--old", type=Path)
    parser.add_argument("--out", type=Path, default=Path("target/storage-sharing-bench"))
    args = parser.parse_args()
    # Refuse reuse so a test can never append fixture data to a user's database.
    args.out.mkdir(parents=True, exist_ok=False)
    # Cargo outputs may carry low integrity on Windows. Detached test copies use the
    # caller's ordinary integrity; neither compiler files nor data directories are relabelled.
    for label in ["new", "old"]:
        executable = getattr(args, label)
        if executable:
            detached = args.out / (label + "-cli.exe")
            shutil.copyfile(executable, detached)
            if os.name == "nt":
                subprocess.run(["icacls.exe", str(detached), "/setintegritylevel", "M"], check=True, capture_output=True)
            setattr(args, label, detached)
    inputs = "".join(material(n) for n in range(60))
    report = {"fixture": "100 unchanged services, 60 observations, 10-minute spacing", "production_database_access": False}
    for label, exe, shared in [("new", args.new, True), ("old", args.old, False)]:
        if not exe:
            continue
        db = args.out / (label + ".db")
        elapsed = ingest(exe.resolve(), db, inputs)
        report[label] = stats(db, shared) | {"ingest_ms": elapsed}
        assert report[label]["snapshot_rows"] == 60
    assert report["new"]["content_versions"] == 1
    replay_ms = ingest(args.new.resolve(), args.out / "new.db", inputs)
    assert stats(args.out / "new.db", True) == {k: v for k, v in report["new"].items() if k != "ingest_ms"}
    report["replay_ms"] = replay_ms
    if args.old:
        assert report["old"]["evidence_rows"] == report["new"]["evidence_rows"]
        assert report["old"]["queue_rows"] == report["new"]["queue_rows"]
        report["payload_reduction_percent"] = round(100 * (1 - report["new"]["payload_bytes"] / report["old"]["payload_bytes"]), 2)
        report["database_reduction_percent"] = round(100 * (1 - report["new"]["database_bytes"] / report["old"]["database_bytes"]), 2)
        legacy = args.out / "old.db"
        with sqlite3.connect(legacy) as c:
            before = c.execute("select source,scope,run_id,payload from snapshots order by run_id").fetchall()
        report["upgrade_ms"] = ingest(args.new.resolve(), legacy, material(60))
        with sqlite3.connect(legacy) as c:
            assert c.execute("pragma user_version").fetchone()[0] == 11
            after = c.execute("select source,scope,run_id,payload from snapshots where run_id!='60' order by run_id").fetchall()
            assert before == after
            report["legacy_payload_sha256"] = hashlib.sha256(json.dumps(after).encode()).hexdigest()
        report["legacy_rows_preserved"] = len(before)
        downgraded = subprocess.run([str(args.old.resolve()), "ingest", str(legacy)], input=b"", capture_output=True, timeout=10)
        assert downgraded.returncode != 0 and b"newer" in downgraded.stderr
        report["older_writer_refused"] = True
    (args.out / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
