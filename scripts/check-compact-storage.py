"""Compare the same fixture benchmark built against v0.2.42 and the current release.
Only creates databases in a new output directory. No production database is opened.
"""
import argparse
import hashlib
import json
import shutil
import sqlite3
import subprocess
from pathlib import Path


def run(command, **kwargs):
    result = subprocess.run([str(x) for x in command], capture_output=True, timeout=180, **kwargs)
    assert result.returncode == 0, result.stderr.decode(errors="replace")
    return result


def rows(db, table):
    with sqlite3.connect(db) as conn:
        return conn.execute(f'SELECT * FROM "{table}" ORDER BY rowid').fetchall()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["before", "after", "cli", "old-cli", "out"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--samples", type=int, default=3)
    args = parser.parse_args()
    assert 1 <= args.samples <= 10
    args.out.mkdir(parents=True, exist_ok=False)
    report = {"production_access": False, "paired_runs": [], "fixture": "100 services, 60 checks; one stop and reversion"}
    tables = ["snapshots", "snapshot_contents", "records", "object_instances", "object_facts", "fact_history", "object_actions"]
    for i in range(args.samples):
        pair = {}
        for label in ["before", "after"]:
            db = args.out / f"{label}-{i}.db"
            run([getattr(args, label).resolve(), db.resolve(), "60"])
            data = json.loads(db.with_suffix(".json").read_text(encoding="utf-8"))
            pair[label] = {k: v for k, v in data.items() if k not in ["samples", "usage"]}
        for table in tables:
            assert rows(args.out / f"before-{i}.db", table) == rows(args.out / f"after-{i}.db", table), table
        assert pair["before"]["evidence"] == pair["after"]["evidence"] == 6000
        assert pair["after"]["database_bytes"] < pair["before"]["database_bytes"] * 0.8
        report["paired_runs"].append(pair)
    report["semantic_tables_identical"] = tables
    # Upgrade a CLOSED synthetic v11 database. Preserve all raw/history/model rows byte-for-byte.
    upgrade = args.out / "upgrade.db"
    shutil.copyfile(args.out / "before-0.db", upgrade)
    with sqlite3.connect(upgrade) as c:
        c.execute("INSERT INTO settings VALUES('native_program_identity','pcscope-native-v1')")
    before = {table: rows(upgrade, table) for table in tables + ["evidence", "evidence_refs", "projection_queue"]}
    run([args.cli.resolve(), "ingest", upgrade.resolve()], input=b"")
    for table, data in before.items():
        assert rows(upgrade, table) == data, f"upgrade changed {table}"
    with sqlite3.connect(upgrade) as c:
        assert c.execute("PRAGMA user_version").fetchone()[0] == 12
        assert c.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
        assert not c.execute("PRAGMA foreign_key_check").fetchall()
        assert c.execute("SELECT count(*) FROM storage_source_bytes").fetchone()[0] == 0
        # Reuse a pre-upgrade content block: only the new envelope is accounted, not old content.
        body = json.loads(c.execute("SELECT payload FROM snapshots WHERE run_id='59'").fetchone()[0])
        key = body.pop("_pcscope_shared_items_v1")
        body["items"] = json.loads(c.execute("SELECT items FROM snapshot_contents WHERE id=?", (key,)).fetchone()[0])
    body.update(run_id="upgrade-check", observed_at="2026-10-24T00:00:00Z")
    # Persisted snapshot bodies omit the outer envelope fields; restore them for ingest.
    body.update(protocol_version=1, request_id="upgrade-check", type="snapshot")
    run([args.cli.resolve(), "ingest", upgrade.resolve()], input=(json.dumps(body) + "\n").encode())
    with sqlite3.connect(upgrade) as c:
        assert c.execute("SELECT count(*) FROM all_evidence").fetchone()[0] == 6100
        assert c.execute("SELECT count(*) FROM observation_runs").fetchone()[0] == 1
        assert c.execute("SELECT count(*) FROM storage_content_owners").fetchone()[0] == 0
        assert c.execute("SELECT retained_bytes FROM storage_source_bytes").fetchone()[0] == c.execute("SELECT length(CAST(payload AS BLOB)) FROM snapshots WHERE run_id='upgrade-check'").fetchone()[0]
    downgrade = subprocess.run([str(args.old_cli.resolve()), "ingest", str(upgrade.resolve())], input=b"", capture_output=True, timeout=10)
    assert downgrade.returncode != 0 and b"newer" in downgrade.stderr
    report["upgrade"] = {"history_preserved": True, "old_content_not_recounted": True, "mixed_history_items": 6100,
        "older_writer_refused": True, "raw_history_sha256": hashlib.sha256(json.dumps(before, ensure_ascii=False).encode()).hexdigest()}
    (args.out / "result.json").write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False))


if __name__ == "__main__":
    main()
