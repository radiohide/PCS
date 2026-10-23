"""Take one lightweight disk sample for the explicitly reset PCScope instance.

Reads file sizes and free disk space only: no database query, checkpoint or collector.
The reset manifest is local/ignored. Repeated runs append samples for the same database.
"""
from collections import deque
from datetime import datetime, timezone
import json
from pathlib import Path
import shutil


def measure(root):
    out = root / "target/storage-monitor"
    manifest_path = out / "reset-result.json"
    if not manifest_path.exists():
        return {"state": "waiting_for_reset"}
    manifest = json.loads(manifest_path.read_text(encoding="utf-8-sig"))
    if manifest.get("state") != "enhanced_backend_verified":
        return {"state": "waiting_for_reset"}
    config = json.loads((root / "dist/dev/pcscope-launch.json").read_text(encoding="utf-8-sig"))
    data = Path(config["data_dir"]).resolve()
    if data != Path(manifest["data_dir"]).resolve():
        return {"state": "instance_changed", "message": "Configured data directory changed; no sample appended."}
    database = data / "history.db"
    stat = database.stat()
    identity = [stat.st_dev, stat.st_ino]
    if identity != manifest.get("database_file_id"):
        return {"state": "instance_changed", "message": "Database was replaced; no sample appended."}
    files = {}
    for suffix in ["", "-wal", "-shm"]:
        try:
            files["history.db" + suffix] = Path(str(database) + suffix).stat().st_size
        except FileNotFoundError:
            files["history.db" + suffix] = 0
    now = datetime.now(timezone.utc)
    sample = {"state": "sampled", "at": now.isoformat(), "baseline": manifest["baseline"]["started_at"],
        "files": files, "total_bytes": sum(files.values()), "disk_free_bytes": shutil.disk_usage(data).free}
    log = out / "samples.jsonl"
    previous = []
    if log.exists():
        with log.open(encoding="utf-8") as stream:
            for line in deque(stream, maxlen=96):
                row = json.loads(line)
                if row["baseline"] == sample["baseline"]:
                    previous.append(row)
    if previous:
        seconds = (now - datetime.fromisoformat(previous[-1]["at"])).total_seconds()
        sample["interval_seconds"] = round(seconds, 3)
        sample["interval_net_bytes"] = sample["total_bytes"] - previous[-1]["total_bytes"]
        # A short startup sample is not extrapolated into a daily growth prediction.
        if seconds >= 600:
            sample["interval_net_bytes_per_hour"] = round(sample["interval_net_bytes"] * 3600 / seconds)
    with log.open("a", encoding="utf-8") as stream:
        stream.write(json.dumps(sample) + "\n")
    return sample


if __name__ == "__main__":
    print(json.dumps(measure(Path(__file__).resolve().parent.parent), ensure_ascii=False, indent=2))
