"""Byte-exact offline SQLite ZIP64 backup, including uncheckpointed WAL.

Stop the application first. Originals are never modified or removed. On Windows,
read handles deny writes/deletion until the entire archive has been verified.
This checks backup fidelity, not the logical health of the source database.
"""
import argparse
import contextlib
import ctypes
import hashlib
import json
import os
from pathlib import Path
import shutil
import time
import zipfile


def locked_reader(path):
    if os.name != "nt":
        return path.open("rb")
    import msvcrt
    from ctypes import wintypes
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    create = kernel.CreateFileW
    create.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD,
                       ctypes.c_void_p, wintypes.DWORD, wintypes.DWORD, wintypes.HANDLE]
    create.restype = wintypes.HANDLE
    handle = create(str(path), 0x80000000, 1, None, 3, 0x08000000, None)
    if handle == ctypes.c_void_p(-1).value:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        fd = msvcrt.open_osfhandle(handle, os.O_RDONLY | os.O_BINARY)
    except BaseException:
        kernel.CloseHandle.argtypes = [wintypes.HANDLE]
        kernel.CloseHandle(handle)
        raise
    return os.fdopen(fd, "rb")


def backup(source, output, reserve_bytes=2 * 1024**3):
    source, output = source.resolve(strict=True), output.absolute()
    if not (source / "history.db").is_file():
        raise ValueError("history.db is required")
    output.parent.mkdir(parents=True, exist_ok=True)
    partial = output.with_name(output.name + ".partial")
    status = output.with_name(output.name + ".status.json")
    if output.exists() or partial.exists() or status.exists():
        raise FileExistsError("Backup destination already exists; choose a new name")
    started, updated = time.monotonic(), 0
    progress = {"phase": "opening", "source": str(source), "output": str(output)}

    def report(force=False):
        nonlocal updated
        now = time.monotonic()
        if force or now - updated >= 3:
            progress["elapsed_seconds"] = round(now - started, 1)
            progress["archive_bytes"] = partial.stat().st_size if partial.exists() else (output.stat().st_size if output.exists() else 0)
            progress["free_bytes"] = shutil.disk_usage(output.parent).free
            temp = status.with_name(status.name + ".tmp")
            temp.write_text(json.dumps(progress, ensure_ascii=False), encoding="utf-8")
            os.replace(temp, status)
            updated = now
            if progress["phase"] == "compressing" and progress["free_bytes"] < reserve_bytes:
                raise OSError("Free-space reserve reached; originals preserved; archive incomplete")

    try:
        with contextlib.ExitStack() as stack:
            files = []
            for name in ["history.db", "history.db-wal", "history.db-shm", "service-startup.json"]:
                path = source / name
                if path.exists():
                    if path.is_symlink() or (getattr(path.stat(), "st_file_attributes", 0) & 0x400):
                        raise ValueError("Reparse points are not supported")
                    stream = stack.enter_context(locked_reader(path))
                    info = os.fstat(stream.fileno())
                    files.append((name, stream, info.st_size, info.st_mtime_ns))
            progress.update(phase="compressing", total_bytes=sum(row[2] for row in files), processed_bytes=0)
            report(True)
            manifest = {"format": "pcscope-offline-zip-v1", "source": str(source),
                        "restore": "Restore history.db and its WAL together while the application is stopped. Protected evidence requires the original Windows account and DPAPI keys.",
                        "files": []}
            with zipfile.ZipFile(partial, "x", compression=zipfile.ZIP_DEFLATED, compresslevel=6, allowZip64=True) as archive:
                for name, stream, size, mtime in files:
                    progress["file"] = name
                    digest, count = hashlib.sha256(), 0
                    with archive.open(name, "w", force_zip64=True) as entry:
                        while block := stream.read(4 * 1024**2):
                            entry.write(block)
                            digest.update(block)
                            count += len(block)
                            progress["processed_bytes"] += len(block)
                            report()
                    after = os.fstat(stream.fileno())
                    if count != size or after.st_size != size or after.st_mtime_ns != mtime:
                        raise RuntimeError("Source changed during backup")
                    manifest["files"].append({"name": name, "bytes": size, "mtime_ns": mtime, "sha256": digest.hexdigest()})
                archive.writestr("backup-manifest.json", json.dumps(manifest, ensure_ascii=False, indent=2))
            with partial.open("rb+") as archive_file:
                os.fsync(archive_file.fileno())
            progress.update(phase="verifying", processed_bytes=0)
            report(True)
            with zipfile.ZipFile(partial) as archive:
                if json.loads(archive.read("backup-manifest.json")) != manifest:
                    raise RuntimeError("Manifest mismatch")
                for row in manifest["files"]:
                    progress["file"] = row["name"]
                    digest, count = hashlib.sha256(), 0
                    with archive.open(row["name"]) as entry:
                        while block := entry.read(4 * 1024**2):
                            digest.update(block)
                            count += len(block)
                            progress["processed_bytes"] += len(block)
                            report()
                    if count != row["bytes"] or digest.hexdigest() != row["sha256"]:
                        raise RuntimeError("Decompressed data does not match the source")
            os.rename(partial, output)
            progress.update(phase="verified", manifest=manifest)
            report(True)
        return progress
    except BaseException as error:
        progress.update(phase="failed", error=str(error))
        report(True)
        raise


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    print(json.dumps(backup(args.source, args.output), ensure_ascii=False))
