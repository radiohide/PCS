'use strict';
// Archive a legacy PCScope store BEFORE the new build opens it.
//
// WHY THIS EXISTS. `Store::open` upgrades an older database IN PLACE. Once the new build has opened
// the store, the old binary refuses it — "database version is newer than this application" — and the
// pre-upgrade shape is gone for good. The archive has to be taken before that first run.
//
// It has to be taken CONSISTENTLY, too. history.db carries a WAL and a shared-memory file, so a byte
// copy of the main file alone can be torn. This uses SQLite's online backup API against a source
// opened READ-ONLY, so it cannot alter the store it is protecting.
//
// The destination must be OUTSIDE the store's directory, and that is not cosmetic. The USN plugin
// resolves configured exclusion paths to (volume serial, file id) pairs at collector start
// (`plugins/usn/src/main.rs`) and the configured list is exactly history.db, its -wal and its -shm.
// An archive sitting next to the store is in neither that list nor the derived id set, so the
// collector would treat it as a fresh object to observe — a self-collection loop.
//
// Verification is the point. An archive nobody checked is not an archive, so this compares
// user_version, the table set and every table's row count against the source, and runs
// integrity_check on the copy. Any mismatch exits non-zero.
//
// Usage:
//   node scripts/archive-legacy-store.cjs
//   node scripts/archive-legacy-store.cjs --source <db> --destination-dir <dir> --expect-version 4
//   node scripts/archive-legacy-store.cjs --verify <archive>        # re-check an existing archive
//
// The script creates at most one new file, in the directory it is told to use, and never writes to
// the source or to an existing archive.

const fs = require('node:fs');
const path = require('node:path');
const { DatabaseSync, backup } = require('node:sqlite');

function argument(name, fallback) {
  const index = process.argv.indexOf(`--${name}`);
  if (index === -1) return fallback;
  const value = process.argv[index + 1];
  if (value === undefined || value.startsWith('--')) throw new Error(`--${name} 需要一个值`);
  return value;
}

const local = process.env.LOCALAPPDATA || process.env.HOME;
if (!local) throw new Error('无法确定本机应用数据目录（LOCALAPPDATA 未设置）');

const sourcePath = path.resolve(argument('source', path.join(local, 'local.pcscope.desktop', 'history.db')));
const destinationDir = path.resolve(argument('destination-dir', path.join(local, 'local.pcscope.desktop-archive')));
const verifyOnly = argument('verify', null);
const expected = argument('expect-version', null);
const expectedVersion = expected === null ? null : Number(expected);
if (expectedVersion !== null && !Number.isInteger(expectedVersion)) throw new Error('--expect-version 需要一个整数');

const failures = [];
const check = (condition, message) => { if (!condition) failures.push(message); };

function shape(conn) {
  const version = conn.prepare('PRAGMA user_version').get().user_version;
  const journal = conn.prepare('PRAGMA journal_mode').get().journal_mode;
  const tables = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name").all().map((row) => row.name);
  const counts = Object.fromEntries(tables.map((table) => [table, conn.prepare(`SELECT count(*) AS n FROM [${table}]`).get().n]));
  return { version, journal, tables, counts };
}

function report(label) {
  if (failures.length) {
    console.error(`\n${label}失败（${failures.length} 项）：`);
    for (const failure of failures) console.error(`  ! ${failure}`);
    process.exit(1);
  }
  console.log(`${label}通过：user_version、表集合、逐表行数、integrity_check 全部一致，且无遗留 WAL。`);
  // Exit here rather than returning. `report` is the last statement of each mode, and falling
  // through from verify into create mode would mean a read-only re-check silently wrote a new
  // archive — which it did, until this line was added.
  process.exit(0);
}

// Compare an archive against the source it claims to be a copy of. Read-only on both sides.
function compare(archivePath, archiveLabel) {
  const source = new DatabaseSync(sourcePath, { readOnly: true });
  const archive = new DatabaseSync(archivePath, { readOnly: true });
  const before = shape(source);
  const after = shape(archive);
  const integrity = archive.prepare('PRAGMA integrity_check').get().integrity_check;
  archive.close();
  source.close();

  if (expectedVersion !== null) {
    check(before.version === expectedVersion,
      `来源库 schema 为 ${before.version}，与 --expect-version ${expectedVersion} 不符`);
  }
  check(after.version === before.version, `user_version 不一致：来源 ${before.version}，${archiveLabel} ${after.version}`);
  check(integrity === 'ok', `${archiveLabel} integrity_check 为 ${integrity}`);
  const missing = before.tables.filter((table) => !after.tables.includes(table));
  const extra = after.tables.filter((table) => !before.tables.includes(table));
  check(missing.length === 0, `${archiveLabel}缺少表：${missing.join(', ')}`);
  check(extra.length === 0, `${archiveLabel}多出表：${extra.join(', ')}`);
  const differing = before.tables.filter((table) => after.counts[table] !== before.counts[table])
    .map((table) => `${table} ${before.counts[table]}→${after.counts[table]}`);
  check(differing.length === 0, `行数不一致：${differing.join(', ')}`);
  // A reference copy has to be self-contained. A -wal holding committed pages would mean the archive
  // is incomplete without its sidecar. A -shm is only a scratch index that SQLite recreates and that
  // never holds committed data — and opening a WAL database, even read-only, creates one — so this
  // asserts the journal mode and the -wal rather than any sidecar at all.
  const wal = archivePath + '-wal';
  const walSize = fs.existsSync(wal) ? fs.statSync(wal).size : 0;
  check(walSize === 0, `${archiveLabel}遗留 -wal（${walSize} 字节）：已提交内容落在 .db 之外，留档不完整`);
  check(after.journal !== 'wal', `${archiveLabel}仍为 WAL 模式（journal_mode=${after.journal}）：参考副本应当是自足的 DELETE 模式`);
  return { before, after, integrity, fileBytes: fs.statSync(archivePath).size };
}

if (!fs.existsSync(sourcePath)) {
  console.error(`来源库不存在：${sourcePath}`);
  process.exit(1);
}

// --- Verify-only mode: re-check an archive already on disk, writing nothing ----------------------
if (verifyOnly !== null) {
  const archivePath = path.resolve(verifyOnly);
  if (!fs.existsSync(archivePath)) {
    console.error(`留档不存在：${archivePath}`);
    process.exit(1);
  }
  const result = compare(archivePath, '留档');
  console.log(JSON.stringify({
    source: sourcePath,
    source_version: result.before.version,
    archive: archivePath,
    archive_bytes: result.fileBytes,
    tables: result.after.tables.length,
    rows: Object.values(result.after.counts).reduce((total, n) => total + n, 0),
    integrity_check: result.integrity,
  }, null, 2));
  report('留档校验');
}

// --- Create mode ---------------------------------------------------------------------------------
const sourceDir = path.resolve(path.dirname(sourcePath));
if (destinationDir === sourceDir || destinationDir.startsWith(sourceDir + path.sep)) {
  console.error(`留档目录不能位于来源库目录内：\n  来源目录 ${sourceDir}\n  留档目录 ${destinationDir}`);
  console.error('  USN 采集按「路径 → 文件 ID」排除自身库，邻近的留档不在其中，会成为新的自采集对象。');
  process.exit(1);
}

const stamp = new Date().toISOString().replace(/[-:]/g, '').replace(/\..+$/, '');
const name = argument('name', '');
if (name && (name.includes('/') || name.includes('\\') || name === '.' || name === '..')) {
  throw new Error('--name 只能是文件名，不能包含路径分隔符');
}
const destinationPath = path.resolve(destinationDir, name || `history-legacy-${stamp}.db`);
if (fs.existsSync(destinationPath)) {
  console.error(`留档文件已存在，拒绝覆盖：${destinationPath}`);
  process.exit(1);
}
fs.mkdirSync(path.dirname(destinationPath), { recursive: true });

const sourceBytes = fs.statSync(sourcePath).size;
let freeBytes = null;
try {
  const stat = fs.statfsSync(path.dirname(destinationPath));
  freeBytes = stat.bsize * stat.bavail;
} catch { /* best effort; the backup API reports a real failure if it runs out */ }
if (freeBytes !== null && freeBytes < sourceBytes * 1.1) {
  console.error(`空间不足：需要约 ${(sourceBytes * 1.1 / 1073741824).toFixed(1)} GiB，可用 ${(freeBytes / 1073741824).toFixed(1)} GiB`);
  process.exit(1);
}

const started = Date.now();
const source = new DatabaseSync(sourcePath, { readOnly: true });
const preview = shape(source);

// Refuse before writing anything if the source is not the shape the caller expects.
if (expectedVersion !== null && preview.version !== expectedVersion) {
  console.error(`拒绝留档：来源库 schema 为 ${preview.version}，与 --expect-version ${expectedVersion} 不符`);
  source.close();
  process.exit(1);
}
source.close();

backup(new DatabaseSync(sourcePath, { readOnly: true }), destinationPath).then(() => {
  // The source is in WAL mode, so the copy inherits it. Switch the reference copy to DELETE mode so
  // it carries no -wal and stays readable as a single self-contained file by any SQLite tool.
  const normalize = new DatabaseSync(destinationPath);
  normalize.exec('PRAGMA journal_mode=DELETE');
  normalize.close();

  const result = compare(destinationPath, '留档');
  console.log(JSON.stringify({
    source: sourcePath,
    source_version: result.before.version,
    source_bytes: sourceBytes,
    destination: destinationPath,
    destination_bytes: result.fileBytes,
    tables: result.after.tables.length,
    rows: Object.values(result.after.counts).reduce((total, n) => total + n, 0),
    integrity_check: result.integrity,
    seconds: Number(((Date.now() - started) / 1000).toFixed(1)),
  }, null, 2));
  report('留档');
}, (error) => {
  console.error(`备份失败：${error.message}`);
  process.exit(1);
});
