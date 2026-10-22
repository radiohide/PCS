import importlib.util
import json
import os
import shutil
import sqlite3
from pathlib import Path
import tempfile
import unittest
import zipfile

spec = importlib.util.spec_from_file_location("backup", Path(__file__).with_name("backup-offline-database.py"))
backup = importlib.util.module_from_spec(spec)
spec.loader.exec_module(backup)


class OfflineBackupTests(unittest.TestCase):
    def test_restored_sqlite_recovers_committed_rows_from_wal(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "offline"
            source.mkdir()
            live = root / "live.db"
            connection = sqlite3.connect(live)
            connection.execute("PRAGMA journal_mode=WAL")
            connection.execute("PRAGMA wal_autocheckpoint=0")
            connection.execute("CREATE TABLE baseline(value TEXT)")
            connection.commit()
            connection.execute("PRAGMA wal_checkpoint(TRUNCATE)")
            connection.execute("INSERT INTO baseline VALUES('committed only in WAL')")
            connection.commit()
            # Make a stable offline fixture without checkpointing its copied WAL.
            for suffix in ["", "-wal", "-shm"]:
                shutil.copyfile(str(live) + suffix, source / ("history.db" + suffix))
            connection.close()
            output = root / "backup.zip"
            backup.backup(source, output, reserve_bytes=0)
            restored = root / "restored"
            with zipfile.ZipFile(output) as archive:
                for name in ["history.db", "history.db-wal", "history.db-shm"]:
                    archive.extract(name, restored)
            with sqlite3.connect(restored / "history.db") as database:
                self.assertEqual(database.execute("SELECT value FROM baseline").fetchall(), [("committed only in WAL",)])
                self.assertEqual(database.execute("PRAGMA integrity_check").fetchone()[0], "ok")
            database.close()

    def test_all_database_bytes_and_wal_survive_archive_verification(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            source.mkdir()
            original = {"history.db": os.urandom(5 * 1024**2), "history.db-wal": b"uncheckpointed commit" * 1024, "history.db-shm": b"index"}
            for name, data in original.items():
                (source / name).write_bytes(data)
            output = root / "backup.zip"
            result = backup.backup(source, output, reserve_bytes=0)
            self.assertEqual(result["phase"], "verified")
            with zipfile.ZipFile(output) as archive:
                for name, data in original.items():
                    self.assertEqual(archive.read(name), data)
                    self.assertEqual((source / name).read_bytes(), data)
                self.assertEqual(json.loads(archive.read("backup-manifest.json")), result["manifest"])
            with self.assertRaises(FileExistsError):
                backup.backup(source, output, reserve_bytes=0)

    def test_insufficient_space_never_publishes_verified_archive_or_changes_original(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "history.db").write_bytes(b"database")
            with self.assertRaises(OSError):
                backup.backup(root, root / "backup.zip", reserve_bytes=2**63)
            self.assertFalse((root / "backup.zip").exists())
            self.assertEqual((root / "history.db").read_bytes(), b"database")
            self.assertEqual(json.loads((root / "backup.zip.status.json").read_text())["phase"], "failed")

    @unittest.skipUnless(os.name == "nt", "Windows file sharing")
    def test_open_source_denies_concurrent_write(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "history.db"
            path.write_bytes(b"database")
            with backup.locked_reader(path):
                with self.assertRaises(OSError):
                    path.open("rb+")


if __name__ == "__main__":
    unittest.main()
