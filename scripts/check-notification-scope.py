"""Read-only native notification scope smoke test; writes only isolated test results."""
import json
from pathlib import Path
import shutil
import subprocess
import time

root = Path(__file__).resolve().parent.parent
out = root / "target" / ("notification-scope-" + str(time.time_ns()))
out.mkdir()
exe = out / "pcscope-platform-native.exe"
shutil.copyfile(root / "target/debug/pcscope-platform-native.exe", exe)
subprocess.run(["icacls.exe", str(exe), "/setintegritylevel", "M"], check=True, capture_output=True)
cases = [
    ("network-change", "pcscope-powershell-adapter.exe", ["pcscope.network.adapters", "pcscope.network.addresses"]),
    ("device-change", "pcscope-powershell-adapter.exe", ["pcscope.hardware.devices", "pcscope.storage.volumes", "pcscope.network.adapters", "pcscope.network.addresses"]),
    ("routes", "network", ["pcscope.network.routes"]),
    ("storage", "storage", ["pcscope.hardware.driver-packages", "pcscope.hardware.driver-services", "pcscope.hardware.driver-bindings", "pcscope.storage.disks", "pcscope.storage.partitions"]),
]
report = []
for label, unit, allowed in cases:
    tick = time.perf_counter()
    result = subprocess.run([str(exe), unit], input=json.dumps({"selection": {"allowed_sources": allowed}}).encode(), capture_output=True, timeout=120)
    assert result.returncode == 0, result.stderr.decode(errors="replace")
    rows = [json.loads(line) for line in result.stdout.splitlines()]
    sources = sorted({row["source"] for row in rows})
    assert set(sources) == set(allowed), (label, sources)
    summary = {"case": label, "ms": round((time.perf_counter() - tick) * 1000), "sources": sources,
        "snapshots": sum(row["type"] == "snapshot" for row in rows),
        "gaps": [{"source": row["source"], "reason": row["reason"]} for row in rows if row["type"] == "gap"]}
    report.append(summary)
    print(json.dumps(summary), flush=True)
(root / "target/storage-frequency-native-scope.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
